{
  description = "AI Agent System";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo
            rust-analyzer
            pkg-config
            openssl
            postgresql
            ollama
          ];
        };

        packages = {
          indexing = pkgs.rustPlatform.buildRustPackage {
            pname = "ai-agent-indexing";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            # TODO: Build configuration
          };

          orchestrator = pkgs.rustPlatform.buildRustPackage {
            pname = "ai-agent-orchestrator";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            # TODO: Build configuration
          };

          cli = pkgs.rustPlatform.buildRustPackage {
            pname = "ai-agent-cli";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            # TODO: Build configuration
          };
        };

        nixosModules.default = { config, lib, pkgs, ... }: {
          # TODO: NixOS module configuration
        };
      }
    );
}
