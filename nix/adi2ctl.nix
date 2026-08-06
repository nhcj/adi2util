{ lib, rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "adi2ctl";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [ ../Cargo.toml ../Cargo.lock ../crates ];
  };

  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "--package" "adi2ctl" ];
  cargoTestFlags = [ "--package" "adi2ctl" ];

  meta = {
    mainProgram = "adi2ctl";
    platforms = lib.platforms.linux;
  };
}
