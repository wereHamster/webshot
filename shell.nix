let
  pkgs = import <nixpkgs> {};

in pkgs.mkShell {
  buildInputs = [
    pkgs.deno
    pkgs.google-cloud-sdk
  ];
}
