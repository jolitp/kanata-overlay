//! Is a Kanata answering at an address? (research R2)

use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::protocol::ServerMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// Something answered with Kanata's protocol.
    Kanata,
    /// Nothing is listening.
    Nothing,
    /// Something is listening, but it doesn't speak Kanata's protocol.
    NotKanata,
}

/// Connect and wait for Kanata's greeting. Kanata sends a `LayerChange` to every
/// new client, so the probe only listens: asking for something and hanging up
/// before the reply makes Kanata log "Broken pipe". `timeout` applies to the
/// connect and to the greeting separately.
pub fn probe(addr: SocketAddr, timeout: Duration) -> Probe {
    let Ok(stream) = TcpStream::connect_timeout(&addr, timeout) else {
        return Probe::Nothing;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let mut line = String::new();
    match BufReader::new(stream).read_line(&mut line) {
        Ok(n) if n > 0 && serde_json::from_str::<ServerMessage>(line.trim()).is_ok() => {
            Probe::Kanata
        }
        _ => Probe::NotKanata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    const T: Duration = Duration::from_millis(300);

    fn serve(reply: &'static [u8]) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut c, _)) = listener.accept() {
                let _ = c.write_all(reply);
                thread::sleep(Duration::from_millis(500));
            }
        });
        addr
    }

    #[test]
    fn kanata() {
        let addr = serve(b"{\"CurrentLayerName\":{\"name\":\"base\"}}\n");
        assert_eq!(probe(addr, T), Probe::Kanata);
    }

    #[test]
    fn nothing() {
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        assert_eq!(probe(addr, T), Probe::Nothing);
    }

    #[test]
    fn not_kanata() {
        assert_eq!(
            probe(serve(b"HTTP/1.1 400 Bad Request\r\n"), T),
            Probe::NotKanata
        );
        assert_eq!(probe(serve(b""), T), Probe::NotKanata);
    }
}
