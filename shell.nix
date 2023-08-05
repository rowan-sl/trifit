{ pkgs ? import <nixpkgs> { overlays = [ (import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz)) ]; } }:

with pkgs;

mkShell {
  name = "winit-project";
  nativeBuildInputs = with xorg; [
    libxcb
    libXcursor
    libXrandr
    libXi
    pkg-config
  ] ++ [
    python3
    libGL
    libGLU
  ];
  buildInputs = [
    clang
    lld
    #latest.rustChannels.stable.rust
    xorg.libX11
    wayland
    libxkbcommon
  ];

  shellHook = ''
      export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${lib.makeLibraryPath ([libGL libGLU])}
  '';
}
