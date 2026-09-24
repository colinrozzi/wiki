{
  description = "wiki — a minimal two-actor (acceptor + handler) HTTP server on Theater";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";

    theater = {
      # in-module-state + packr-0.24 host. Keep this rev in lockstep with the
      # theater-guest rev in acceptor/Cargo.toml so the actor's interface hashes
      # match the runtime it spawns on.
      url = "github:colinrozzi/theater/00b0bf93fe69a231463d3ba918fa435c5f2a517d";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.crane.follows = "crane";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, theater }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (pkgs.lib.hasSuffix ".rs" path) ||
            (pkgs.lib.hasSuffix ".toml" path) ||
            (pkgs.lib.hasSuffix ".lock" path) ||
            (pkgs.lib.hasSuffix ".html" path) ||   # baked handler content (wiki.html)
            (type == "directory");
        };

        commonArgs = {
          inherit src;
          pname = "wiki";
          version = "0.1.0";
          cargoExtraArgs = "--target wasm32-unknown-unknown";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          doCheck = false;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      in {
        # nix build 'path:.' -> both wasm artifacts in ./result
        packages.default = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          installPhaseCommand = ''
            mkdir -p $out
            cp target/wasm32-unknown-unknown/release/wiki_acceptor.wasm $out/
            cp target/wasm32-unknown-unknown/release/wiki_handler.wasm $out/
          '';
        });

        devShells.default = craneLib.devShell {
          packages = [ rustToolchain theater.packages.${system}.default ];
        };
      });
}
