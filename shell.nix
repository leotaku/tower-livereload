{ pkgs ? import <nixpkgs> { } }:
with pkgs;
stdenvNoCC.mkDerivation {
  name = "dev-shell";
  buildInputs = [ rustup llvmPackages_latest.clang ];
}
