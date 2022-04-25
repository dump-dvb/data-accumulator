{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    naersk = {
      url = "github:nix-community/naersk";
    };

    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, naersk, utils, ... }: 
   utils.lib.eachDefaultSystem (system: let
     pkgs = nixpkgs.legacyPackages.${system};
    
     package = pkgs.callPackage ./derivation.nix { 
        naersk = naersk.lib.${system};
     };
      in rec {
        checks = packages;
        packages.data-accumulator = package;
        overlay = (final: prev: {
          data-accumulator = package;
        });
      }
    );
}
