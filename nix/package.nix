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

  # The UI is data, not a build artifact: ship it next to the binary and point
  # the service at it with JINTEKI_UI_DIR.
  postInstall = ''
    mkdir -p $out/share/jinteki-rs
    cp -r ui $out/share/jinteki-rs/ui
    # Stamp the build id: visible in the UI and cache-busting the assets
    # (nix-store mtimes are 1970, so Last-Modified revalidation is useless —
    # the URL must change per build). Handles both HTML generations: with
    # __BUILD__ markers (substitute) and without (inject).
    ix=$out/share/jinteki-rs/ui/index.html
    if grep -q "__BUILD__" "$ix"; then
      substituteInPlace "$ix" --replace-quiet "__BUILD__" "${rev}"
    else
      sed -i \
        -e 's|href="style.css"|href="style.css?v=${rev}"|' \
        -e 's|src="app.js"|src="app.js?v=${rev}"|' \
        -e 's|Long-press any card to read it\.|\0 · build ${rev}|' \
        "$ix"
    fi
  '';

  doCheck = false; # the engine suite runs in CI/dev, not on the deploy path

  meta = {
    description = "jinteki-rs — Netrunner engine + mobile client (local play vs bot, jnet bridge)";
    mainProgram = "jinteki-server";
    license = lib.licenses.wtfpl;
    platforms = lib.platforms.unix;
  };
}
