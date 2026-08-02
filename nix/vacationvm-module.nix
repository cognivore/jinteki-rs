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
{ lib, pkgs, config, ... }:

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
      # Accounts/decks SQLite lives in the framework-provisioned state dir
      # (systemd StateDirectory; also the unit's HOME).
      JINTEKI_DATA_DIR = mkDefault config.vacationvm.services.jinteki-rs.stateDir;
      # Behind Caddy the app sees X-Forwarded-Proto: https and marks its
      # session cookie Secure on its own; this pin makes it unconditional.
      JINTEKI_SECURE_COOKIES = mkDefault "1";
      # Magic links are absolute, so the app must know its public origin.
      # The operator overrides this when the subdomain differs.
      APP_URL = mkDefault "https://netrunner.sweater.vac.fere.me";
      # Mail sender identity (see ACCOUNTS-AND-DECKS.md OI-1: FROM domain
      # should align with a warmed, domain-authenticated SendGrid sender;
      # a mismatched FROM/link domain is what spam-foldered draftroom's
      # first mails). Operator's call at deploy time.
      FROM_NAME = mkDefault "jinteki-rs";
      # No SENDGRID_API_KEY default: without it the server runs in dev mode
      # and logs magic links to the journal. The operator enables real mail
      # with, e.g.:
      #   environmentSecrets.SENDGRID_API_KEY = "jinteki-rs-sendgrid-key";
      #   environment.FROM_EMAIL = "noreply@example.org";
    };
  };
}
