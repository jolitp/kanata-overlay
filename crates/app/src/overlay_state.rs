//! The QObject QML talks to.
//!
//! All state lives in Rust; QML only renders it. A background thread owns the
//! Kanata TCP connection and pushes updates onto the Qt thread with
//! `qt_thread().queue(...)`, the only safe way to touch a QObject from another
//! thread.

use std::pin::Pin;
use std::thread;
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use kanata_client::protocol::{ClientMessage, ServerMessage};
use kanata_client::{Client, Incoming};
use layout::Layout;

use crate::Args;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, current_layer)]
        #[qproperty(bool, connected)]
        #[qproperty(QString, status)]
        /// The physical layout as JSON (the `layout` crate's format); QML parses it.
        #[qproperty(QString, layout_json)]
        type OverlayState = super::OverlayStateRust;

        /// Called once by QML when the window is ready.
        #[qinvokable]
        fn start(self: Pin<&mut OverlayState>);

        /// Ask Kanata to switch layers (handy for testing the overlay).
        #[qinvokable]
        fn request_layer(self: Pin<&mut OverlayState>, name: &QString);
    }

    impl cxx_qt::Threading for OverlayState {}
}

#[derive(Default)]
pub struct OverlayStateRust {
    current_layer: QString,
    connected: bool,
    status: QString,
    layout_json: QString,
    started: bool,
    args: Args,
}

impl qobject::OverlayState {
    fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let args = Args::from_env();
        self.as_mut().rust_mut().args = args.clone();

        let layout = match &args.layout {
            Some(path) => std::fs::read_to_string(path)
                .map_err(anyhow::Error::from)
                .and_then(|text| load_layout(&text).map_err(Into::into)),
            None => load_layout(include_str!("../../../examples/layouts/sample.kle.json"))
                .map_err(Into::into),
        };
        match layout {
            Ok(layout) => self
                .as_mut()
                .set_layout_json(QString::from(&layout.to_json())),
            Err(e) => self
                .as_mut()
                .set_status(QString::from(&format!("layout error: {e:#}"))),
        }

        spawn_connection(self.qt_thread(), args.address);
    }

    fn request_layer(self: Pin<&mut Self>, name: &QString) {
        let address = self.rust().args.address.clone();
        let name = name.to_string();
        // Fire-and-forget on a short-lived connection; the main connection
        // will see the resulting LayerChange event.
        thread::spawn(move || {
            let result = Client::connect(address.as_str(), Duration::from_secs(1))
                .and_then(|mut c| c.send(&ClientMessage::ChangeLayer { new: name }));
            if let Err(e) = result {
                tracing::warn!("ChangeLayer failed: {e}");
            }
        });
    }
}

/// Accept either our own layout format or raw KLE JSON.
fn load_layout(text: &str) -> layout::Result<Layout> {
    Layout::from_json(text).or_else(|_| Layout::from_kle_json(text))
}

/// A state change to run on the Qt thread.
type Update = Box<dyn FnOnce(Pin<&mut qobject::OverlayState>) + Send>;

/// Keep a connection to Kanata alive, reconnecting forever.
fn spawn_connection(qt: cxx_qt::CxxQtThread<qobject::OverlayState>, address: String) {
    thread::spawn(move || {
        let push = |f: Update| {
            // Err means the QObject is gone (app quitting): nothing to do.
            let _ = qt.queue(f);
        };
        loop {
            push(Box::new({
                let address = address.clone();
                move |mut o| {
                    o.as_mut()
                        .set_status(QString::from(&format!("connecting to {address}…")))
                }
            }));

            match Client::connect(address.as_str(), Duration::from_secs(1)) {
                Ok(mut client) => {
                    push(Box::new(|mut o| {
                        o.as_mut().set_connected(true);
                        o.as_mut().set_status(QString::from("connected"));
                    }));
                    for line in client.incoming() {
                        match line {
                            Ok(Incoming::Message(ServerMessage::LayerChange { new }))
                            | Ok(Incoming::Message(ServerMessage::CurrentLayerName {
                                name: new,
                            })) => {
                                push(Box::new(move |o| o.set_current_layer(QString::from(&new))));
                            }
                            Ok(other) => tracing::debug!("kanata: {other:?}"),
                            Err(e) => {
                                tracing::warn!("kanata connection error: {e}");
                                break;
                            }
                        }
                    }
                    push(Box::new(|mut o| {
                        o.as_mut().set_connected(false);
                        o.as_mut().set_status(QString::from("disconnected"));
                    }));
                }
                Err(e) => tracing::debug!("connect to {address} failed: {e}"),
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}
