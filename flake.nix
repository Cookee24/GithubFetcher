{
  description = "Github Fetcher with multi-target support";

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
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };
        lib = pkgs.lib;

        baseRust = pkgs.rust-bin.stable.latest.default;

        isLinux = pkgs.stdenv.isLinux;

        muslCcX64 = pkgs.pkgsCross.musl64.stdenv.cc;
        muslCcArm64 = pkgs.pkgsCross.aarch64-multiplatform-musl.stdenv.cc;

        targetAliases = {
          "arm64-apple-darwin" = "aarch64-apple-darwin";
        };

        canonicalTarget = target: targetAliases.${target} or target;

        mkPackage =
          {
            target,
            targetPkgs ? pkgs,
            crossCc ? null,
          }:
          let
            rustTarget = canonicalTarget target;
            toolchain = baseRust.override { targets = [ rustTarget ]; };
            rustPlatform = targetPkgs.makeRustPlatform {
              cargo = toolchain;
              rustc = toolchain;
            };

            targetEnv = lib.optionalAttrs (crossCc != null) {
              "CC_${builtins.replaceStrings [ "-" ] [ "_" ] rustTarget}" = "${crossCc}/bin/${rustTarget}-gcc";
              "CARGO_TARGET_${builtins.replaceStrings [ "-" ] [ "_" ] (lib.toUpper rustTarget)}_LINKER" =
                "${crossCc}/bin/${rustTarget}-gcc";
              "AR_${builtins.replaceStrings [ "-" ] [ "_" ] rustTarget}" =
                "${crossCc.bintools}/bin/${rustTarget}-ar";
              CARGO_BUILD_TARGET = rustTarget;
            };
          in
          rustPlatform.buildRustPackage (
            {
              pname = "github_fetcher";
              version = "0.1.0";
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
              cargoBuildTarget = rustTarget;

              nativeBuildInputs = lib.optionals (crossCc != null) [ crossCc ];
            }
            // targetEnv
          );

      in
      {
        packages = {
          default = self.packages.${system}.github_fetcher;

          github_fetcher = mkPackage { target = pkgs.stdenv.hostPlatform.config; };
        }
        // lib.optionalAttrs isLinux {
          "x86_64-unknown-linux-musl" = mkPackage {
            target = "x86_64-unknown-linux-musl";
            targetPkgs = pkgs.pkgsCross.musl64;
            crossCc = muslCcX64;
          };
          "aarch64-unknown-linux-musl" = mkPackage {
            target = "aarch64-unknown-linux-musl";
            targetPkgs = pkgs.pkgsCross.aarch64-multiplatform-musl;
            crossCc = muslCcArm64;
          };
        }
        // lib.optionalAttrs pkgs.stdenv.isDarwin {
          "aarch64-apple-darwin" = mkPackage {
            target = "aarch64-apple-darwin";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            baseRust
            pkgs.pkg-config
          ]
          ++ lib.optionals isLinux [
            muslCcX64
            muslCcArm64
          ];

          shellHook =
            if isLinux then
              ''
                echo "🔧 Configured for Linux Musl Cross-Compilation"
                export CC_x86_64_unknown_linux_musl=${muslCcX64}/bin/x86_64-unknown-linux-musl-gcc
                export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=$CC_x86_64_unknown_linux_musl

                export CC_aarch64_unknown_linux_musl=${muslCcArm64}/bin/aarch64-unknown-linux-musl-gcc
                export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=$CC_aarch64_unknown_linux_musl
              ''
            else
              ''
                echo "🍎 Configured for macOS"
              '';
        };
      }
    );
}
