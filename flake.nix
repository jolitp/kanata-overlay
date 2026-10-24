{
  description = "kanata-overlay: an on-screen keyboard overlay for the Kanata remapper (Rust + Qt 6/QML)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      # cxx-qt locates Qt through a single qmake prefix, but Nix puts every Qt
      # module in its own store path. qt6.env merges them into one prefix with
      # a qmake (and qt.conf) that reports it.
      qtEnvFor =
        pkgs:
        pkgs.qt6.env "kanata-overlay-qt" [
          pkgs.qt6.qtdeclarative # QML / Qt Quick
          pkgs.qt6.qtwayland # Wayland platform plugin
          pkgs.qt6.qtsvg # SVG icons and key legends
          pkgs.kdePackages.layer-shell-qt # Wayland layer-shell: always-on-top, click-through overlay
        ];
    in
    {
      # The installable app (feature 007, research R9): the binary wrapped with
      # the Qt paths and Kanata, so it starts from the application menu and at
      # login, outside the development shell.
      packages = forAllSystems (
        pkgs:
        let
          qtEnv = qtEnvFor pkgs;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "kanata-overlay";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [
              "-p"
              "kanata-overlay"
            ];
            # The tests run in the development shell (`just check`).
            doCheck = false;
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.cmake
              pkgs.makeWrapper
            ];
            buildInputs = [
              qtEnv
              pkgs.libGL
              pkgs.kdePackages.kglobalaccel
            ];
            QMAKE = "${qtEnv}/bin/qmake";
            KF6_GLOBALACCEL_INCLUDE_DIR = "${pkgs.kdePackages.kglobalaccel.dev}/include/KF6/KGlobalAccel";
            postInstall = ''
              wrapProgram $out/bin/kanata-overlay \
                --set QT_PLUGIN_PATH "${qtEnv}/${pkgs.qt6.qtbase.qtPluginPrefix}" \
                --set QML_IMPORT_PATH "${qtEnv}/${pkgs.qt6.qtbase.qtQmlPrefix}" \
                --set QML2_IMPORT_PATH "${qtEnv}/${pkgs.qt6.qtbase.qtQmlPrefix}" \
                --prefix PATH : "${pkgs.kanata}/bin" \
                --set KANATA_OVERLAY_WRAPPER "$out/bin/kanata-overlay"
            '';
            meta.mainProgram = "kanata-overlay";
          };
        }
      );

      devShells = forAllSystems (
        pkgs:
        let
          qtEnv = qtEnvFor pkgs;
        in
        {
          default = pkgs.mkShell {
            packages = [
              qtEnv
            ]
            ++ (with pkgs; [
              # Rust toolchain
              rustc
              cargo
              clippy
              rustfmt
              rust-analyzer

              # C++ side of cxx-qt
              pkg-config
              cmake
              clang-tools # clangd for the small C++ shims

              # Project tooling
              just
              uv # runs the Spec Kit CLI (`just speckit ...`)
              git

              # Kanata itself and input debugging
              kanata
              evtest
            ]);

            # Qt Gui links against libglvnd's -lGLX / -lOpenGL; as a build input
            # the cc wrapper puts its lib dir on the linker search path.
            # KGlobalAccel: global shortcuts (research R5); as a build input its
            # headers/lib land on the cc-wrapper paths too.
            buildInputs = [
              pkgs.libGL
              pkgs.kdePackages.kglobalaccel
            ];

            # KGlobalAccel's headers include each other without the KF6/ prefix.
            KF6_GLOBALACCEL_INCLUDE_DIR = "${pkgs.kdePackages.kglobalaccel.dev}/include/KF6/KGlobalAccel";

            # cxx-qt-build finds Qt through this qmake.
            QMAKE = "${qtEnv}/bin/qmake";

            # Let `cargo run` find Qt plugins and QML modules without a wrapper.
            QT_PLUGIN_PATH = "${qtEnv}/${pkgs.qt6.qtbase.qtPluginPrefix}";
            QML_IMPORT_PATH = "${qtEnv}/${pkgs.qt6.qtbase.qtQmlPrefix}";
            QML2_IMPORT_PATH = "${qtEnv}/${pkgs.qt6.qtbase.qtQmlPrefix}";

            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            RUST_BACKTRACE = "1";

            shellHook = ''
              echo "kanata-overlay dev shell: rustc $(rustc --version | cut -d' ' -f2), Qt ${pkgs.qt6.qtbase.version}, kanata $(kanata --version 2>/dev/null | cut -d' ' -f2)"
              echo "run 'just' to list tasks"
            '';
          };
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
