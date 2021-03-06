{
  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/nixos-unstable;

    naersk = {
      url = github:nix-community/naersk;
    };

    utils = {
      url = github:numtide/flake-utils;
    };

    stops = {
      url = github:dump-dvb/stop-names;
      flake = false;
    };
  };

  outputs = { self, nixpkgs, naersk, utils, stops, ... }:
    utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          package = pkgs.callPackage ./derivation.nix {
            naersk = naersk.lib.${system};
            stops = stops;
          };

        in
        rec {
          checks = packages;
          packages = {
            data-accumulator = package;
            default = package;
          };

          devShells = pkgs.mkShell {
            buildInputs = with pkgs; [
              pkg-config
              cmake
              zlib
              llvmPackages.bintools
              postgresql
            ];

            shellHook = ''
            '';
          };
        }
      ) // {
      overlays.default = final: prev: {
        inherit (self.packages.${prev.system})
          data-accumulator;
      };
      hydraJobs =
        let
          hydraSystems = [
            "x86_64-linux"
            "aarch64-linux"
          ];
        in
        builtins.foldl'
          (hydraJobs: system:
            builtins.foldl'
              (hydraJobs: pkgName:
                nixpkgs.lib.recursiveUpdate hydraJobs {
                  ${pkgName}.${system} = self.packages.${system}.${pkgName};
                }
              )
              hydraJobs
              (builtins.attrNames self.packages.${system})
          )
          { }
          hydraSystems;
    };
}
