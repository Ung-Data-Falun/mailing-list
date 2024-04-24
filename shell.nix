{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell rec {
    buildInputs = with pkgs; [
      rustup
    ];
    RUSTC_VERSION = "stable"; 
    shellHook = ''
      rustup default $RUSTC_VERSION
      rustup component add rust-analyzer
      export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
      export PATH=$PATH:''${RUSTUP_HOME:-~/.rustup}/toolchains/nightly-x86_64-unknown-linux-gnu/bin/
    '';
}
