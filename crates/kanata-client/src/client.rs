//! Blocking client for Kanata's TCP server.
//!
//! Kanata sends one JSON object per line. Most lines are [`ServerMessage`]s
//! (layer changes, config reloads, pushed messages…); replies to some commands
//! are bare `{"status": "Ok"}` / `{"status": "Error", ...}` lines, which are
//! [`ServerResponse`]s. [`Incoming`] covers both.
//!
//! The client is deliberately synchronous: the app runs it on its own thread and
//! forwards events to the Qt thread, which keeps this crate free of an async runtime.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use kanata_tcp_protocol::{ClientMessage, ServerMessage, ServerResponse};

use crate::{Error, Result};

/// One line received from Kanata.
#[derive(Debug)]
pub enum Incoming {
    /// An event or a reply carrying data.
    Message(ServerMessage),
    /// A plain status reply to a command.
    Response(ServerResponse),
}

impl Incoming {
    /// Decode one line of Kanata output.
    pub fn parse(line: &str) -> Result<Self> {
        match serde_json::from_str::<ServerMessage>(line) {
            Ok(msg) => Ok(Self::Message(msg)),
            Err(msg_err) => serde_json::from_str::<ServerResponse>(line)
                .map(Self::Response)
                .map_err(|_| Error::Decode {
                    line: line.to_owned(),
                    source: msg_err,
                }),
        }
    }
}

/// A connection to a running Kanata TCP server.
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    peer: SocketAddr,
}

impl Client {
    /// Connect to Kanata, giving up after `timeout` per resolved address.
    pub fn connect(addr: impl ToSocketAddrs, timeout: Duration) -> Result<Self> {
        let mut last_err = None;
        for addr in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&addr, timeout) {
                Ok(stream) => return Self::from_stream(stream),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err
            .unwrap_or_else(|| std::io::Error::other("address resolved to nothing"))
            .into())
    }

    fn from_stream(stream: TcpStream) -> Result<Self> {
        stream.set_nodelay(true)?;
        let peer = stream.peer_addr()?;
        let writer = stream.try_clone()?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer,
            peer,
        })
    }

    /// Address of the Kanata server this client is connected to.
    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    /// Send one command to Kanata.
    pub fn send(&mut self, msg: &ClientMessage) -> Result<()> {
        let mut line = serde_json::to_vec(msg).map_err(Error::Encode)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Block until the next line arrives and decode it.
    ///
    /// Returns [`Error::Closed`] when Kanata closes the connection (e.g. it exited).
    pub fn recv(&mut self) -> Result<Incoming> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.reader.read_line(&mut line)? == 0 {
                return Err(Error::Closed);
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return Incoming::parse(trimmed);
            }
        }
    }

    /// Iterate over incoming lines until the connection closes or errors.
    pub fn incoming(&mut self) -> impl Iterator<Item = Result<Incoming>> + '_ {
        std::iter::from_fn(move || match self.recv() {
            Err(Error::Closed) => None,
            other => Some(other),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn parses_layer_change() {
        let msg = Incoming::parse(r#"{"LayerChange":{"new":"nav"}}"#).unwrap();
        assert!(matches!(
            msg,
            Incoming::Message(ServerMessage::LayerChange { ref new }) if new == "nav"
        ));
    }

    #[test]
    fn parses_status_response() {
        let msg = Incoming::parse(r#"{"status":"Ok"}"#).unwrap();
        assert!(matches!(msg, Incoming::Response(ServerResponse::Ok)));
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            Incoming::parse("not json"),
            Err(Error::Decode { .. })
        ));
    }

    /// End-to-end against a fake Kanata: it greets with the current layer (as the
    /// real server does on connect), then answers a layer-name request.
    #[test]
    fn talks_to_fake_server() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        let server = thread::spawn(move || -> anyhow::Result<String> {
            let (mut stream, _) = listener.accept()?;
            stream.write_all(b"{\"LayerChange\":{\"new\":\"base\"}}\n")?;
            let mut request = String::new();
            BufReader::new(stream.try_clone()?).read_line(&mut request)?;
            stream.write_all(b"{\"LayerNames\":{\"names\":[\"base\",\"nav\"]}}\n")?;
            Ok(request)
        });

        let mut client = Client::connect(addr, Duration::from_secs(2))?;
        assert!(matches!(
            client.recv()?,
            Incoming::Message(ServerMessage::LayerChange { ref new }) if new == "base"
        ));

        client.send(&ClientMessage::RequestLayerNames {})?;
        match client.recv()? {
            Incoming::Message(ServerMessage::LayerNames { names }) => {
                assert_eq!(names, ["base", "nav"]);
            }
            other => panic!("unexpected reply: {other:?}"),
        }

        let request = server.join().unwrap()?;
        assert_eq!(request.trim(), r#"{"RequestLayerNames":{}}"#);

        assert!(client.incoming().next().is_none(), "server closed");
        Ok(())
    }
}
