//! Print everything a running Kanata sends.
//!
//!     kanata --cfg my.kbd --port 7070    # in another terminal
//!     cargo run -p kanata-client --example watch -- 127.0.0.1:7070

use std::time::Duration;

use kanata_client::protocol::ClientMessage;
use kanata_client::{Client, Incoming};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7070".to_owned());

    let mut client = Client::connect(addr.as_str(), Duration::from_secs(2))?;
    println!("connected to kanata at {}", client.peer());
    client.send(&ClientMessage::RequestLayerNames {})?;

    for line in client.incoming() {
        match line? {
            Incoming::Message(msg) => println!("{msg:?}"),
            Incoming::Response(resp) => println!("  ↳ {resp:?}"),
        }
    }
    println!("kanata closed the connection");
    Ok(())
}
