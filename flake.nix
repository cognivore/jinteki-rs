{
  description = "jinteki-rs — Netrunner backend in Rust, final tagless, jnet wire-compatible";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      # The deployable artifact: the server binary with the UI beside it.
      packages = forAll (pkgs: {
        default = pkgs.callPackage ./nix/package.nix {
          rev = self.shortRev or self.dirtyShortRev or "dev";
        };
        jinteki-rs = pkgs.callPackage ./nix/package.nix {
          rev = self.shortRev or self.dirtyShortRev or "dev";
        };
      });

      # The headline output for a vacationvm fleet (see nix/vacationvm-module.nix).
      nixosModules.default = import ./nix/vacationvm-module.nix self;

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            # Rust toolchain — the only blessed way to build this repo.
            rustc
            cargo
            clippy
            rustfmt
            rust-analyzer
            rust-script # tools/fetch-carddata.rs (card data actualiser)
            # Reference-server oracle tooling (docker-compose against colima VM).
            colima
            docker-client
            docker-compose
            # Misc.
            curl
            jq
          ] ++ nixpkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
          shellHook = ''
            export JINTEKI_RS_NIX_SHELL=1
          '';
        };
      });
    };
}
