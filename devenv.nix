{ pkgs, lib, config, inputs, ... }:

{
  languages.rust.enable = true;

  packages = with pkgs; [
    act
    cargo-outdated
    sqld
    turso-cli
  ];
}
