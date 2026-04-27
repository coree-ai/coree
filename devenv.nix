{ pkgs, lib, config, inputs, ... }:

{
  dotenv.enable = true;

  languages.rust.enable = true;

  env.COREE_BINARY_OVERRIDE = "${config.devenv.root}/target/release/coree";
  env.COREE_CHANNEL = "dev";
  env.RUST_LOG = "coree=debug";

  packages = with pkgs; [
    act
    cargo-bloat
    cargo-outdated
    gh
    openssl
    pkg-config
    python3
    sqld
    sqlite
    turso-cli
    upx
  ];
}
