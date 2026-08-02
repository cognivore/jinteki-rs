# The vacationvm integration for jinteki-rs.
#
# Adding it to a fleet is two lines in the hive:
#
#     imports = [ jinteki-rs.nixosModules.default ];
#     vacationvm.services.jinteki-rs = { enable = true; subdomain = "netrunner"; };
#
# Everything run-time (package, exec, socket, UI dir) is an `mkDefault` here;
# the operator decides enablement and the public subdomain. Merely importing
# the module is inert.
#
# The daemon serves HTTP over a Unix socket when JINTEKI_SOCKET is set, which
# is exactly the annexwyrm-style listen mode the framework prefers: Caddy
# proxies to the socket and nothing binds a public port.

self:
{ lib, pkgs, ... }:

let
  inherit (lib) mkDefault;
  pkg = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
in
{
  config.vacationvm.services.jinteki-rs = {
    package = mkDefault pkg;
    description = mkDefault "jinteki-rs — Netrunner engine + mobile client";
    exec = mkDefault [ "${pkg}/bin/jinteki-server" ];
    environment = {
      JINTEKI_SOCKET = mkDefault "/run/vacationvm-jinteki-rs/sock";
      JINTEKI_UI_DIR = mkDefault "${pkg}/share/jinteki-rs/ui";
    };
  };
}
