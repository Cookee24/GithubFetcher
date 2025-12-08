{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forEachSystem = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forEachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          lib = pkgs.lib;

          mkPackage =
            {
              cross ? null,
            }:
            let
              lib = pkgs.lib;

              crossAttrs = lib.optionalAttrs (cross != null) (
                let
                  cc = cross.cc;
                  target = cross.target;
                in
                {
                  "CC_${builtins.replaceStrings [ "-" ] [ "_" ] target}" = "${cc}/bin/${target}-gcc";
                  "CARGO_TARGET_${builtins.replaceStrings [ "-" ] [ "_" ] (lib.toUpper target)}_LINKER" =
                    "${cc}/bin/${target}-gcc";
                  "AR_${builtins.replaceStrings [ "-" ] [ "_" ] target}" = "${cc.bintools}/bin/${target}-ar";
                  CARGO_BUILD_TARGET = target;

                  rustTarget = lib.optional (target != null) target;
                  buildInputs = lib.optional (cc != null) [ cc ];
                }
              );
            in
            pkgs.rustPlatform.buildRustPackage (
              {
                pname = "github-fetcher-mcp";
                version = "0.1.3";
                src = ./.;
                cargoLock.lockFile = ./Cargo.lock;
              }
              // crossAttrs
            );
        in
        {
          default = mkPackage { };
        }
        // lib.optionalAttrs (pkgs.stdenv.isLinux) {
          "x86_64-unknown-linux-musl" = mkPackage {
            cross = {
              target = "x86_64-unknown-linux-musl";
              cc = pkgs.pkgsCross.musl64.stdenv.cc;
            };
          };
          "aarch64-unknown-linux-musl" = mkPackage {
            cross = {
              target = "aarch64-unknown-linux-musl";
              cc = pkgs.pkgsCross.aarch64-multiplatform-musl.stdenv.cc;
            };
          };
        }
        // lib.optionalAttrs pkgs.stdenv.isDarwin {
          "aarch64-apple-darwin" = mkPackage { };
        }
      );

      devShells = forEachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              pkg-config
              rustc
              cargo
            ];

            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
        }
      );
    };
}
