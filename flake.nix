{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    systems.url = "github:nix-systems/default";
  };
  outputs = { self, systems, nixpkgs, ... }@inputs:
    let
      eachSystem = f:
        nixpkgs.lib.genAttrs (import systems) (system:
          f (import nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          }));
    in {
      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs;
            [
              # Complete Rust toolchain with cargo, rustc, etc.
              (rust-bin.stable.latest.default.override {
                extensions =
                  [ "rust-analyzer" "clippy" "rustfmt" "rust-src" "rust-docs" ];
                targets = [ "x86_64-unknown-linux-musl" ];
              })
              # Or alternatively, you can use the complete toolchain:
              # (rust-bin.stable.latest.complete)
              # (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
              # clang
              # Use mold when we are running in Linux
              # (pkgs.lib.optionals pkgs.stdenv.isLinux pkgs.mold)
            ];
          RUST_SRC_PATH =
            "${pkgs.rust-bin.stable.latest.rust-src}/lib/rustlib/src/rust/library";
        };
      });
    };
}
