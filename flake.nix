{
  description = "t - a text processing language and utility";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    crane,
    rust-overlay,
    ...
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
    forAllSystems = fn: nixpkgs.lib.genAttrs systems (system: fn system);
    pkgsFor = system:
      import nixpkgs {
        inherit system;
        overlays = [rust-overlay.overlays.default];
      };
    craneLibFor = system: let
      pkgs = pkgsFor system;
      toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    in
      (crane.mkLib pkgs).overrideToolchain toolchain;
    src = system: let
      craneLib = craneLibFor system;
    in
      craneLib.cleanCargoSource ./.;
    commonArgs = system: let
      craneLib = craneLibFor system;
    in {
      src = src system;
      strictDeps = true;
    };
    cargoArtifactsFor = system: let
      craneLib = craneLibFor system;
    in
      craneLib.buildDepsOnly (commonArgs system);
  in {
    packages = forAllSystems (
      system: let
        craneLib = craneLibFor system;
        args =
          commonArgs system
          // {
            cargoArtifacts = cargoArtifactsFor system;
          };
      in {
        t = craneLib.buildPackage args;
        default = craneLib.buildPackage args;
      }
    );

    checks = forAllSystems (
      system: let
        craneLib = craneLibFor system;
        cargoArtifacts = cargoArtifactsFor system;
        args = commonArgs system // {inherit cargoArtifacts;};
      in {
        t-test = craneLib.cargoTest args;
        t-clippy = craneLib.cargoClippy (args // {cargoClippyExtraArgs = "--all-targets -- -D warnings";});
        t-fmt = craneLib.cargoFmt {src = src system;};
      }
    );

    devShells = forAllSystems (
      system: let
        pkgs = pkgsFor system;
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in {
        default = pkgs.mkShell {
          nativeBuildInputs = [
            toolchain
            pkgs.cargo-watch
          ];
        };
      }
    );
  };
}
