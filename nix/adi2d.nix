{ lib, rustPlatform, pkg-config, alsa-lib }:

rustPlatform.buildRustPackage {
  pname = "adi2d";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [ ../Cargo.toml ../Cargo.lock ../crates ];
  };

  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "--package" "adi2d" ];
  cargoTestFlags = [ "--package" "adi2d" ];

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ alsa-lib ];

  meta = {
    mainProgram = "adi2d";
    platforms = lib.platforms.linux;
  };
}
