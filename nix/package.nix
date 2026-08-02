{ lib, rustPlatform, rev ? "dev" }:

rustPlatform.buildRustPackage {
  pname = "jinteki-rs";
  version = "0.1.0";

  src = lib.cleanSourceWith {
    src = lib.cleanSource ./..;
    # Keep the closure lean: sources, card data, the UI, nothing else.
    filter =
      path: type:
      let
        rel = lib.removePrefix (toString ./.. + "/") (toString path);
        top = lib.head (lib.splitString "/" rel);
      in
      builtins.elem top [
        "Cargo.toml"
        "Cargo.lock"
        "crates"
        "ui"
      ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  # The build id travels IN the binary (compile-time env; the server exposes
  # it at GET /version and the UI displays what the server reports). No
  # artifact text-mangling: the UI ships verbatim. Response caching is
  # handled where it belongs — the deploy's Caddy serves no-store, because
  # nix-store mtimes are 1970 and Last-Modified revalidation is a trap.
  env.JINTEKI_BUILD_REV = rev;

  # The UI is data, not a build artifact: ship it next to the binary and point
  # the service at it with JINTEKI_UI_DIR.
  postInstall = ''
    mkdir -p $out/share/jinteki-rs
    cp -r ui $out/share/jinteki-rs/ui
  '';

  doCheck = false; # the engine suite runs in CI/dev, not on the deploy path

  meta = {
    description = "jinteki-rs — Netrunner engine + mobile client (local play vs bot, jnet bridge)";
    mainProgram = "jinteki-server";
    license = lib.licenses.wtfpl;
    platforms = lib.platforms.unix;
  };
}
