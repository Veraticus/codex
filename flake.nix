{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.nightly.latest.default;
        rustPlatform = pkgs.makeRustPlatform {
          inherit (pkgs) stdenv;
          cargo = rustToolchain;
          rustc = rustToolchain;
        };
        rmcpSrc = pkgs.fetchgit {
          url = "https://github.com/modelcontextprotocol/rust-sdk";
          rev = "c0b777c7f784ba2d456b03c2ec3b98c9b28b5e10";
          hash = "sha256-uAEBai6Uzmpi5fcIn9v4MPE9DbzPvemkaaZ+alwM4PQ=";
        };
        ratatuiSrc = pkgs.fetchgit {
          url = "https://github.com/nornagon/ratatui";
          rev = "9b2ad1298408c45918ee9f8241a6f95498cdbed2";
          hash = "sha256-HBvT5c8GsiCxMffNjJGLmHnvG77A6cqEL+1ARurBXho=";
        };
        cargoPatchConfig = pkgs.writeText "cargo-config.toml" ''
          [patch."https://github.com/modelcontextprotocol/rust-sdk"]
          rmcp = { path = "${rmcpSrc}/crates/rmcp" }
          rmcp-macros = { path = "${rmcpSrc}/crates/rmcp-macros" }

          [patch.crates-io]
          ratatui = { path = "${ratatuiSrc}" }
        '';
        codex-tui = rustPlatform.buildRustPackage {
          pname = "codex-tui";
          version = "unstable";
          src = ./codex-rs;
          cargoLock = {
            lockFile = ./codex-rs/Cargo.lock;
            outputHashes = {
              "ratatui-0.29.0" = "sha256-HBvT5c8GsiCxMffNjJGLmHnvG77A6cqEL+1ARurBXho=";
              "rmcp-0.7.0" = "sha256-uAEBai6Uzmpi5fcIn9v4MPE9DbzPvemkaaZ+alwM4PQ=";
              "rmcp-macros-0.7.0" = "sha256-uAEBai6Uzmpi5fcIn9v4MPE9DbzPvemkaaZ+alwM4PQ=";
            };
          };
          cargoSha256 = "sha256-NP94EW+XS1PrbFfMnGOCnwoNoT1S7txJ8bDD6xRb5hw=";
          cargoBuildFlags = [ "--package" "codex-tui" "--bin" "codex-tui" ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs;
            [ openssl libgit2 curl zlib ]
            ++ lib.optionals stdenv.isDarwin [ libiconv Security CoreServices ];
          preBuild = ''
            export CARGO_HOME="$TMPDIR/cargo-home"
            mkdir -p "$CARGO_HOME"
            cp ${cargoPatchConfig} "$CARGO_HOME/config.toml"
          '';
          doCheck = false;
          meta = with pkgs.lib; {
            description = "Codex TUI built from codex-rs";
            homepage = "https://github.com/sourcegraph/codex";
            license = licenses.asl20;
            mainProgram = "codex-tui";
            platforms = platforms.unix;
          };
        };
      in {
        packages = {
          codex-tui = codex-tui;
          default = codex-tui;
        };
        apps =
          let
            codexApp = flake-utils.lib.mkApp { drv = codex-tui; };
          in {
            codex-tui = codexApp;
            default = codexApp;
          };
      }
    );
}
