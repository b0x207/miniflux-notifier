{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    lorem-rss.url = "github:b0x207/lorem-rss";
    treefmt-nix.url = "github:numtide/treefmt-nix";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        {
          config,
          pkgs,
          system,
          ...
        }:
        let
          naersk' = pkgs.callPackage inputs.naersk { };
          treefmtEval = inputs.treefmt-nix.lib.evalModule pkgs ./treefmt.nix;
        in
        {
          packages.default = naersk'.buildPackage {
            src = ./.;
          };

          formatter = treefmtEval.config.build.wrapper;

          devShells.default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              rustc
              cargo
              rust-analyzer
              clippy
              cargo-audit
              inputs.lorem-rss.packages.${system}.default
            ];
          };

          checks = {
            package = config.packages.default;

            formatter = treefmtEval.config.build.check inputs.self;

            clippy = naersk'.buildPackage {
              src = ./.;
              mode = "clippy";
            };

            cargo-audit = pkgs.stdenv.mkDerivation {
              name = "cargo-audit-check";
              src = ./.;

              nativeBuildInputs = with pkgs; [
                cargo
                cargo-audit
                cacert
              ];

              buildPhase = ''
                export HOME=$(mktemp -d)

                cargo audit --no-yanked --no-fetch --db ${inputs.advisory-db}
              '';

              installPhase = "touch $out";
            };
          };
        };
    };
}
