{ pkgs, lib, config, inputs, ... }:

{
  dotenv.enable = true;

  languages.rust.enable = true;

  env.MEMSO_BINARY_OVERRIDE = "${config.devenv.root}/target/release/memso";

  packages = with pkgs; [
    act
    cargo-outdated
    gh
    sqld
    turso-cli
    upx
  ];
}
