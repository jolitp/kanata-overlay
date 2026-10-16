use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("io.github.jolitp.kanataoverlay")
            .qml_files(["qml/Main.qml", "qml/KeyCap.qml"]),
    )
    .qt_module("Quick")
    .files(["src/overlay_state.rs"])
    .build();

    println!("cargo:rerun-if-changed=../../examples/layouts/sample.kle.json");
}
