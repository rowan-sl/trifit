{
  description = "Triangle Art";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem(system:
      let
        fs = nixpkgs.lib.fileset;
        overlays = [ (import rust-overlay ) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        lib = pkgs.lib;
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        # tell crane to use this toolchain
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        # cf. https://crane.dev/API.html#libcleancargosource
        # src = craneLib.cleanCargoSource ./.;
        src = fs.toSource {
          root = ./.;
          fileset = fs.unions [
            (fs.fromSource (craneLib.cleanCargoSource ./.))
          ];
        };
        # compile-time
        nativeBuildInputs = with pkgs; with pkgs.xorg; [
          rustToolchain clang mold-wrapped pkg-config
          libxcb
          libXcursor
          libXrandr
          libXi
          pkg-config
          python3
          libGL
          libGLU
        ];
        # runtime
        buildInputs = with pkgs; [
          clang
          lld
          xorg.libX11
          wayland
          libxkbcommon
        ]; # needed system libraries
        cargoArtifacts = craneLib.buildDepsOnly { inherit src buildInputs nativeBuildInputs; };
        bin = craneLib.buildPackage ({ inherit src buildInputs nativeBuildInputs cargoArtifacts; });
      in
      {
        packages = {
          # so bin can be spacifically built, or just by default
          inherit bin;
          default = bin;
        };
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          name = "trifit";
          nativeBuildInputs = [
            pkgs.rust-analyzer-unwrapped
            pkgs.nodePackages.vscode-langservers-extracted
          ] ++ nativeBuildInputs;
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          shellHook = ''
          export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${lib.makeLibraryPath (with pkgs; [libGL libGLU])}
          if [ -n "$\{EXEC_THIS_SHELL}" ]; then 
            exec $EXEC_THIS_SHELL
          fi
          '';
        };
      }
    );
}
