# ACCOUNTS-AND-DECKS — identity, decklists, and NetrunnerDB import

**Subsystem of Interest:** session-based identity upgradeable to an email account (magic links, no passwords ever), a personal + public decklist library, and NetrunnerDB import — for the jinteki-rs native server and its vanilla-JS client.

| | |
|---|---|
| Version | 0.1.0 — DRAFT FOR RATIFICATION |
| Date | 2026-08-02 |
| Status | Specification only. No code in this change. |
| Parent document | `DESIGN.md` (SYS-* format, sans-IO mandate §A.5, ICD Appendix B) |
| Reference implementations studied | `~/Github/draftroom` (Go, magic-link auth + JSONL logs), `~/Github/north-london-cube-community` ("Cubehall", TS + SQLite, the original magic-link flow), `~/Github/jinteki/jinteki-reference` @ pin `4054730` (NRDB import, deck validator) |
| Deployment target | vacationvm on NixOS, Unix socket behind Caddy, `https://netrunner.sweater.vac.fere.me` |

**Scope note.** This subsystem is the *native-mode* account system for the jinteki-rs UI (`ui/`). It is deliberately NOT the jnet-compat auth of DESIGN.md SYS-I-7 (bcrypt + HS512 JWT against the reference `users` collection) — that contract stays untouched for the drop-in story. The two identity systems coexist: compat mode authenticates jnet users against Mongo when that phase lands; native mode authenticates people who walked in from a phone with no account at all. Nothing here modifies the engine core; every I/O-touching piece lives in `jinteki-server` per the sans-IO mandate (DESIGN.md SYS-Q-1, Appendix A.5).

---

## 1. What the reference projects actually do (study findings)

The task is to follow patterns *proven* in two sibling projects. Here is precisely what they prove, with citations, and where they differ.

### 1.1 draftroom (Go) — the flow to copy

The magic-link core, `draftroom/auth.go:20-34`, documents itself as "modelled on the cube.london (Cubehall) flow": `register(email, name)` → pending user + emailed challenge; `login(email)` → challenge stored **separately** from the user + emailed; `verify(user, token)` → single-use consume, issue a 14-day session.

**Tokens.** All tokens — user ids, session ids, magic-link challenges, share links — come from one constructor: 16 bytes of `crypto/rand`, base64url, i.e. 128-bit entropy (`draftroom/store.go:63-69`). Compared with constant-time equality for the registration path (`auth.go:16-18,297`). Stored **in plaintext** in the append-only log (weakness; see §5.3).

**Sessions.** Cookie `dr_session` (`auth.go:40`), TTL 14 days (`sessionTTL`, `auth.go:38`), sliding-window refresh at most once per hour (`touchInterval`, `auth.go:39`, `ValidateSession` `auth.go:353-395`). Cookie attributes: `HttpOnly`, `SameSite=Lax`, `Secure` when behind TLS (`main.go:262-272`). No CSRF tokens — Lax + JSON-body POSTs is the whole story.

**Challenges.** TTL 30 minutes (`challengeTTL`, `auth.go:37`). Single-use with a subtle, correct detail: an expired login token is *burned on presentation anyway* so it can never be retried (`auth.go:305-318`). A returning user's login challenge lives in its own log op, never in the user record — the comment at `auth.go:26-32` cites the Cubehall outage that produced this rule (a pending-account GC deleted a real user mid-login).

**Delivery.** SendGrid HTTP API v3 (`email.go:112`), configured by env `SENDGRID_API_KEY`, `FROM_EMAIL`, `FROM_NAME`, `APP_URL` (`email.go:31-37`). Unset key = dev mode: the link is logged, never sent (`email.go:60-63`). Link shape: `{APP_URL}/auth/verify?userId=…&token=…` (`email.go:44-47`); verify is a GET that sets the cookie and redirects, with the `next` param clamped to local paths to kill open-redirect (`main.go:420-424`). Email copy: "Click below to sign in… This link expires in 30 minutes. If you didn't request this, ignore this email."

**Abuse guard** (`guard.go`, ported line-for-line from Cubehall's `auth-guard.ts`):
1. Email send limits — per-address cooldown 60 s, daily cap 5, **global hourly budget 80** as a circuit breaker on sender reputation (`guard.go:32-34,328-356`). A suppressed send is logged and dropped; the HTTP response *does not change* (`email.go:50-56`).
2. Per-IP exponential backoff on auth POSTs — 3 free hits per 10-minute window, then 2 s, 8 s, 32 s … capped at 5 min (`guard.go:267-303`).
3. Proof-of-work hashcash CAPTCHA — sha256(salt:nonce) with ≥16 leading zero bits (20 for VPN/datacenter IPs), 10-minute TTL, single attempt per challenge (`guard.go:152-249`); solved in the browser (`web/pow.js`, driven by `web/auth.js:42-50`).
4. VPN/datacenter classification — soft; only raises PoW difficulty; dormant when the ranges file is absent (`guard.go:88-149`).

**Enumeration safety.** Register-on-taken-email and login-on-unknown-email return the *same* `{"sent": true}` shape as success (`main.go:365-368,389-392`). Cubehall gets this wrong (below); draftroom is the corrected version.

**Anonymous access.** This is the part that does NOT transfer directly: draftroom has no anonymous *account*. Everything requires a session except share-token surfaces (`main.go:164-166`). Guests exist only inside a shared room: a `dr_guest` cookie (1-year, random token, "It is not authentication" — `main.go:529-551`) lets an anonymous reviewer delete their own comments (`UserID: "guest:"+id`, `main.go:658-666`). **There is no adoption path**: comments written as `guest:X` stay guest comments forever, even if that person later registers. jinteki-rs needs real adoption; we design it in §4 rather than copying an absence.

**Storage.** Append-only JSONL op logs folded into state, `flock`-guarded, multi-process-safe (`authlog.go:19-32,247-286`; same pattern in `store.go`, `link.go`). Deterministic merge rules per op type (earliest-wins contested email, monotonic verify, tombstoned sessions — `authlog.go:76-184`). Audit is the log itself: nothing is ever rewritten, deleted comments' text remains in history (`store.go:527-529`).

### 1.2 Cubehall / north-london-cube-community (TS) — the original, and its scars

Same flow, earlier draft, different storage: SQLite (sql.js WASM) via Kysely-ish repos, Effect-TS programs (`packages/server/src/programs/auth.ts`). Differences that matter:

- **Tokens are `rng.uuid()`** — UUIDv4, 122-bit; fine but weaker ergonomics than draftroom's 128-bit base64url, and also plaintext at rest (`auth.ts:60-61`, `db/sqlite.ts:159-165`).
- **Enumeration is leaky by design**: register returns 409 `EMAIL_TAKEN`, login returns 404 `USER_NOT_FOUND` (`http/routes/auth.ts:92,137-140`). Acceptable for a ~30-person club, wrong for a public game server. Draftroom fixed it; we follow draftroom.
- **The scar**: `login_challenges` is a separate table with a comment explaining why — "otherwise the pending-user GC deletes verified accounts mid-login. (This conflation is exactly the bug that deleted the coordinator account on 2026-06-11.)" (`programs/auth.ts:233-237`, `db/sqlite.ts:154-165`). Load-bearing lesson: **ephemeral challenge state never lives in the durable account row, and no GC may key off a field that a login mutates.**
- **Verify is a POST** (`routes/auth.ts:99-124`) driven by a client route that lifts `userId`/`token` from the URL; the challenge token must never appear in an API response outside TEST_MODE, or a scripted register+verify loop bypasses email entirely (`routes/auth.ts:79-85`).
- **`merged` account state + `user_merges` audit table** — an admin can fold account A into B; A's rows are reassigned, A becomes `{kind:"merged", mergedInto}`, refuses magic-link login, and the merge is recorded with before-values so it can be reverted (`core/src/model/user.ts:16-26`, `db/sqlite.ts:205-217`, `programs/auth.ts:219-224`). This is the exact machinery anonymous→claimed adoption needs, proven here in admin form.
- Session semantics identical to draftroom (14 d, 1 h sliding touch, cookie `session` HttpOnly/Secure/Lax — `routes/auth.ts:108-114`, `programs/auth.ts:288-344`).

### 1.3 jinteki-rs today — what we are extending

- **No database.** Local games live in an in-memory `HashMap<String, Arc<Mutex<LocalGame>>>` keyed by an opaque token (`crates/jinteki-server/src/local.rs:35-43`), token = 2×`u64` random hex = 128-bit (`local.rs:121-125`), pruned after 72 h idle (`local.rs:43-60`). The client stores `{token, side}` in `localStorage["jinteki_local"]` and resumes over any fresh WebSocket (`ui/app.js:72,144-157`). A server restart loses every game. Accounts and decks must NOT inherit that property.
- **Wire:** one WebSocket per session, JSON text frames with a `type` field (`local.rs:6-14`); HTTP surface is just `/ws/local`, `/ws/bridge`, `/health`, and static `ui/` fallback (`main.rs:27-31`).
- **Strict mode:** game start refuses any deck containing a card whose behavior is not natively implemented, listing the offenders (`local.rs:85-109`) — driven by `printed::impl_status` (`crates/jinteki-core/src/printed.rs:130-138`), which classifies every title as `Behavior` / `JnetOnly` / `Unimplemented`.
- **Card DB:** 2,065 titles, keyed by title, each with its latest NRDB `code` (`crates/jinteki-core/carddata/cards.json`, `printed.rs:17-42`). **Gap:** `PrintedCard` carries no `factioncost` (influence pips), no `influencelimit`, no `minimumdecksize`, no format-legality map, and no previous-printing codes — all present in the source EDN (`tools/raw_data.edn` has `:factioncost`, `:influencelimit`, `:minimumdecksize`, `:format`, `:previous-versions`, `:normalizedtitle`) but dropped by `tools/gen-carddata.py`. Deck validation and import need them; §6.1 makes extending the generator a prerequisite.
- **Deployment:** Unix socket when `JINTEKI_SOCKET` is set (`main.rs:34-46`), vacationvm module supplies `JINTEKI_SOCKET`/`JINTEKI_UI_DIR` (`nix/vacationvm-module.nix:28-31`). Secrets pattern from the fleet: `environmentSecrets.<VAR> = "<agenix-name>"` (`vacationvm-deploy/services/propscope.nix:29-33`); draftroom's mail wiring including the shared-SendGrid-account rationale and the FROM-domain spam lesson at `vacationvm-deploy/services/draftroom.nix:22-63`.
- **UX law:** mobile-first, one file, no framework, no build step (`ui/app.js`); one decision on screen at a time, big tap targets, legality shown not discovered (`docs/UX.md`).

### 1.4 Reference NRDB import (jinteki.net) — the parser to port

`jinteki-reference/src/clj/web/nrdb.clj`:
- Base URL `https://netrunnerdb.com/api/2.0/public/` with endpoints `decklist/{id}` (public) and `deck/{id}` (private-but-published) (`nrdb.clj:9-14`).
- Input parsing accepts a bare id or any NRDB URL; the id is whatever follows `decklist/`, `deck/view/`, or `deck/`, truncated at the next `/` (`nrdb.clj:15-27`). URL containing `/decklist/` → public endpoint, `/deck/` → private, otherwise try public then fall back to private (`nrdb.clj:65-85`).
- Response contract: `{success: true, total: 1, data: [{id, name, cards: {code: qty, …}}]}`; anything else is logged and rejected (`nrdb.clj:57-63`).
- Card lookup by `code`, **falling back to `previous-versions.code`** so decks referencing old printings still resolve (`nrdb.clj:29-31`). Identity is split out of the card map by type (`nrdb.clj:33-40`). Unknown codes are silently dropped (`nrdb.clj:40` — the `m` fallthrough); we do better in §7.4.
- Provenance note written into the deck: `"imported from https://netrunnerdb.com/en/decklist/{id}"` (`nrdb.clj:52-55`).
- The WS handler `:decks/import` wraps this, requires `{:name :identity :cards}` all present, stamps `format "standard"`, validates, saves (`web/decks.clj:105-125`).
- Card *data* (as opposed to decklists) comes from the `netrunner-data` EDN blob, NOT the NRDB API (`tasks/nrdb.clj:23-37`); images from `https://card-images.netrunnerdb.com/v2/large/{code}.jpg` throttled to 5 req/s (`tasks/nrdb.clj:21-22,66-67`). jinteki-rs already follows both conventions (`tools/gen-carddata.py`, `ui/app.js:523-525`).

**Live API verification (2026-08-02).** Probed directly while writing this spec:
- v2 `GET https://netrunnerdb.com/api/2.0/public/decklist/81579` → 200, shape exactly as above; `cards` keyed by 5-digit code strings. The same endpoint **also accepts the modern UUID ids** (`…/decklist/45c2efd4-62e3-4e44-aa27-a49a3a7f6368` → 200).
- v3 lives on a different host: `GET https://api.netrunnerdb.com/api/v3/public/decklists/{uuid}` → JSON:API document; `attributes.card_slots` is keyed by **snake_case title slugs** (`sure_gamble: 3`), not codes, and `identity_card_id` is a slug too.
- NRDB fronts `netrunnerdb.com` with bot protection (a default-UA fetch of the v2 API returned 403 during testing; a browser UA succeeded). The importer must send an honest, stable `User-Agent` and treat 403 as a distinct, user-explainable failure.
- Decklist page URLs in the wild: legacy `netrunnerdb.com/en/decklist/81579/slug` (numeric) and modern `netrunnerdb.com/en/decklist/{uuid}/slug`.

Conclusion: **support v2 only**, accepting both numeric and UUID ids — one endpoint, one response shape, covers every URL form; v3 adds a second host and a slug-keyed vocabulary for zero user-visible gain. Revisit only if v2 is retired.

---

## 2. Design overview

One paragraph, then the details.

Every visitor gets an **anonymous account** on first contact: a real `users` row with `kind='anon'`, identified by a `jrs_session` HttpOnly cookie. Anonymous users can build decks, import from NRDB, and play; everything they make is owned by their user id. **Claiming** is giving an email: a magic link (draftroom flow, draftroom guard, draftroom enumeration-safety, plus token-hashing at rest which both references skipped) lands in the inbox; clicking it either upgrades the anonymous row in place (new email) or merges it into the existing account (returning player), Cubehall-merge style with an audit row. Storage is **SQLite via `rusqlite`** in the server crate — the engine core stays sans-IO. Decks reference cards by canonical **title + latest code**, validate per the reference validator (ported from `validator.cljc`), and carry per-card **implementation status** so strict mode (`local.rs:85-109`) is a badge in the deck builder, not a surprise at game start. Public library = per-deck publish flag + fork. The client talks plain **HTTP JSON** (`/api/*`) for auth and decks — WebSocket stays what it is: the game channel.

```
first visit ──► anon user (cookie jrs_session, users.kind='anon')
                   │  builds decks, imports, plays; everything owned by user_id
                   ▼
            POST /api/auth/claim {email}
                   │  magic link → inbox (30 min, single-use, hashed at rest)
                   ▼
            GET /auth/verify?token=…
        ┌──────────┴───────────┐
   email is new           email has an account
        │                       │
  upgrade in place        merge anon → existing
  (same user_id;          (decks/games reassigned in one
   kind='claimed')         transaction; anon row → kind='merged';
                           merge audit row)
```

---

## 3. Identity model

### 3.1 Anonymous session

- **Creation.** The first request to any `/api/*` endpoint (in practice: the UI calls `GET /api/me` on load) mints a `users` row `{id, kind:'anon', display_name:'guest-<4 hex>', created_at}` and a `sessions` row, and sets the cookie. No interaction, no consent screen — an anonymous account is a cookie, nothing more. Static assets and `/health` never mint (crawlers must not create rows).
- **Cookie.** `jrs_session`; value is a 128-bit token, 16 bytes `OsRng` → base64url (exactly draftroom's `newToken`, `store.go:63-69`); attributes `HttpOnly; SameSite=Lax; Path=/; Secure` (secure iff TLS or `X-Forwarded-Proto: https`, as `draftroom/main.go:268-269`); `Max-Age` 400 days (browser cap). The cookie value is the session id; the session row is the authority.
- **Session rows.** `expires_at` = now + 14 days for claimed users (draftroom `sessionTTL`), now + 400 days for anonymous ones (an anon "session" is the only key to that identity — expiring it orphans the decks; the long TTL is the draftroom guest-cookie precedent, `main.go:548`). Sliding-window touch at most once per hour (`ValidateSession` semantics, `auth.go:380-393`).
- **Why a cookie and not localStorage** (the existing game token lives in localStorage): the identity token must be invisible to page JS (XSS containment) and must ride automatically on the WebSocket upgrade request so game sessions can be attributed to the user (§8.4). localStorage can do neither. HttpOnly cookies do both for free on a same-origin app.
- **Composition with the existing game token.** Untouched and orthogonal. `localStorage["jinteki_local"]` (`ui/app.js:72`) keys a *game* in the in-memory registry; `jrs_session` keys a *person*. A resumed game works with no cookie (registry token suffices, exactly today's behavior); a person with no live game has no localStorage entry. The only new coupling: when a game starts, the server records the owning `user_id` next to the registry entry so game history can be persisted per-user (§5.2 `games`), and deck-loaded starts check deck ownership. Neither token ever substitutes for the other.

### 3.2 Claimed account

A claimed user is the same row with `kind='claimed'`, an `email`, and `email_verified_at`. Claiming changes ownership of nothing — that is the whole point of anonymous rows being real users (§4).

### 3.3 Display names

- Anonymous: auto-generated `guest-xxxx`; editable immediately via `PUT /api/profile` — you do not need an email to be called something (lobby and library both want names).
- Rules (from the reference, `DESIGN.md` §B.10): ≤20 code points, no `://`, no `</`; additionally non-empty after trim (Cubehall `NonEmptyString`). Uniqueness NOT required (native mode has user ids; jnet-compat usernames are a different, later system).
- Resolved at read time, draftroom-style (`main.go:411-425,996-1002`): the library shows the deck author's *current* name.

### 3.4 Public vs private

| Datum | Visibility |
|---|---|
| Display name | Public (library author lines, future lobby) |
| Email | **Never rendered anywhere**, never returned by any endpoint except `GET /api/me` to its own session; stored for delivery + account matching only (§12.1) |
| Anonymous/claimed status | Private (only `/api/me` sees `kind`) |
| Decks | Private by default; public only via explicit publish (§6.4) |
| Game history | Private to the owner (native mode has no spectator/stat surface yet; when it does, opt-in per the reference's `:gamestats` precedent, `DESIGN.md` §B.12) |
| User ids | Opaque 128-bit tokens; appear in no public payload (library entries carry deck id + author *name* only) |

---

## 4. Magic-link claim flow

### 4.1 Endpoints

All under the existing axum router (`main.rs:27-31`); `/api/*` routes registered before the static fallback.

| Method | Path | Auth | Purpose |
|---|---|---|---|
| GET | `/api/me` | mints anon session if absent | `{user_id, display_name, kind, email?}` — email only for the owner |
| POST | `/api/auth/claim` | session required | body `{email}` → sends magic link; response always `{"sent": true}` (see 4.4) |
| GET | `/auth/verify` | none (the link IS the credential) | query `?token=…`; consumes, upgrades/merges, sets a fresh session cookie, `303 See Other` → `/` |
| POST | `/api/auth/logout` | session | deletes the session row, clears the cookie. For an anon user this **orphans the identity** — the UI warns before allowing it (§9.4) |
| PUT | `/api/profile` | session | `{display_name}` |
| DELETE | `/api/account` | claimed session | account deletion (§12.1) |

Design deltas from draftroom, each deliberate:
- **No `userId` in the link.** Draftroom's `verify?userId=…&token=…` (`email.go:44-47`) exists because registration challenges live inside the user record and need the user key to find. Our `claims` table is token-keyed (like Cubehall's `login_challenges`), so the token alone suffices — one fewer identifier leaking into referrer logs and inbox previews.
- **No separate register/login.** Draftroom needs two verbs because accounts begin at registration. Here the account already exists (it's the anon session); there is exactly one verb, *claim*, and it covers both "attach my email" and "log me back in on this device". A claim from a session that is already claimed is how re-login on a new device works too: fresh anon session → claim with your email → merge (§4.5 case B).
- **Verify stays a GET** (draftroom `main.go:403-425`), not Cubehall's POST: the link must work from a mail client with zero JS. Consuming a token on GET is a CSRF-shaped wrinkle; harmless here because verification is idempotent-in-effect for the legitimate owner and an attacker cannot benefit from forcing it (§12.3). Mail-scanner prefetch, however, can burn tokens — see 4.3.

### 4.2 Token lifecycle

- **Generation:** 16 bytes `OsRng`, base64url (`store.go:63-69` equivalent). Sent in the link **once**; never in any API response (Cubehall's TEST_MODE rule, `routes/auth.ts:79-85`, honored: test builds may echo it behind a `cfg(test)`/env gate only).
- **At rest: store `SHA-256(token)`, not the token.** Both reference projects store plaintext (`draftroom/authlog.go` ops carry `Challenge`/`Token`; `cubehall/db/sqlite.ts:159-165`); a DB/backup leak there converts directly into account takeover for 30 minutes. Hashing at rest costs one line (`sha2` crate) and removes the class. Lookup is by hash; no constant-time comparison needed beyond SQLite's index equality on a value the attacker can't choose collisions for.
- **TTL:** 30 minutes (`challengeTTL`, both references).
- **Single-use:** consumption = one SQLite transaction: `DELETE FROM claims WHERE token_hash=? RETURNING *` then act on the returned row; the delete-first shape is Cubehall's (`programs/auth.ts:171-187`) and draftroom's burn-even-if-expired subtlety (`auth.go:305-318`) comes free — an expired row is still deleted on presentation, and expiry is then checked on the returned value ("expired" distinct from "invalid", both references).
- **One pending claim per (session, email):** issuing a new one tombstones the old (draftroom `auth.go:253-259`, Cubehall `programs/auth.ts:243`).
- **Never inside the user row.** The Cubehall 2026-06-11 outage rule (§1.2). Our `claims` table is separate by construction, and the anon-GC (§5.2) keys only on ownership emptiness + inactivity, never on claim state.
- **Mail-scanner prefetch.** Corporate mail security opens links before the human does; with single-use GET tokens that burns the claim. Draftroom accepts this (the user requests a fresh link). We accept it too, with one mitigation: `/auth/verify` responds to `HEAD` without consuming, and the page served on GET completes the consume — acceptable to defer to a confirm-button page later if real-world burn rates demand (open item OI-2).

### 4.3 Delivery

SendGrid HTTP API v3, exactly draftroom's `sendViaSendGrid` (`email.go:70-129`): `POST https://api.sendgrid.com/v3/mail/send`, bearer `SENDGRID_API_KEY`, plain + HTML parts. `reqwest` is already a server dependency (`bridge/mod.rs:68-72`); no new crate.

- Env contract (draftroom `email.go:31-37`): `SENDGRID_API_KEY` (secret), `FROM_EMAIL`, `FROM_NAME`, `APP_URL`. Unset key ⇒ dev mode: log the link, send nothing (`email.go:60-63`) — this is also how local development works, full stop.
- **Email copy** (subject: `Sign in to jinteki-rs`):
  > Click below to sign in: **[Sign in]**
  > This link expires in 30 minutes. If you didn't request this, ignore this email — nothing happens without clicking it.
  If the claim would merge an existing account (case B), same copy; the email must not disclose whether the address has an account (that would leak the enumeration answer to anyone who can trigger a send — both references send distinguishable register/login copy; with a single *claim* verb we get uniform copy for free).
- Send failure never rolls back the claim row (draftroom `auth.go:143-146`): the user just asks for a fresh link.
- **Deployment reality** (from `vacationvm-deploy/services/draftroom.nix:22-63`): mail goes out through the shared geosurge SendGrid account; `FROM_EMAIL` must be an already-warmed identity on a domain whose reputation exists, and mails whose links point at a domain unrelated to the FROM domain get spam-filtered — draftroom learned this the hard way and moved the whole app onto the FROM's domain. jinteki-rs at `netrunner.sweater.vac.fere.me` sending from `noreply@cube.london` reproduces exactly the mismatch that failed. Options, operator's call at deploy time: (a) accept the risk initially and measure, (b) serve the app from a domain aligned with an authenticated sender. The spec only requires: `APP_URL` is the public origin, and the from-identity is domain-authenticated in SendGrid. Recorded as OI-1.

### 4.4 Abuse guard & enumeration safety

Port draftroom's `guard.go` wholesale — it is self-contained, dependency-free, in-memory-by-design ("heuristics, not correctness-critical", `guard.go:26-29`):

- Email send limits: 60 s per-address cooldown, 5/day per address, 80/hour global circuit breaker (`guard.go:32-34`), applied immediately before the mailer; suppressed sends logged and dropped with an unchanged HTTP response (`email.go:50-56`).
- Per-IP backoff on `POST /api/auth/claim`: 3 free per 10 min window, then 2 s → 8 s → 32 s … cap 5 min (`guard.go:267-303`); client IP from `X-Real-IP`/first `X-Forwarded-For` hop (Caddy sets them; `guard.go:65-76`).
- **Proof-of-work: omitted in v1.** Draftroom/Cubehall front *open registration* — an unauthenticated POST that triggers email. Ours is the same shape, so the email caps and IP backoff are MUST; the hashcash layer (a page of code + a client worker, `guard.go:152-249` + `web/pow.js`) is a SHOULD held in reserve, to be added unchanged if abuse appears. Rationale: every claim already requires an existing session cookie (a bot must first hit `/api/me`), which is a mild speed bump PoW would only deepen, and the global 80/hour budget hard-caps the blast radius at the SendGrid account.
- Enumeration: `POST /api/auth/claim` returns `{"sent": true}` for success, unknown-vs-known email, suppressed send, and suspended target alike (draftroom `main.go:365-368,389-392`). Timing: the SendGrid call is spawned async (draftroom does `go s.mailer.SendMagicLink…`, `main.go:375`), so response time doesn't oracle the DB lookup.

### 4.5 Verify outcomes

`GET /auth/verify?token=T`, in one transaction:

1. `DELETE FROM claims WHERE token_hash = sha256(T) RETURNING session_id, user_id, email, expires_at`. No row → redirect `/?auth=invalid`. Expired → redirect `/?auth=expired` (row already burned).
2. Look up `users` by claim's `email`:
   - **Case A — email unknown (first claim):** `UPDATE users SET kind='claimed', email=?, email_verified_at=now WHERE id = claim.user_id`. The user keeps their id; every deck and game row already points at it. Adoption is a no-op — this is the payoff of anonymous-users-are-users.
   - **Case B — email belongs to existing user E (returning player / second device):** merge `claim.user_id` (anon A) into E, per §4.6. If A *is* E (re-claiming your own account from a logged-in session — harmless), skip the merge.
   - **Case C — A is already claimed with a different email:** refuse (redirect `/?auth=conflict`). Changing the email of a claimed account is a future explicit re-key flow, not a side effect of a typo. (Neither reference supports email change; neither do we, yet.)
3. Mint a **fresh session row** for the resulting user and set the cookie — the verifying browser gets a new session regardless of what it held before (session-fixation hygiene, §12.2). The *initiating* browser's old session still points at the same (now upgraded) user in case A, so both devices end up signed in; in case B the initiating session was re-pointed by the merge.
4. `303 See Other → /`. No `next` parameter in v1 (single-page app, nothing to deep-link); if added later, clamp to local paths exactly as `draftroom/main.go:420-424`.
5. Append an `audit` row (§5.2): `claim_verified {user, email_hash, case}`.

### 4.6 Merge (anonymous → existing account), precisely

Modelled on Cubehall's admin merge (`db/sqlite.ts:205-217`, `model/user.ts:20-26`), executed automatically, inside the verify transaction:

```sql
UPDATE decks    SET owner_id = :E WHERE owner_id = :A;
UPDATE games    SET owner_id = :E WHERE owner_id = :A;
UPDATE sessions SET user_id  = :E WHERE user_id  = :A;   -- every device converges on E
UPDATE users    SET kind='merged', merged_into=:E, merged_at=now WHERE id=:A;
INSERT INTO merges (id, source_user_id, target_user_id, at, via) VALUES (…, :A, :E, now, 'claim');
```

Conflict & abuse cases:

| Case | Handling |
|---|---|
| Deck-name collision (A and E both have "My Hoshiko") | Allowed. Decks are id-keyed; names are labels (reference behavior — jnet decks have no unique names either, `web/decks.clj`) |
| Two anon sessions claim the same email concurrently | Two claim rows (keyed by token, one per (session,email) after tombstoning); each verify merges its own A into E; second merge finds A₂ still anon → merges cleanly. Serialized by SQLite's writer lock |
| A has zero belongings | Merge still runs (it's cheap); the tombstone documents where the session went |
| Claim link forwarded to someone else | Whoever clicks completes the claim *for the anon session that requested it* — they gain nothing except merging a stranger's anonymous decks into the email owner's account, and only the email owner receives the link in the first place. The dangerous direction (stealing A's decks) requires A's cookie, not the link |
| Hostile claim: attacker on their own browser claims *victim@example.com* | Link goes to the victim's inbox. If the victim clicks it, the *attacker's anonymous decks* merge into the *victim's* account — pollution, not theft: nothing of the victim's moves anywhere, the attacker's session now points at an account the attacker cannot access (their cookie's session row was re-pointed to E, but E's other sessions are untouched and… note: this WOULD log the attacker in as E). **Therefore case B must NOT re-point the claiming session automatically; instead:** the verify handler mints the fresh session (step 3) *only for the browser that clicked the link* — i.e., possession of the inbox. The `UPDATE sessions` line above is restricted to `WHERE user_id=:A AND id != :clicking_session` → **deleted instead of re-pointed** (`DELETE FROM sessions WHERE user_id=:A`). A's other devices fall back to anonymous-nothing and must claim again. This closes the "send a claim for an address you don't own, wait for the owner to click" fixation entirely: only the clicker (inbox holder) ends up signed in to E. The email copy's "nothing happens without clicking it" stays true |
| A is `merged` already (stale tab claims again) | `users.kind='merged'` refuses new claims and new writes; `/api/me` on such a session returns 401 + clears cookie, client re-bootstraps as fresh anon (Cubehall's merged-refuses-login, `programs/auth.ts:219-224`) |
| GC race: A pruned mid-claim | Cannot happen: GC (§5.2) skips users with any live claim row (cheap `NOT EXISTS`), the Cubehall lesson applied in the other direction |

The audit `merges` row keeps only ids and counts, not a full before-image (Cubehall records reversible `changes`; our merge moves ownership between two accounts *the same human controls* — reversal has no story worth its complexity).

---

## 5. Storage

### 5.1 Recommendation: SQLite via `rusqlite` (bundled), one file, WAL mode

Three candidates were mandated for consideration:

- **Append-only JSONL + fold (draftroom).** Beautiful where its property is needed: multi-process writers converging on shared files (`authlog.go:19-32`). jinteki-rs is one systemd unit on one box — that property is unexercised ballast. Costs we'd pay anyway: hand-rolled folds for every query shape (decks-by-owner, public library with filters, code→deck joins), rewrite semantics for mutable deck documents (draftroom's meta.json rewrite path, `store.go:151-189`, is the awkward half of its own design), and no transactions across stores (the §4.6 merge touches four tables atomically — with JSONL that's four files and a prayer; draftroom never needs cross-store atomicity, we do).
- **MongoDB (ICD §B.12).** Already contemplated — but for the *jnet-compat* surface (SYS-I-8: "reference server and jinteki-rs pointed at the same database", P1-read/P4-full). That is a compatibility obligation to *existing* jinteki.net operators, not a store for native accounts. Running mongod on the small vacationvm box for a subsystem that needs none of Mongo's properties (the box also hosts everything else; draftroom runs there precisely because its footprint is a binary and a directory) buys operational weight and buys nothing else. When the compat phase lands, Mongo arrives for the compat collections; the native tables do not move into it.
- **SQLite (Cubehall).** Proven in the sibling project for this exact flow (`db/sqlite.ts` — users, sessions, login_challenges, audit_events, user_merges: the same tables we need, minus its sql.js/WASM oddity which `rusqlite` with the bundled C library replaces). Single file at `${STATE_DIRECTORY}/jinteki.db`, zero daemons, real transactions (single-use token consume, §4.2; four-table merge, §4.6), indexes for the library queries, `VACUUM INTO` for backups (draftroom's `backup.sh` rsync pattern applies to the one file). `rusqlite` with `bundled` adds no system dependency — `nix/package.nix` needs nothing new.

Sans-IO discipline: the database is an interpreter-side concern. `jinteki-core` gains **zero** dependencies; all of §5 lives in `crates/jinteki-server` (DESIGN.md SYS-Q-1, A.5 — "Transport, DB, clocks, entropy live in interpreter/server crates"). Deck *validation* (§6.2) is pure and lives in core; deck *storage* is the server's.

Concurrency: axum handlers are async; `rusqlite` is sync. One writer connection behind a `tokio::sync::Mutex` + `spawn_blocking` (or a 2-3 connection read pool if the library page ever measures slow). At this scale (draftroom: "rooms number in the dozens, not the millions", `store.go:254-256` — same order here) this is not a bottleneck worth architecture.

### 5.2 Schema (DDL, normative)

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE schema_migrations (
  version    INTEGER PRIMARY KEY,          -- 1, 2, 3…; applied in order at boot
  applied_at TEXT NOT NULL
);

CREATE TABLE users (
  id                TEXT PRIMARY KEY,      -- 128-bit base64url token
  kind              TEXT NOT NULL CHECK (kind IN ('anon','claimed','merged','suspended')),
  display_name      TEXT NOT NULL,         -- ≤20 code points, no '://', no '</'
  email             TEXT UNIQUE,           -- lowercase-normalized; NULL for anon
  email_verified_at TEXT,
  merged_into       TEXT REFERENCES users(id),
  merged_at         TEXT,
  created_at        TEXT NOT NULL,
  last_seen_at      TEXT NOT NULL          -- coarse (daily), for the anon GC
);
CREATE UNIQUE INDEX idx_users_email ON users(email) WHERE email IS NOT NULL;

CREATE TABLE sessions (                     -- Cubehall db/sqlite.ts:137-143
  id               TEXT PRIMARY KEY,        -- the cookie value
  user_id          TEXT NOT NULL REFERENCES users(id),
  created_at       TEXT NOT NULL,
  expires_at       TEXT NOT NULL,           -- +14d claimed, +400d anon; sliding
  last_activity_at TEXT NOT NULL            -- touch at most 1/hour
);
CREATE INDEX idx_sessions_user ON sessions(user_id);

-- Ephemeral. NEVER folded into users (Cubehall 2026-06-11 lesson,
-- programs/auth.ts:233-237). Token stored hashed (improvement over both refs).
CREATE TABLE claims (
  token_hash  TEXT PRIMARY KEY,             -- hex sha256 of the link token
  session_id  TEXT NOT NULL,                -- the requesting session
  user_id     TEXT NOT NULL REFERENCES users(id),
  email       TEXT NOT NULL,                -- normalized target address
  created_at  TEXT NOT NULL,
  expires_at  TEXT NOT NULL                 -- created + 30 min
);
CREATE INDEX idx_claims_user ON claims(user_id);

CREATE TABLE merges (                       -- Cubehall user_merges, slimmed
  id             TEXT PRIMARY KEY,
  source_user_id TEXT NOT NULL,
  target_user_id TEXT NOT NULL,
  at             TEXT NOT NULL,
  via            TEXT NOT NULL DEFAULT 'claim'
);

CREATE TABLE decks (
  id          TEXT PRIMARY KEY,
  owner_id    TEXT NOT NULL REFERENCES users(id),
  name        TEXT NOT NULL,                -- ≤120 chars
  side        TEXT NOT NULL CHECK (side IN ('corp','runner')),
  identity_title TEXT NOT NULL,             -- canonical printed.rs title
  format      TEXT NOT NULL DEFAULT 'standard',
  cards_json  TEXT NOT NULL,                -- [{"title":…,"code":…,"qty":n}]
  notes       TEXT NOT NULL DEFAULT '',
  source_json TEXT,                         -- {"kind":"nrdb","id":…,"url":…} | {"kind":"fork","deck":…}
  published_at TEXT,                        -- NULL = private; set = in the library
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
CREATE INDEX idx_decks_owner   ON decks(owner_id, updated_at DESC);
CREATE INDEX idx_decks_library ON decks(published_at DESC) WHERE published_at IS NOT NULL;

CREATE TABLE games (                        -- durable per-user game history
  id          TEXT PRIMARY KEY,             -- the registry token (local.rs:121)
  owner_id    TEXT NOT NULL REFERENCES users(id),
  side        TEXT NOT NULL,
  deck_id     TEXT REFERENCES decks(id),    -- NULL: built-in demo decks
  seed        INTEGER,
  started_at  TEXT NOT NULL,
  finished_at TEXT,
  winner      TEXT,                         -- 'corp'|'runner'|NULL
  reason      TEXT
);
CREATE INDEX idx_games_owner ON games(owner_id, started_at DESC);

CREATE TABLE audit (                        -- Cubehall audit_events, slimmed;
  id      TEXT PRIMARY KEY,                 -- draftroom's log-IS-the-audit spirit
  at      TEXT NOT NULL,
  user_id TEXT,
  action  TEXT NOT NULL,   -- claim_requested|claim_verified|merge|login|logout|
                           -- deck_published|deck_unpublished|account_deleted|gc_anon
  detail  TEXT             -- JSON; NEVER contains raw tokens or full emails
);
```

Notes:
- `cards_json` as a JSON column, not a `deck_cards` join table: decks are read and written whole (the reference stores them whole too, `web/decks.clj:37-43`), the library filters on columns (`side`, `identity_title`, `published_at`), and nothing queries "all decks containing card X" — if that feature ever appears, add the index table then.
- **Game state itself stays in the in-memory registry** (`local.rs:35-43`) in this phase. The `games` table records existence/outcome for "my games" history; full durable state/replay is DESIGN.md SYS-I-9/F-4 territory (the event-log replay interpreter), not this subsystem's. The registry entry gains an `owner: Option<String>` and on game-over writes `finished_at/winner/reason`.
- **Anon GC:** daily sweep deletes `users WHERE kind='anon' AND last_seen_at < now-90d AND NOT EXISTS(deck) AND NOT EXISTS(game) AND NOT EXISTS(claim)` plus their sessions, and expired sessions/claims generally. Ownership emptiness is a GC criterion; claim state is a GC *veto* — both halves of the Cubehall lesson.

### 5.3 Migration story

- `schema_migrations` + numbered embedded SQL scripts (`include_str!`), applied in a transaction at boot before the listener binds. Version 1 is the DDL above. This is the boring, correct thing; Cubehall's create-if-not-exists-on-boot (`db/sqlite.ts`) without versions is the only thing there *not* worth copying.
- From today's deployment: nothing to migrate — there is no persisted account/deck data. First boot creates the file in `STATE_DIRECTORY` (the vacationvm module already grants a state dir to services that declare one; wire `JINTEKI_STATE_DIR` in `nix/vacationvm-module.nix` alongside `JINTEKI_SOCKET`, `vacationvm-module.nix:28-31`).
- Toward jnet-compat (SYS-I-8): native `users` and Mongo `users` remain distinct populations. If unification is ever wanted (a native account adopting a jnet username), that is a linking row, not a storage merge — recorded as OI-4, explicitly out of scope here.
- Backup: the vacationvm host's existing rsync/backup arrangements pick up the single DB file; `VACUUM INTO '<path>.bak'` gives a consistent snapshot without stopping writes if ever needed.

---

## 6. Decklists

### 6.1 Prerequisite: extend the card-data pipeline

`tools/gen-carddata.py` must additionally emit, per card, from `tools/raw_data.edn` (all fields verified present in the EDN):

| New `PrintedCard` field | EDN source | Needed by |
|---|---|---|
| `influence_cost: Option<i64>` | `:factioncost` | influence math (§6.2) |
| `influence_limit: Option<i64>` | `:influencelimit` (identities) | influence ceiling |
| `min_deck_size: Option<i64>` | `:minimumdecksize` (identities) | size + agenda floor |
| `previous_codes: Vec<String>` | `:previous-versions[].code` | NRDB import of old printings (§7.3) |
| `slug: String` | `:normalizedtitle` | future NRDB v3 (slug-keyed `card_slots`, §1.4) — cheap to carry now |
| `standard_banned: bool` (or a small `formats` map) | `:format` | format legality surface (v1 needs standard ban flags only) |

And two derived indexes in `printed.rs` beside `printed_index()` (`printed.rs:51-60`): `by_code` (latest codes) and `by_previous_code`. This mirrors the reference lookup exactly (`web/nrdb.clj:29-31`: find by `code`, else by `previous-versions.code`).

### 6.2 Internal representation & validation

Wire/storage deck shape (the `cards_json` of §5.2 and every API payload):

```json
{
  "id": "…", "name": "Reavershop", "side": "runner", "format": "standard",
  "identity": {"title": "Hoshiko Shiro: Untold Protagonist", "code": "26066"},
  "cards": [ {"title": "Sure Gamble", "code": "30001", "qty": 3}, … ],
  "notes": "imported from https://netrunnerdb.com/en/decklist/81579",
  "source": {"kind": "nrdb", "id": "81579", "url": "…"},
  "published_at": null, "created_at": "…", "updated_at": "…"
}
```

Cards are referenced by **canonical title** (the `printed.rs` key — the engine's `GameState::new_with_decks` takes titles, `local.rs:110-116`) **and** latest `code` (art URLs, `ui/app.js:523-525`; NRDB round-trips). On write the server re-canonicalizes: title looked up, code force-set to the latest printing (a client cannot smuggle a mismatched pair); unknown titles are rejected with the offending string.

**Validation** is a pure function in `jinteki-core` (new module `crates/jinteki-core/src/deckcheck.rs` — pure data-in/data-out, sans-IO clean), a direct port of the reference validator with the exotic cases staged:

v1 (must match `jinteki-reference/src/cljc/jinteki/validator.cljc` behavior):
- Identity present, correct side; no Identity cards in the list; every card's side matches (`validator.cljc:170-181` `allowed?`).
- Agendas neutral-or-identity-faction only (`validator.cljc:174-178`).
- Copies: `qty ≤ deck_limit` (default 3) (`validator.cljc:81-86`).
- Deck size ≥ `identity.min_deck_size` (`validator.cljc:46-49,186-189`).
- Influence: Σ off-faction `qty × influence_cost` ≤ `identity.influence_limit`, `NULL`/`"∞"` limit = unlimited (`validator.cljc:62-66,96-130` minus the alliance/Professor discounts).
- Corp agenda points: `min = 2 + 2·⌊max(count, min_deck_size)/5⌋`; legal iff `min ≤ points ≤ min+1` (`validator.cljc:51-55,199-201`).
- Standard bans surfaced as warnings (not blocks) in v1: casual server, no MWL points machinery yet.

Deferred, tagged in code as v2 (each cites its validator.cljc lines): alliance discounts (`:13-42`), The Professor (`:88-92,111-113`), singleton identities Nova/Ampère (`:68-78,146-167`), Custom Biotics (`:180-181`), full MWL/points (`:253+`). **Dependency note:** `docs/rules/` does not exist in the repo at time of writing; when the CR text lands there, `deckcheck.rs` doc-comments should cite CR §1.4 clauses next to the validator.cljc anchors. Until then validator.cljc at the pin IS the normative source — consistent with DESIGN.md's parity-with-the-reference stance (SYS-F-1).

Validation output (returned by every deck read/write and by `POST /api/decks/validate` for unsaved drafts):

```json
{
  "legal": false,
  "problems": [ {"code": "agenda-points", "message": "20 agenda points; needs 22–23"} ],
  "counts": {"cards": 49, "influence_used": 12, "influence_limit": 15,
             "agenda_points": 20, "min_deck_size": 45},
  "playable": {"behavior": 44, "jnet_only": 4, "unimplemented": 1},
  "cards": [ {"title": "…", "code": "…", "qty": 3, "impl": "behavior",
              "influence_spent": 0, "banned": false}, … ]
}
```

Problem messages name the card and the fix in card language — the reference's messages (`validator.cljc:203-211`) are the floor, DESIGN.md SYS-D-3's error-quality bar is the spirit.

### 6.3 Implementation status is a first-class deck property

The strict-mode rule stays exactly where it is: game start refuses decks containing non-`Behavior` cards and lists them (`local.rs:85-109`). This spec's obligation is that the refusal is **never news**: every deck payload carries per-card `impl` (`printed::impl_status`, `printed.rs:130-138`) and the roll-up `playable` counts, and the UI renders both (§9.2) — a deck the engine can't run yet says so on its card in the list, on every row in the editor, and on the import preview. `JnetOnly` and `Unimplemented` render distinctly (the former means "arrives with engine coverage", the latter "not even jinteki.net has it") — the same distinction `docs/CARD-COVERAGE.md` tracks for the porting campaign, now pointed at players.

### 6.4 Public library

Smallest thing that is genuinely a library — flag, not infrastructure:

- **Publish/unpublish:** `POST /api/decks/:id/publish` sets `published_at` (owner only; requires `legal` to keep the shelf browsable — impl-status does NOT gate publishing, a correct-but-not-yet-runnable deck is legitimate library content). Unpublish clears it. Publishing is the only anonymous-user action that requires being claimed: library content must survive its author's cookie hygiene, and a name like `guest-3f2a` is not an author line. (Anon users see the button with a "claim your account to publish" nudge — the organic claim funnel.)
- **Browse:** `GET /api/library?side=&faction=&q=&sort=recent|name&page=` → summaries `{deck_id, name, side, identity: {title, code}, author_name, published_at, playable, legal}`. Author name resolved at read (§3.3). Newest-first default (draftroom's listing order, `store.go:147`).
- **Read:** `GET /api/library/:deck_id` → full deck (redacted: no owner id, no source URL if the owner marked notes private — v1 keeps notes public, they're provenance).
- **Fork:** `POST /api/library/:id/fork` copies a published deck into the caller's decks (anon callers included — fork is how a phone visitor starts playing a real deck within a minute of arriving), `source_json = {"kind":"fork","deck": id}`, name suffixed " (fork)". Forks are copies, not references: upstream edits/unpublish don't mutate downstream (draftroom's first-writer-wins room mapping is the same instinct, `link.go:88-95`).
- **Moderation floor:** owner delete cascades to unpublish; a `users.kind='suspended'` account's published decks drop out of the listing query. Nothing fancier until scale demands it.
- **Seed content:** the two built-in demo decks (`carddb.rs:717-778`) are published at first boot under a system user ("jinteki-rs starter decks"), so the library is never empty and the game's current 100%-playable pool is one fork away.

---

## 7. NetrunnerDB import

### 7.1 Input forms (all accepted by `POST /api/decks/import {input}`)

Per the reference parser (`nrdb.clj:15-27`) extended with modern UUID reality (§1.4):

| Input | Example | Endpoint chosen |
|---|---|---|
| Public decklist URL | `https://netrunnerdb.com/en/decklist/81579/some-slug` | `decklist/81579` |
| Modern UUID decklist URL | `…/en/decklist/45c2efd4-…-a49a3a7f6368/slug` | `decklist/{uuid}` |
| Private deck URL | `…/en/deck/view/123456` | `deck/123456` |
| Bare id (numeric or UUID) | `81579` | `decklist/` first, `deck/` fallback (`nrdb.clj:65-85`) |

Extraction rule: substring after `decklist/` \| `deck/view/` \| `deck/`, truncated at `/`; then validated against `^[0-9]+$` or UUID shape before it goes anywhere near a URL we fetch (the reference skips this hardening; we don't).

### 7.2 Fetch

`GET https://netrunnerdb.com/api/2.0/public/{endpoint}{id}` via the existing `reqwest` client. Requirements:
- Honest static `User-Agent: jinteki-rs/0.1 (+https://netrunner.sweater.vac.fere.me)` — verified necessary (default UAs get 403 from NRDB's bot shield).
- Timeout 15 s; response cap 1 MiB; verify `success == true`, `total == 1`, `data[0]` present (`nrdb.clj:57-63`).
- Server-side per-user rate limit: 10 imports/minute (NRDB is a guest on someone else's infrastructure; same manners as the 5 req/s image throttle, `tasks/nrdb.clj:66-67` and DESIGN.md §B.12's "≤5 req/s").
- No card-data fetching ever — decklists are the only thing NRDB's API is used for; card data stays on the `netrunner-data` EDN pipeline (`tasks/nrdb.clj:23-37`, DESIGN.md SYS-D-5).

### 7.3 Mapping

`data[0].cards` is `{code: qty}`. For each code: resolve via `by_code`, else `by_previous_code` (§6.1; reference parity `nrdb.clj:29-31`) — either way the stored line gets the canonical title + **latest** code. `type == "Identity"` splits into the identity slot (`nrdb.clj:33-40`). Deck fields: `name` from NRDB `name`; `notes = "imported from " + readable_url` (`nrdb.clj:52-55`); `source = {"kind":"nrdb","id":…,"url":…}`; `format:"standard"` (`web/decks.clj:117`); side derived from the identity.

### 7.4 Error handling (better than the reference's silent drop)

The reference silently ignores unknown codes (`nrdb.clj:40`) — a deck imports "successfully" missing cards. We return a structured report and import anyway when possible:

```json
{ "deck": {…draft, unsaved…},
  "report": {
    "resolved": 48, "via_previous_printing": 3,
    "unknown_codes": ["09999"],
    "rotated": ["Ancestral Imager"],
    "validation": {…§6.2 shape, incl. per-card impl status…} } }
```

- Unknown code(s) → import proceeds minus those cards, prominently reported; if the *identity* is unknown, import fails (a deck with no identity is not a deck — same predicate as `web/decks.clj:113`).
- Rotated cards resolve normally (they exist in `cards.json` with `rotated: true`, `printed.rs:40-41`) and are flagged, not dropped — casual play allows them; the standard-legality warning does the talking.
- Failure taxonomy, each with user-facing copy: `bad-input` (unparseable id), `not-found` (NRDB 404 / `total != 1`), `nrdb-blocked` (403 — "NetrunnerDB refused the request; try again later"), `nrdb-down` (timeout/5xx), `rate-limited` (ours). Upstream failures map to 502-style semantics, not 400 — the caller did nothing wrong (draftroom's exact reasoning at `main.go:914-918`).
- The response is a **draft**: the client shows the report + preview (with impl badges), and saving is a second, explicit `POST /api/decks`. Import never silently lands a broken deck in the list. (The reference saves first and toasts after — `web/decks.clj:105-125`; the preview step is deliberate divergence, per docs/UX.md lesson 1: the report IS the one decision on screen.)

### 7.5 Interaction with strict mode

Import is where players first meet the coverage wall, so the import preview is where impl-status rendering matters most: the `playable` roll-up ("44 of 49 cards playable vs bot") sits at the top of the preview with per-card badges below. Saving is never blocked by impl status (decks are also for humans-vs-humans later, and for tracking the campaign); only *game start* enforces, exactly as today (`local.rs:85-109`).

---

## 8. Wire protocol additions

### 8.1 HTTP, not WS, for auth and decks — and why

The games channel stays WebSocket (`local.rs:6-14` contract untouched). Auth + decks go over plain HTTP JSON under `/api/`:

1. Cookie auth is native to HTTP; threading identity through a WS message envelope means reinventing session binding inside the socket protocol for zero gain.
2. `/auth/verify` MUST be an HTTP GET regardless (it's a link in an email) — so the auth subsystem is HTTP-shaped already; splitting it across two transports buys two places to have bugs.
3. Deck CRUD is request/response with no push component; the reference itself keeps deck CRUD on REST (`/data/decks`, ICD §B.2) even though it has a socket, and puts only `:decks/import` on WS — an accident of its lobby plumbing, not a design worth copying.
4. The dependency-free client stays simple: `fetch()` + cookie happens by default; no WS connection needed just to browse the library.

### 8.2 Endpoint summary (normative)

```
GET    /api/me                      → user summary (mints anon session)
POST   /api/auth/claim              {email} → {"sent":true}
GET    /auth/verify?token=…         → 303 / (+ Set-Cookie)
POST   /api/auth/logout             → {"ok":true}
PUT    /api/profile                 {display_name} → user summary
DELETE /api/account                 → {"ok":true}            (claimed only)

GET    /api/decks                   → [deck summaries, mine, newest first]
POST   /api/decks                   {deck draft} → full deck (validated)
GET    /api/decks/:id               → full deck + validation   (owner only)
PUT    /api/decks/:id               {deck} → full deck         (owner only)
DELETE /api/decks/:id               → {"ok":true}              (owner only)
POST   /api/decks/validate          {deck draft} → validation  (no save)
POST   /api/decks/import            {input} → {deck draft, report}
POST   /api/decks/:id/publish       → deck   (claimed owner, legal deck)
POST   /api/decks/:id/unpublish     → deck   (owner)

GET    /api/library?side&faction&q&sort&page → {decks:[…], total}
GET    /api/library/:id             → published deck (public)
POST   /api/library/:id/fork        → new deck (any session)
```

Conventions: JSON bodies ≤ 1 MiB (`decodeJSON` cap, `draftroom/main.go:1198-1205`); errors `{"error": "human sentence"}` with correct status codes; `Retry-After` on 429 (`draftroom/main.go:435-437`); mutating endpoints rely on SameSite=Lax + JSON `Content-Type` for CSRF exactly as both references do (no token dance; §12.4).

### 8.3 WS additions (the only two)

```
client → server on /ws/local:
  {"type":"start","side":"corp"|"runner","seed"?:n,"deck_id"?:"…"}   -- deck_id NEW
server → client:
  {"type":"error","error":"deck contains cards without implemented behavior: …"}  -- existing shape, local.rs:100-108
```

`start` with `deck_id`: the WS upgrade request carried `jrs_session` (same-origin cookie); `ws_local` (`main.rs:55-57`) extracts it and hands `Option<UserId>` to `local::handle`. The handler loads the deck (must be owned by that user or published), re-runs strict-mode over its actual titles instead of the hard-coded `carddb::corp_deck()/runner_deck()` (`local.rs:83-116`), and records the `games` row (§5.2). `start` without `deck_id` behaves exactly as today (demo decks, anonymous-OK, no cookie required — nothing regresses for cookieless clients).

### 8.4 Compat-mode note

None of `/api/*` collides with the ICD §B.2 REST surface (`/data/*`, `/chsk`, `/login`…) — when the compat router lands, the two surfaces coexist under one axum app without route conflicts. Deliberate: native endpoints are namespaced where jnet's never were.

---

## 9. UI

Constraints honored: one file `ui/app.js`, no framework, no build step; mobile-first per `docs/UX.md` (≥48 px targets, one decision per screen, bottom-anchored actions, long-press to read).

### 9.1 Screen graph (additions in caps)

```
screen-home ──┬── screen-game            (existing)
              ├── screen-lobby           (existing, bridge)
              ├── SCREEN-DECKS           my decks: list + [New] [Import]
              │      ├── SCREEN-DECK-EDIT   editor + validation panel
              │      └── SCREEN-IMPORT      paste URL → preview/report → save
              ├── SCREEN-LIBRARY         browse published; open → fork
              └── SCREEN-ACCOUNT         name, claim status, claim form, history
```

`screen-home` gains three chips under the existing cards — "Decks", "Library", and an identity chip (top corner: display name; tap → account). The Play-vs-Bot card gains a deck selector chip: default "Starter deck", tap → bottom-sheet listing my decks, **playable decks first**, unplayable ones greyed with their `unimplemented` count and not selectable for bot games — the strict-mode rule surfaced at the moment of choice, per UX.md lesson 2 (legality shown, not discovered).

### 9.2 Deck screens

- **My decks (`SCREEN-DECKS`):** rows = name, identity, side glyph, `legal` tick/cross, playable badge ("44/49" in amber when short of full, green check at 100%). Thumb-reachable primary actions; swipe/long-press for delete with confirm.
- **Editor (`SCREEN-DECK-EDIT`):** mobile deck editing is deliberately modest in v1 — name, notes, qty steppers per row (+/− at 48 px), remove; card *search-and-add* over the 2,065-title pool with a filter row (side auto-locked to identity, type, faction, text query) rendered as a virtual list (slice-render like the log drawer's `slice(-200)` trick, `app.js:778-792`). Every row: title, influence pips, impl badge; long-press → the existing zoom card (`cardInfoHtml`, `app.js:528-545` — reused untouched, it already renders NRDB art by code). A sticky validation strip pins to the bottom: size, influence x/y, agenda band, problem count; tap expands the full problem list. Server-side `POST /api/decks/validate` on every mutation, debounced — the client never re-implements the validator (UX.md lesson 2's "the UI never guesses" applied to deckbuilding).
- **Import (`SCREEN-IMPORT`):** one paste field + Import button; then the §7.4 report as a full-screen preview — deck header, playable roll-up, flagged cards grouped (unknown / rotated / not-yet-playable), Save / Discard as the two big bottom buttons. One decision on screen.
- **Library (`SCREEN-LIBRARY`):** filter chips (side, faction), search field, rows like My Decks plus author name. Open → read-only deck view with FORK as the primary bottom action; forking while anonymous just works (§6.4) — the phone-to-first-real-game path is: open library → fork → home → play.

### 9.3 Account & claim

`SCREEN-ACCOUNT`: display-name field (saves on blur); status line — either "Anonymous — decks live in this browser's cookie" with an email field + "Email me a sign-in link" button, or "Signed in as <email>" with logout. After claim submission: "Check your inbox — the link works for 30 minutes" (mirroring `draftroom/web/auth.js:65-66`). `/auth/verify` redirect lands on `/?auth=ok|invalid|expired|conflict`; app.js reads the param on boot and toasts the mapped copy (the `REASONS` table pattern, `web/auth.js:17-22`).

### 9.4 Session plumbing in app.js

- Boot: `fetch("/api/me")` before anything else; store the summary in a module global (never in localStorage — the cookie is the credential and it is HttpOnly). Then the existing localStorage game-resume runs unchanged (`app.js:144-157`).
- Logout while anonymous: confirm dialog — "This browser is the only key to these N decks. Claim with an email first?" (the §4.1 orphan warning).
- The bridge screen's saved jnet credentials (`localStorage["jinteki_bridge"]`, `app.js:137` — including a plaintext password) are out of scope but noted: native accounts must never adopt that pattern.

---

## 10. Deployment & secrets

Per the fleet patterns (`vacationvm-deploy/services/{draftroom,propscope}.nix`):

```nix
# vacationvm-deploy/services/jinteki-rs.nix (operator side)
vacationvm.services.jinteki-rs = {
  enable = true;
  subdomain = "netrunner";                          # netrunner.sweater.vac.fere.me
  environmentSecrets.SENDGRID_API_KEY = "jinteki-rs-sendgrid-key";   # agenix file
  environment = {
    APP_URL   = "https://netrunner.sweater.vac.fere.me";  # absolute magic links
    FROM_EMAIL = "noreply@cube.london";   # warmed sender — but see OI-1 (domain mismatch spam risk)
    FROM_NAME  = "jinteki-rs";
  };
};
```

Module side (`nix/vacationvm-module.nix`): add `JINTEKI_STATE_DIR` defaulting to the service's state directory; the server opens `${JINTEKI_STATE_DIR}/jinteki.db`. Unset `SENDGRID_API_KEY` = link-logging dev mode (§4.3) — the service must boot and fully function without any secret present, magic links landing in `journalctl` (draftroom's `main.go:93-95` startup warning included).

New crate dependencies (all in `jinteki-server` only): `rusqlite` (bundled), `sha2`. Everything else (reqwest, rand, serde_json, axum, tokio) is already there.

---

## 11. Proposed SYS-* requirements (text for DESIGN.md — do not paste into DESIGN.md from here without ratification)

New groups: **A** (accounts & identity), **K** (decklists & library), **N** (NetrunnerDB import). Format per DESIGN.md §5: statement · rationale · trace · verification. Suggested phase tags relative to the existing plan: these ride beside P1–P2 as a parallel native-mode track.

**SYS-A-1.** When operating in native mode, the SoI shall establish, on first API contact and without user interaction, an anonymous identity — a durable user record referenced by an opaque 128-bit HttpOnly session cookie — under which all user-created data (decks, game records) is owned from the first moment.
*Rationale:* MOE-2 (time-to-first-action) forbids a sign-up wall; ownership-from-birth is what makes later claiming a pointer flip instead of a data migration. *Trace:* NEED-1, STK-2. *Verify:* T (cookie minted exactly once per fresh client; rows owned; static assets mint nothing).

**SYS-A-2.** The SoI shall upgrade an anonymous identity to an email-claimed account exclusively via single-use emailed magic links; the SoI shall store no passwords and expose no password-accepting endpoint in native mode.
*Rationale:* the no-passwords property is structural (nothing to breach, nothing to phish at scale) and is the proven cube.london/draftroom flow (`draftroom/auth.go:20-34`). *Trace:* NEED-1, STK-2, STK-8. *Verify:* I (endpoint census) + T (claim flow end-to-end with a mock mailer).

**SYS-A-3.** Magic-link tokens shall be ≥128-bit random, stored only as cryptographic hashes at rest, expire within 30 minutes, be consumed atomically on first presentation (including expired presentations), and never appear in any API response or log.
*Rationale:* single-use-with-burn is draftroom's semantics (`auth.go:305-318`); hashing at rest closes the token-leak-via-backup class both reference implementations left open. *Trace:* STK-8. *Verify:* T (replay/expiry matrix) + I (schema and log inspection).

**SYS-A-4.** Claim-request handling shall be enumeration-safe — responses for known, unknown, suppressed, and suspended targets shall be indistinguishable in shape and observable timing — and shall enforce layered send limits: per-address cooldown and daily cap, a global hourly email budget, and per-IP backoff.
*Rationale:* a public game server must not oracle its member list, and outbound email is a shared-reputation resource with a hard budget (`draftroom/guard.go:31-40`, `main.go:365-368`). *Trace:* STK-8. *Verify:* T (response equality) + T (limit thresholds).

**SYS-A-5.** When a claim's email already belongs to an existing account, the SoI shall merge the anonymous identity into it in a single transaction — reassigning decks, game records, and terminating the anonymous identity behind an auditable merge record — and shall grant the resulting authenticated session only to the browser that presented the emailed token.
*Rationale:* the merge machinery is Cubehall's proven `user_merges` design (`db/sqlite.ts:205-217`); granting sessions only against inbox possession closes hostile-claim fixation (§4.6). *Trace:* STK-8, NEED-1. *Verify:* T (merge matrix incl. the hostile-claim case) + I (audit rows).

**SYS-A-6.** Ephemeral authentication state (pending claims) shall be stored disjointly from durable account records, and no garbage-collection criterion shall depend on any field mutated by an authentication attempt.
*Rationale:* the Cubehall 2026-06-11 incident — a GC keyed on auth state deleted a live account mid-login (`programs/auth.ts:233-237`); this requirement is that incident, generalized. *Trace:* STK-6. *Verify:* I + T (GC with in-flight claims).

**SYS-A-7.** Native-mode persistence shall reside entirely in server/interpreter crates; the engine core shall acquire no database, network, or clock dependency from this subsystem.
*Rationale:* restates SYS-Q-1's sans-IO boundary against the first subsystem tempted to violate it; deck *validation* is pure core, deck *storage* is the edge. *Trace:* SYS-Q-1. *Verify:* I (dependency lint, existing Q-1 CI).

**SYS-K-1.** The SoI shall let any session — anonymous included — create, edit, validate, and delete decklists owned by its identity, persisted durably across server restarts.
*Rationale:* decks are the on-ramp to play (MOE-2); durability is the property today's in-memory registry pointedly lacks (`local.rs:35-43`). *Trace:* NEED-1, STK-2. *Verify:* T (CRUD + restart survival).

**SYS-K-2.** Deck entries shall reference cards by canonical title and current NRDB code, re-canonicalized server-side on every write; unknown titles shall be rejected naming the offender.
*Rationale:* the engine keys on titles (`printed.rs:51-60`), art and NRDB round-trips key on codes (`ui/app.js:523-525`); carrying both, server-verified, prevents drift between them. *Trace:* SYS-D-5. *Verify:* T.

**SYS-K-3.** The SoI shall validate decks per the reference deck-construction rules — size, influence, per-card copy limits, identity/side/agenda-faction legality, and the Corp agenda-point band — reproducing `jinteki/validator.cljc` outcomes at the pin for the supported rule subset, with divergences allowlisted per SYS-F-1.
*Rationale:* deck legality is rules parity like everything else; the validator is small, pure, and portable. *Trace:* SYS-F-1, STK-4. *Verify:* T (fixture decks cross-checked against reference validator outputs).

**SYS-K-4.** Every deck representation delivered to a client shall carry per-card implementation status and an aggregate playability summary; game start shall continue to refuse decks containing cards without implemented behavior, naming them.
*Rationale:* the strict-mode refusal (`local.rs:85-109`) is correct and must stay; this requirement makes it impossible for that refusal to be the user's first notice. *Trace:* NEED-1 (trust), docs/UX.md lesson 2. *Verify:* T + D (UI walkthrough).

**SYS-K-5.** The SoI shall provide a public deck library: owners of claimed accounts may publish and unpublish their legal decks; any session may browse, read, and fork published decks into its own collection as independent copies.
*Rationale:* the library is the anonymous visitor's fastest path to a real deck (fork-and-play) and the claim funnel's honest incentive (publishing requires an account that outlives a cookie). *Trace:* NEED-1, STK-7. *Verify:* T + D.

**SYS-N-1.** The SoI shall import decklists from NetrunnerDB via the v2 public API, accepting decklist/deck URLs and bare ids in both numeric and UUID form, resolving cards by current code with fallback to previous-printing codes.
*Rationale:* reproduces the reference importer's contract (`web/nrdb.clj:15-31,65-85`) extended to the UUID ids NRDB now issues (verified live §1.4). *Trace:* STK-4, NEED-1. *Verify:* T (recorded-fixture responses; no live NRDB in CI).

**SYS-N-2.** Import shall produce an explicit report — cards resolved, resolved via previous printings, unknown codes, rotated cards, validation and playability results — and shall require a separate user confirmation before the imported deck is saved; an unresolvable identity shall fail the import.
*Rationale:* the reference silently drops unknown codes (`nrdb.clj:40`) and saves before showing consequences; both are the wrong default for trust (MOE-direction "faster to trust"). *Trace:* NEED-1, SYS-K-4. *Verify:* T + D.

**SYS-N-3.** Outbound NetrunnerDB requests shall carry an identifying User-Agent, be rate-limited per user and globally, time-bound, and size-capped; NRDB failures shall be classified and reported as upstream errors distinct from caller errors.
*Rationale:* polite-guest constraint, same manners as SYS-C-4's oracle rule and the 5 req/s image throttle (`tasks/nrdb.clj:66-67`); NRDB's bot shield (403s observed) makes the failure taxonomy user-visible. *Trace:* SYS-C-4 spirit. *Verify:* I + T.

---

## 12. Threat model & privacy

### 12.1 Emails

- Stored lowercase-normalized (draftroom `normaliseEmail`, `auth.go:119-130` incl. the `Name <a@b>` stripping), plaintext — delivery requires it; hashing at rest would be theater since the mailer needs cleartext. Unique-indexed.
- Never rendered to any other user, never in library payloads, never in logs (audit rows store `email_hash` when correlation is needed, `§5.2`); draftroom's token-redacting request logger (`main.go:1224-1246`) is the pattern for our request log: `/auth/verify` logged with the query string stripped.
- **Deletion:** `DELETE /api/account` — sessions and claims deleted; decks deleted (published ones included — the fork model means forks survive independently, so the shelf loses nothing it was promised); games kept with `owner_id` re-pointed to a system "deleted user" row (aggregate stats without the person); users row reduced to a tombstone (id + kind='merged'-style terminal state, email NULLed). Email is gone from the system at that point; SendGrid's own send logs are governed by SendGrid retention, out of our control and noted honestly.

### 12.2 Session fixation & hijack

- Verify always mints a fresh session for the clicking browser (§4.5 step 3) — a pre-set cookie value never survives authentication.
- The hostile-claim path (attacker requests a claim for a victim's address from the attacker's session) is closed structurally: sessions belonging to the anonymous requester are *deleted*, not upgraded, in merge case B; only the inbox-holding clicker receives an authenticated session (§4.6 table, row "hostile claim").
- Cookie is HttpOnly (no JS exfiltration), SameSite=Lax (no cross-site rides), Secure behind TLS. Token 128-bit — unguessable. Session ids in the DB are the raw cookie values (not hashed): accepted, matching both references; the DB-leak-to-hijack window is bounded by the 14-day TTL and is a candidate hardening (hash sessions like claims) noted in OI-3.

### 12.3 Magic-link interception

- The link rides SMTP+TLS to the provider and HTTPS from the click; the exposure is the inbox itself — accepted residual risk of the passwordless model (identical posture to both references).
- 30-minute TTL and single-use bound the window; burn-on-expired-presentation (`auth.go:313-317`) prevents retry-after-expiry probing.
- Token-only URLs (no userId, §4.1) keep account identifiers out of referrer headers and link-preview fetches; `Referrer-Policy: no-referrer` on the verify response as belt-and-braces.
- Consumed-on-GET + mail-scanner prefetch: HEAD is side-effect-free; if scanners that GET become a measured problem, the confirm-button page (OI-2) is the escalation — chosen over draftroom's raw-GET only if reality demands, because every added click is MOE-2 tax.

### 12.4 CSRF

State-changing endpoints are JSON-body POSTs guarded by SameSite=Lax cookies — a cross-site form can't set `Content-Type: application/json`, and Lax withholds the cookie on cross-site POST regardless. Identical posture to draftroom (`main.go:262-272`) and Cubehall (`routes/auth.ts:108-114`). The one GET with side effects (`/auth/verify`) mutates only state the bearer of an unguessable token is entitled to mutate; forcing someone else's verify requires the token, which is the credential. No CSRF token machinery in native mode (compat mode has its own, mandated by ICD §B.1 — unrelated).

### 12.5 "No passwords ever"

There is no password column, no password endpoint, no reset flow to phish. The bridge screen's stored jnet password (`ui/app.js:132-137`) predates this spec, talks to a *different* system, and is explicitly not part of native identity; native mode's property is absolute. Consequences owned: account access = inbox access; losing the email means losing the account (no recovery backchannel by design); both are the documented, accepted trade of the model the two reference projects shipped.

### 12.6 Abuse of the library

Published decks are user content: names/notes are text-rendered (the UI already renders all card text via `textContent`/escaped templates — keep it that way; no innerHTML with user strings in library rows), publish requires a claimed account (one email = one publishing identity, rate-limitable), and suspension exists as a kind (§5.2) from day one even though the moderation UI does not.

---

## 13. Implementation sketch (server crate layout)

Non-normative, for orientation. All inside `crates/jinteki-server/src/`:

```
main.rs        — router grows: .nest("/api", api::router(state)) + /auth/verify;
                 AppState { db: Db, mailer: Mailer, guard: Guard } via Extension
db.rs          — rusqlite wrapper: migrations, one writer conn (Mutex + spawn_blocking),
                 typed row structs; newToken() port (16B OsRng → base64url)
auth.rs        — session mint/validate/touch (draftroom ValidateSession semantics),
                 claim create/verify (§4.5 transaction), merge (§4.6), anon GC task
guard.rs       — draftroom guard.go port: email caps, IP backoff (PoW reserved)
mail.rs        — SendGrid via reqwest; dev-mode link logging; email copy (§4.3)
decks.rs       — CRUD + publish/fork + library queries; canonicalization (§6.2);
                 calls jinteki_core::deckcheck for validation + printed::impl_status
nrdb.rs        — input parse (§7.1), fetch (§7.2), map (§7.3), report (§7.4)
api.rs         — axum handlers, JSON conventions, error mapping
local.rs       — start gains deck_id + owner attribution (§8.3); strict-mode check
                 generalized from the hard-coded demo decks to the loaded deck
```

And in core (pure): `crates/jinteki-core/src/deckcheck.rs` (§6.2) plus the `printed.rs` field/index extensions (§6.1) fed by `tools/gen-carddata.py`.

Illustrative signatures (the load-bearing shapes only):

```rust
// jinteki-core/src/deckcheck.rs — sans-IO pure
pub struct DeckLine { pub title: String, pub qty: u32 }
pub struct Verdict { pub legal: bool, pub problems: Vec<Problem>,
                     pub counts: Counts, pub playable: PlayableSummary }
pub fn check(identity: &str, cards: &[DeckLine]) -> Verdict;   // printed-data only

// jinteki-server/src/auth.rs
pub async fn verify_claim(db: &Db, raw_token: &str, clicking_session: Option<&str>)
    -> Result<VerifyOutcome, VerifyError>;   // one transaction, §4.5–§4.6
```

---

## 14. Open items

| ID | Item | Closes when |
|---|---|---|
| OI-1 | Sender/domain alignment: `noreply@cube.london` sending links to `netrunner.sweater.vac.fere.me` reproduces the exact mismatch that spam-foldered draftroom's first mails (`services/draftroom.nix:24-31,43-56`). Decide: accept-and-measure vs domain-aligned serving | first deploy with mail enabled |
| OI-2 | Mail-scanner prefetch burning single-use GET tokens; escalation is a confirm-button verify page | if observed burn rate is non-trivial |
| OI-3 | Hash session ids at rest (claims already are); pure hardening | any schema-touching change |
| OI-4 | Native-account ↔ jnet-compat-account linking (shared identity across the two auth systems) | jnet-compat auth phase (SYS-I-7/I-8) |
| OI-5 | `docs/rules/` CR text absent; `validator.cljc` at the pin is the interim normative source for §6.2 — re-cite CR §1.4 clauses when the rules land | rules import |
| OI-6 | Validator v2 exotics: alliance, Professor, singleton IDs, MWL points (§6.2 deferred list) | card pool makes them reachable |
| OI-7 | PoW layer held in reserve (§4.4); wire the draftroom port if claim-endpoint abuse appears | abuse observed |
