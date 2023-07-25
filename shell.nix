{ pkgs ? import <nixpkgs> { } }:
with pkgs;
stdenvNoCC.mkDerivation {
  name = "dev-shell";
  buildInputs = [ cargo-edit cargo-readme rustup llvmPackages_latest.clang ];
}
