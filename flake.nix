{
  description = "Semantic code search with Swiftide and Qdrant";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        semantic-search = pkgs.rustPlatform.buildRustPackage {
          pname = "semantic-search";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            rustToolchain
          ];

          buildInputs = with pkgs; [
            openssl
          ];

          cargoBuildFlags = [ "--bins" ];
        };

      in
      {
        packages = {
          default = semantic-search;
          semantic-search = semantic-search;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            openssl
            cargo-watch
            # semantic-search
          ];
          # RUST_LOG="swiftide=trace,swiftide_indexing=trace,swiftide_integrations=trace,debug";

          shellHook = ''
          echo "##########################################"
          echo "# INDEXING:                              #"
          echo "# cargo run --bin semantic-indexer <dir> #"
          echo "#                                        #"
          echo "# SEARCH:                                #"
          echo "# cargo run --bin q <search>             #"
          echo "##########################################"

          '';
        };

        nixosModules.default = { config, lib, pkgs, ... }:
          with lib;
          let
            cfg = config.services.semantic-indexer;
          in
          {
            options.services.semantic-indexer = {
              enable = mkEnableOption "semantic code indexer service";

              paths = mkOption {
                type = types.listOf types.str;
                description = "Paths to watch and index";
                example = [ "/home/user/projects" ];
              };

              configFile = mkOption {
                type = types.path;
                description = "Path to config.toml";
              };

              user = mkOption {
                type = types.str;
                default = "semantic-indexer";
                description = "User to run the service as";
              };

              group = mkOption {
                type = types.str;
                default = "semantic-indexer";
                description = "Group to run the service as";
              };
            };

            config = mkIf cfg.enable {
              users.users.${cfg.user} = {
                isSystemUser = true;
                group = cfg.group;
                description = "Semantic indexer service user";
              };

              users.groups.${cfg.group} = {};

              boot.kernel.sysctl = {
                "fs.inotify.max_user_watches" = 524288;
                # You can also add other related limits here if necessary, e.g.:
                "fs.inotify.max_user_instances" = 1024;
              };

              systemd.services.semantic-indexer = {
                description = "Semantic Code Indexer";
                wantedBy = [ "multi-user.target" ];
                after = [ "network.target" ];

                serviceConfig = {
                  Type = "simple";
                  User = cfg.user;
                  Group = cfg.group;
                  ExecStart = "${semantic-search}/bin/semantic-indexer --config ${cfg.configFile} ${concatStringsSep " " cfg.paths}";
                  Restart = "always";
                  RestartSec = "10s";
                };
              };

              environment.systemPackages = [ semantic-search ];
            };
          };
      }
    );
}
