{ pkgs, lib, config, inputs, ... }:

{
  languages.rust.enable = true;

  packages = with pkgs; [
    cargo-outdated
    sqld
    turso-cli
  ];
}
