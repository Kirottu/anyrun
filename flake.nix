{
  description = "A wayland native, highly customizable runner.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux"];

      perSystem = {
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }: let
        inherit (inputs.nixpkgs) lib;
        inherit (lib) getExe;
      in {
        # provide the formatter for nix fmt
        formatter = pkgs.alejandra;

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self'.packages;

          packages = with pkgs; [
            alejandra # nix formatter
            rustfmt # rust formatter
            statix # lints and suggestions
            deadnix # clean up unused nix code
            rustc # rust compiler
            gcc
            cargo # rust package manager
            clippy # opinionated rust formatter
          ];
        };

        packages = let
          lockFile = ./Cargo.lock;
        in rec {
          anyrun = pkgs.callPackage ./nix/default.nix {inherit inputs lockFile;};
          # alias nix build .# to anyrun
          default = anyrun;

          anyrun-with-all-plugins = pkgs.callPackage ./nix/default.nix {
            inherit inputs lockFile;
            dontBuildPlugins = false;
          };

          # expose each plugin as a package
          applications = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "applications";
          };

          dictionary = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "dictionary";
          };

          kidex = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "kidex";
          };

          randr = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "randr";
          };

          rink = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "rink";
          };

          shell = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "shell";
          };

          stdin = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "stdin";
          };

          symbols = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "symbols";
          };

          translate = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "translate";
          };

          websearch = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "websearch";
          };

          nix-run = pkgs.callPackage ./nix/plugins/default.nix {
            inherit inputs lockFile;
            name = "nix-run";
          };

        };
      };

      flake = _: rec {
        nixosModules.home-manager = homeManagerModules.default;

        homeManagerModules = rec {
          anyrun = import ./nix/hm-module.nix inputs.self;
          default = anyrun;
        };
      };
    };
}
