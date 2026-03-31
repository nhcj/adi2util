{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nixpkgs, rust-overlay, ... }: let
    platforms = [ "x86_64-linux" ];
    forAllPlatforms = f: nixpkgs.lib.genAttrs platforms (system: f (import nixpkgs {
      inherit system;
      overlays = [ rust-overlay.overlays.default ];
    }));
  in {
    devShells = forAllPlatforms (pkgs: {
      default = pkgs.mkShell {
        buildInputs = with pkgs; [
          alsa-lib
          pkg-config
          rust-bin.stable.latest.complete
        ];
      };
    });
  };
}
