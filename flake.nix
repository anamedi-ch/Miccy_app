{
  description = "Miccy - Offline speech-to-text desktop app (fork-friendly, privacy-focused)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    supportedSystems = ["x86_64-linux"];
    forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    # Read version from Cargo.toml
    cargoToml = builtins.fromTOML (builtins.readFile ./src-tauri/Cargo.toml);
    version = cargoToml.package.version;
  in {
    packages = forAllSystems (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      # AppImage-based package
      miccy-appimage = let
        appimage = pkgs.appimageTools.wrapType2 {
          pname = "miccy-appimage-unwrapped";
          inherit version;
          src = pkgs.fetchurl {
            url = "https://github.com/anamedi-ch/anamedi_lokal/releases/download/v${version}/Miccy_${version}_amd64.AppImage";
            hash = "sha256-ZVDRDjru+sQrkpQsUlQH8i1mKjGbRDYxWsU46c1wxdI=";
          };
          extraPkgs = p:
            with p; [
              alsa-lib
            ];
        };
      in
        pkgs.writeShellScriptBin "miccy" ''
          export WEBKIT_DISABLE_DMABUF_RENDERER=1
          exec ${appimage}/bin/miccy-appimage-unwrapped "$@"
        '';

      default = self.packages.${system}.miccy-appimage;
    });

    # Development shell for building from source
    devShells = forAllSystems (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      default = pkgs.mkShell {
        buildInputs = with pkgs; [
          # Rust
          rustc
          cargo
          rust-analyzer
          clippy
          # Frontend
          nodejs
          bun
          # Tauri CLI
          cargo-tauri
          # Native deps
          pkg-config
          openssl
          alsa-lib
          libsoup_3
          webkitgtk_4_1
          gtk3
          glib
          libxtst
          libevdev
          llvmPackages.libclang
          cmake
          vulkan-headers
          vulkan-loader
          shaderc
          libappindicator
        ];

        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath [ pkgs.libappindicator ]}";

        shellHook = ''
          echo "Miccy development environment"
          bun install
          echo "Run 'bun run tauri dev' to start"
        '';
      };
    });
  };
}
