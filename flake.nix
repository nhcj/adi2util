{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nixpkgs, rust-overlay, ... }: let
    platforms = [ "x86_64-linux" "aarch64-darwin" ];
    forAllPlatforms = f: nixpkgs.lib.genAttrs platforms (system: f (import nixpkgs {
      inherit system;
      overlays = [ rust-overlay.overlays.default ];
    }));
  in {
    packages = forAllPlatforms (pkgs:
      let
        rustPlatform = pkgs.makeRustPlatform {
          cargo = pkgs.rust-bin.stable.latest.minimal;
          rustc = pkgs.rust-bin.stable.latest.minimal;
        };
      in {
        adi2d = pkgs.callPackage ./nix/adi2d.nix { inherit rustPlatform; };
        adi2ctl = pkgs.callPackage ./nix/adi2ctl.nix { inherit rustPlatform; };
      });

    devShells = forAllPlatforms (pkgs: {
      default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rust-bin.stable.latest.complete
        ] ++ lib.optionals stdenv.hostPlatform.isLinux [
          alsa-lib
          pkg-config
        ];
      };
    });
  };
}
