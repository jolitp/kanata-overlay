//! Everything the overlay needs to talk to Kanata.
//!
//! - [`client`]: a blocking client for Kanata's TCP server (newline-delimited JSON).
//! - [`launcher`]: builds and spawns a managed `kanata` child process.
//!
//! The wire types come from the upstream `kanata-tcp-protocol` crate and are
//! re-exported as [`protocol`] so the rest of the app never redefines them.

pub mod client;
pub mod launcher;

pub use client::{Client, Incoming};
pub use kanata_tcp_protocol as protocol;
pub use launcher::LaunchOptions;

/// Errors from talking to or launching Kanata.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("could not decode a line from kanata: {line:?}: {source}")]
    Decode {
        line: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not encode a message for kanata: {0}")]
    Encode(#[source] serde_json::Error),

    #[error("connection closed by kanata")]
    Closed,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
