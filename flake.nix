{
  description = "Dev shell for github_fetcher with musl toolchain";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };
        lib = pkgs.lib;
        useMusl = pkgs.stdenv.isLinux;
        rustToolchain =
          if useMusl then
            pkgs.rust-bin.stable.latest.default.override {
              targets = [
                "x86_64-unknown-linux-musl"
                "aarch64-unknown-linux-musl"
              ];
            }
          else
            pkgs.rust-bin.stable.latest.default;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };
        muslCc = pkgs.pkgsCross.musl64.stdenv.cc;
        aarch64MuslCc = pkgs.pkgsCross.aarch64-multiplatform-musl.stdenv.cc;
        linuxMuslEnv = lib.optionalAttrs useMusl {
          CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
          CC_x86_64_unknown_linux_musl = "${muslCc}/bin/x86_64-unknown-linux-musl-gcc";
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = "${muslCc}/bin/x86_64-unknown-linux-musl-gcc";
          AR_x86_64_unknown_linux_musl = "${muslCc.bintools}/bin/x86_64-unknown-linux-musl-ar";
          CC_aarch64_unknown_linux_musl = "${aarch64MuslCc}/bin/aarch64-unknown-linux-musl-gcc";
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER = "${aarch64MuslCc}/bin/aarch64-unknown-linux-musl-gcc";
          AR_aarch64_unknown_linux_musl = "${aarch64MuslCc.bintools}/bin/aarch64-unknown-linux-musl-ar";
          nativeBuildInputs = [
            muslCc
            aarch64MuslCc
          ];
        };
      in
      {
        packages = rec {
          default = github_fetcher;

          github_fetcher = rustPlatform.buildRustPackage (
            {
              pname = "github_fetcher";
              version = "0.1.0";
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
            }
            // linuxMuslEnv
          );
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.pkg-config
            pkgs.cacert
          ]
          ++ lib.optionals useMusl [
            muslCc
            aarch64MuslCc
          ];

          shellHook =
            if useMusl then
              ''
                export CC_x86_64_unknown_linux_musl=${muslCc}/bin/x86_64-unknown-linux-musl-gcc
                export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=$CC_x86_64_unknown_linux_musl
                export AR_x86_64_unknown_linux_musl=${muslCc.bintools}/bin/x86_64-unknown-linux-musl-ar
                export CC_aarch64_unknown_linux_musl=${aarch64MuslCc}/bin/aarch64-unknown-linux-musl-gcc
                export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=$CC_aarch64_unknown_linux_musl
                export AR_aarch64_unknown_linux_musl=${aarch64MuslCc.bintools}/bin/aarch64-unknown-linux-musl-ar
                export CARGO_BUILD_TARGET=x86_64-unknown-linux-musl
                echo "Using musl toolchain (x86_64 + aarch64): $CC_x86_64_unknown_linux_musl / $CC_aarch64_unknown_linux_musl"
              ''
            else
              ''
                echo "Using host toolchain (no musl override)"
              '';
        };
      }
    );
}
