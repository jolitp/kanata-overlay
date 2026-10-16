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
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          # cxx-qt locates Qt through a single qmake prefix, but Nix puts every Qt
          # module in its own store path. qt6.env merges them into one prefix with
          # a qmake (and qt.conf) that reports it.
          qtEnv = pkgs.qt6.env "kanata-overlay-qt" [
            pkgs.qt6.qtdeclarative # QML / Qt Quick
            pkgs.qt6.qtwayland # Wayland platform plugin
            pkgs.qt6.qtsvg # SVG icons and key legends
            pkgs.kdePackages.layer-shell-qt # Wayland layer-shell: always-on-top, click-through overlay
          ];
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
