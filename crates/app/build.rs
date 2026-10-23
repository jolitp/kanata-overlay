use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // Set by the Nix dev shell; the default fits distro packages.
    println!("cargo:rerun-if-env-changed=KF6_GLOBALACCEL_INCLUDE_DIR");
    let kglobalaccel = std::env::var("KF6_GLOBALACCEL_INCLUDE_DIR")
        .unwrap_or_else(|_| "/usr/include/KF6/KGlobalAccel".to_owned());

    let builder =
        CxxQtBuilder::new_qml_module(QmlModule::new("io.github.jolitp.kanataoverlay").qml_files([
            "qml/Main.qml",
            "qml/Board.qml",
            "qml/KeyCap.qml",
            "qml/LayerShellOverlay.qml",
            "qml/FallbackOverlay.qml",
            "qml/SettingsWindow.qml",
            "qml/LogWindow.qml",
        ]))
        .qt_module("Quick")
        .qt_module("DBus") // KGlobalAccel's headers use QtDBus types
        .files([
            "src/overlay_state.rs",
            "src/settings_state.rs",
            "src/kanata_state.rs",
            "src/global_shortcut.rs",
            "src/platform.rs",
        ])
        // Thin C++ shims: KGlobalAccel (research R5) and QGuiApplication statics.
        .cpp_files(["cpp/global_shortcut.cpp", "cpp/qt_extras.cpp"]);
    // SAFETY: only adds an include path to the C++ compiler invocation.
    unsafe {
        builder.cc_builder(|cc| {
            cc.include(&kglobalaccel);
        })
    }
    .build();

    // KF6GlobalAccel's lib dir comes from the dev shell's cc wrapper (flake.nix).
    println!("cargo:rustc-link-lib=KF6GlobalAccel");
    println!("cargo:rerun-if-changed=cpp/global_shortcut.h");
    println!("cargo:rerun-if-changed=cpp/qt_extras.h");
    println!("cargo:rerun-if-changed=../../examples/layouts/sample.kle.json");
}
