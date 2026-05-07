{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer

    pkg-config
    protobuf
    rust-cbindgen
  ];

  buildInputs = with pkgs; [
    libiconv
  ];

  RUST_BACKTRACE = "1";
  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
}
