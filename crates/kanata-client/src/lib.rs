//! Everything the overlay needs to talk to Kanata.
//!
//! - [`client`]: a blocking client for Kanata's TCP server (newline-delimited JSON).
//! - [`launcher`]: builds and spawns a managed `kanata` child process.
//! - [`supervisor`]: keeps a managed Kanata running (check, start, restart, stop).
//! - [`probe`]: is a Kanata answering at an address?
//!
//! The wire types come from the upstream `kanata-tcp-protocol` crate and are
//! re-exported as [`protocol`] so the rest of the app never redefines them.

mod backoff;
pub mod client;
mod failure;
pub mod input_devices;
pub mod launcher;
mod probe;
pub mod supervisor;

pub use client::{Client, Incoming};
pub use failure::Failure;
pub use input_devices::{
    DeviceVerdict, InputDevice, check_devices, conflicts, grabs, input_devices,
    parse_input_devices, resolve_links,
};
pub use kanata_tcp_protocol as protocol;
pub use launcher::LaunchOptions;
pub use probe::{Probe, probe};
pub use supervisor::{SessionState, Supervisor, SupervisorEvent, SupervisorOptions};

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
