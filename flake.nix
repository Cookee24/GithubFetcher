{
  description = "Dev shell for github_fetcher with musl toolchain";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "x86_64-unknown-linux-musl" ];
        };
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };
        muslCc = pkgs.pkgsCross.musl64.stdenv.cc;
      in
      {
        packages = rec {
          default = github_fetcher;

          github_fetcher = rustPlatform.buildRustPackage {
            pname = "github_fetcher";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.pkg-config
            muslCc
            pkgs.cacert
          ];

          shellHook = ''
            export CC_x86_64_unknown_linux_musl=${muslCc}/bin/x86_64-unknown-linux-musl-gcc
            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=$CC_x86_64_unknown_linux_musl
            export AR_x86_64_unknown_linux_musl=${muslCc.bintools}/bin/x86_64-unknown-linux-musl-ar
            export RUSTFLAGS="-C target-feature=+crt-static"
            echo "Using musl toolchain: $CC_x86_64_unknown_linux_musl"
          '';
        };
      });
}
