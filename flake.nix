{
  description = "jinteki-rs — Netrunner backend in Rust, final tagless, jnet wire-compatible";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            # Rust toolchain — the only blessed way to build this repo.
            rustc
            cargo
            clippy
            rustfmt
            rust-analyzer
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
