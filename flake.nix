{
  description = "Backend-independent FFXIV data library";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    notist = {
      url = "github:AzurIce/notist";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        crane.follows = "crane";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      crane,
      rust-overlay,
      notist,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
          targets = [ "wasm32-unknown-unknown" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        notistPkg = notist.packages.${system}.default;
        # Keep JSON fixtures, but never include the archived crate in builds.
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            baseNameOf path != "legacy"
            && (craneLib.filterCargoSources path type || pkgs.lib.hasSuffix ".json" path);
        };
        commonArgs = {
          inherit src;
          pname = "icexiv-data";
          version = (fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          strictDeps = true;
        };
        nativeArgs = commonArgs // {
          cargoExtraArgs = "--locked --all-targets --all-features";
        };
        cargoArtifacts = craneLib.buildDepsOnly (nativeArgs // { cargoCheckExtraArgs = ""; });
        wasmArgs = commonArgs // {
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          cargoExtraArgs = "--locked --lib --all-features";
        };
        wasmArtifacts = craneLib.buildDepsOnly (wasmArgs // { doCheck = false; });
        cargoCheck =
          args:
          craneLib.mkCargoDerivation (
            args
            // {
              pnameSuffix = "-check";
              buildPhaseCargoCommand = "cargoWithProfile check ${args.cargoExtraArgs}";
              doCheck = false;
              doInstallCargoArtifacts = false;
            }
          );
        library = craneLib.cargoBuild (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--locked --lib";
            doCheck = false;
          }
        );
      in
      {
        packages = {
          default = library;
        };
        checks = {
          fmt = craneLib.cargoFmt commonArgs;
          nix-fmt = pkgs.runCommand "icexiv-data-nix-fmt" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
            nixfmt --check ${./flake.nix}
            touch $out
          '';
          native = cargoCheck (nativeArgs // { inherit cargoArtifacts; });
          tests = craneLib.cargoTest (nativeArgs // { inherit cargoArtifacts; });
          minimal = cargoCheck (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "--locked --no-default-features";
            }
          );
          wasm = cargoCheck (wasmArgs // { cargoArtifacts = wasmArtifacts; });
        };
        devShells.default = craneLib.devShell {
          packages = [
            notistPkg
            pkgs.git
            pkgs.just
            pkgs.nixfmt
            pkgs.python3
          ];
        };
        formatter = pkgs.writeShellApplication {
          name = "icexiv-data-nixfmt";
          runtimeInputs = [ pkgs.nixfmt ];
          text = ''
            if [ "$#" -eq 0 ]; then
              set -- flake.nix
            fi
            exec nixfmt "$@"
          '';
        };
      }
    );
}
