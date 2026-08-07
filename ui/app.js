/* jinteki-rs mobile client.
   One renderer, three backends: the local engine and the CR engine (both with
   legality glow from the server's own action list) and the reference-server
   bridge (generic controls; the server is the authority). Both native engines
   speak the same jnet-shaped state and the same command vocabulary, so
   everything below "mode" is shared. Full redraw per state; CSS does the juice. */

"use strict";

const $ = (id) => document.getElementById(id);

let ws = null;
let mode = null;            // "local" | "cr" | "bridge"
/* Native engine (not the bridge): the server sends legal actions and we glow. */
function native() { return mode === "local" || mode === "cr"; }
let mySide = "runner";
let S = null;               // last state
let ACTIONS = [];           // legal actions (local mode)
let raised = null;          // raised hand card cid

/* ── THE ARMED CARD: the one card a second tap would act on ───────────────
 *
 * `null` means NOTHING is focused, and that is the resting state — the board
 * opens with no card singled out and returns there whenever you tap away.
 *
 * It exists because arming is the only cancel the rules leave room for. CR
 * 9.2.7f: "Each option must be fully resolved before another is chosen."
 * Once a card is played or an ability is used, the option has been chosen and
 * it resolves to the end; there is no rewind, and inventing one would also
 * hand the player a way to look at a card and then un-look at it. So the
 * whole cancellable region of the game sits BEFORE the command is sent, and
 * arming is what makes that region big enough to change your mind in: the
 * first tap only says "this one", the second one commits, and anything else
 * you do in between throws the intent away.
 *
 * One card, globally — not one per fan. Two cards lit in two places would be
 * two different answers to "what does the next tap do", and the point of the
 * thing is that the answer is always exactly one card or none. */
let armed = null;
function setArmed(cid) {
  if (armed === cid) return;
  armed = cid;
  repaintArmed();
}
/* Put the game back to nothing-focused. Also drops the hand's raised card:
 * a raised card is an intent too, and leaving it up after the intent it
 * belonged to was abandoned is the same lie in a different widget. */
function disarm() {
  const wasLit = Object.keys(fans).some((k) => fans[k].lit);
  if (armed === null && raised === null && !wasLit) return;
  armed = null;
  raised = null;
  // Nothing lifted, anywhere. The anchors stay where they are — the windows
  // must not jump when you tap the table — but no card is drawn as the one
  // you mean, because you have just said you mean none of them.
  Object.keys(fans).forEach((k) => { fans[k].lit = false; });
  repaintArmed();
}
/* Arming changes only what is LIT, never the game state, so this is a redraw
 * and never a state push (THE LAW §2). It goes through `render`, not through
 * the individual painters, so it keeps the per-section fault isolation and
 * the ordering the hand's band measurement depends on (§8). `armed` is part
 * of the dirty key of every section that draws a card, or the redraw would
 * decide nothing had changed. */
function repaintArmed() { if (S) render(); }
/* The game log's reading position — see "THE READER'S PLACE IN THE LOG IS
   THEIRS" by `renderLog`. Declared up here because `connect` resets them and
   a saved session reconnects from a top-level restore. */
let logFollow = true;
let logSeenK = null;

/* ── THE TWO TAPS, AS ARITHMETIC ─────────────────────────────────────────
 *
 * The board and the deck editor ask the player the same question — "which
 * one do you mean?" — and there must be exactly ONE answer to "what does a
 * tap do", or the deck editor teaches a gesture the board then punishes. The
 * editor used to add a copy on the FIRST tap, which is the single-tap commit
 * the board deleted (see `cardEl`): the same finger, on the same-looking
 * card box, meaning two different things on two screens.
 *
 * What the two screens genuinely do not share is WHERE the intent is kept.
 * The board's `armed` is read directly by a dozen dirty keys and repaints
 * through the whole `render`; the editor's grid repaints by touching two
 * classes on cards that are already on screen. So the SLOT is per screen —
 * `{ get, set }`, nothing more — and this function is the grammar over it:
 * the first tap on a candidate arms it, a tap on a DIFFERENT candidate
 * re-arms to that one and never commits, and only the second tap on the
 * armed thing acts (THE LAW §3, lesson 16). Returns true when it committed,
 * for the callers that care. */
function armTap(slot, key, commit) {
  if (slot.get() !== key) { slot.set(key); return false; }
  commit();
  return true;
}
/* The board's slot. `armed` stays a plain variable because it is read as one
   in every section's dirty key; this is the door the grammar knocks on. */
const BOARD_ARM = { get: () => armed, set: setArmed };

/* ── THE DECK EDITOR'S SLOT ──────────────────────────────────────────────
 *
 * The same intent, in the one other screen that shows cards as cards. It is
 * keyed by SURFACE and catalog id (`pool:01023`, `idp:01023`) rather than by
 * the id alone, because one card is drawn in several places at once — the
 * pool grid, the deck rows, the identity picker — and "the card the next
 * click acts on" must name a place as well as a card, exactly as the board
 * names a server column `srv:remote1` rather than "a remote".
 *
 * The painter, not `render`, is the repaint: the pool is hundreds of card
 * boxes with an image each, and re-running the grid to move a white ring
 * would destroy and rebuild every element on screen (including, mid-gesture,
 * the one under the finger). Focus is a class on cards that already exist. */
let builderFocus = null;
function setBuilderFocus(key) {
  if (builderFocus === key) return;
  builderFocus = key;
  paintBuilderFocus();
}
function clearBuilderFocus() { setBuilderFocus(null); }
const BUILDER_ARM = { get: () => builderFocus, set: setBuilderFocus };

/* THE FOCUSED CARD IS SURFACED — raised over its neighbours, scaled up and
 * ringed, so the card a second click will act on cannot be mistaken. Two
 * things it may not do:
 *
 *   · REFLOW. The grid is a static wall of cards and it stays put (THE LAW
 *     §2) — transform and z-index only, never a size, a margin or a gap.
 *   · CLIP. The grid is a scroller, so a card scaled from its centre in the
 *     first column pushes 8px past the container's edge and is cut off (and
 *     horizontally, worse: an overflow that makes a scrollbar appear IS a
 *     reflow). So the raise grows INWARD at the edges: the origin is pinned
 *     to whichever side would have overflowed. Measured flat — the class is
 *     off every card before anything is read — because a rect measured
 *     through a transform is the scaled rect and would compound.
 */
function paintBuilderFocus() {
  const cards = document.querySelectorAll(".card[data-armkey]");
  cards.forEach((c) => {
    c.classList.remove("armed", "surfaced");
    c.style.transformOrigin = "";
  });
  if (builderFocus == null) return;
  const c = [...cards].find((n) => n.dataset.armkey === builderFocus);
  // The focused card is not on screen any more — a filter, a search or a
  // side switch took it away. Then nothing is focused: keeping the key would
  // mean that a card which came back later would already be armed, and the
  // first click on it would commit.
  if (!c) { builderFocus = null; return; }
  const host = c.closest(".builder-grid");
  if (host) {
    // The scale lives in the stylesheet and is READ here, so the origin can
    // never disagree with the raise it is compensating for.
    const k = parseFloat(getComputedStyle(host).getPropertyValue("--surface-scale")) || 1.12;
    const g = host.getBoundingClientRect();
    const r = c.getBoundingClientRect();
    const pad = 5;                       // the ring's own 3px, and a hair
    const dx = (r.width * (k - 1)) / 2 + pad;
    const dy = (r.height * (k - 1)) / 2 + pad;
    const x = r.left - dx < g.left ? "left" : r.right + dx > g.right ? "right" : "center";
    const y = r.top - dy < g.top ? "top" : r.bottom + dy > g.bottom ? "bottom" : "center";
    c.style.transformOrigin = `${x} ${y}`;
  }
  c.classList.add("armed", "surfaced");
}
let prev = { credits: {}, clicks: {}, logn: 0 };
/* The last state push's timing snapshot: `{ at: performance.now(), d: state.timing }`.
   Null in untimed games — every timing element hides off this. */
let TIMING = null;
/* Which decision the current arm/raise/lit intents belong to — see the
 * "state" handler: a new decision throws stale intents away. */
let lastDecisionKey = "";
// Jitter control: deal-in animation fires only for cards never seen before,
// and each board section re-renders only when its slice of state changed.
let seenCids = new Set();
let sectionCache = {};
const hoverCapable = window.matchMedia("(hover: hover) and (pointer: fine)").matches;
let lobbyGameid = null;
let amHost = false;
let zoomTimer = null;
let toastTimer = null;

/* ── text glyphs ─────────────────────────────────────────────────────── */
function sym(t) {
  return String(t)
    .replaceAll("[Credits]", "⬡").replaceAll("[Credit]", "⬡")
    .replaceAll("[credits]", "⬡").replaceAll("[credit]", "⬡").replaceAll("[c]", "⬡")
    .replaceAll("[Click]", "●").replaceAll("[click]", "●")
    .replaceAll("[Subroutine]", "↳").replaceAll("[subroutine]", "↳").replaceAll("[sub]", "↳")
    // NSG's printed text (the card layer quotes it verbatim) uses these too.
    .replaceAll("[trash]", "⌦").replaceAll("[interrupt]", "⚡")
    .replaceAll("[recurring credit]", "⟳⬡").replaceAll("[link]", "🔗")
    .replaceAll("[mu]", "MU")
    .replaceAll("[their]", "their");
}

/* ── an ability's HANDLE, read as English ────────────────────────────────
 *
 * Every ability in the card layer carries a LABEL that is a developer handle,
 * not a display name. The builder stamps the card's own lowercased name onto
 * the front of whatever the card file wrote, so `.named("jackson: draw 2")` on
 * Jackson Howard is stored as "jackson howard: jackson: draw 2" — that is how
 * the plan-driver tests select an ability and how the traceability discipline
 * names one, and `every_ability_is_labelled_with_its_card` in the card crate
 * enforces the stamp. The server then composes "<handle> — <Card Name>" for
 * action windows, paid windows and the log.
 *
 * None of that was ever meant to be read at a table. This is the ONE place
 * that turns a handle into a sentence, and it does it entirely from the
 * convention — the card layer, its labels and the tests that select by them
 * are untouched.
 *
 * It only ever rewrites a string it can PROVE carries a handle, because it is
 * called on whole log lines and on affordance labels the server wrote in
 * English already. The proof is one of:
 *
 *   - the "<handle> — <Card Name>" suffix the server composes, whose right
 *     half names the very card the left half is stamped with;
 *   - a card name the caller already has (the sheet knows whose card it
 *     opened on, a prompt choice carries its card);
 *   - a title this game has actually shown (`seenTitles`), which is what lets
 *     a log line be cleaned when nothing else says which card it is about.
 *
 * With no proof it hands the string straight back (through `sym`), so
 * "Play Hedge Fund", "Runner: runs HQ." and a player's chat line all survive
 * unchanged — and, critically, so does a card whose own NAME contains a colon
 * ("Play Jinteki Biotech: Life Imagined" must not lose its card to a blind
 * split on the first one).
 *
 * `showCard` asks for "Card Name — Ability": right where the ability appears
 * on its own (the log, a chip in the rail), wrong where the card is already
 * on screen beside it (a sheet opened on that card, a prompt drawing it).
 */
const AB_SHORTHAND = [
  [/\br\s*&\s*d\b/gi, "R&D"], [/\bhq\b/gi, "HQ"], [/\bmu\b/gi, "MU"],
  [/\bai\b/gi, "AI"], [/\barchives\b/gi, "Archives"],
  [/\bcorp\b/gi, "Corp"], [/\brunner\b/gi, "Runner"],
];
/* The builder's automatic labels, for an ability the card file never named.
   They describe the KIND of ability and nothing else — the honest reading of
   "paid" is "a paid ability", and inventing card text here would be worse. */
const AB_KIND = [
  [/^\[sub\](?:\s+(\d+))?$/i, "Subroutine $1"],
  [/^paid(?:\s+(\d+))?$/i, "Paid ability $1"],
  [/^ability(?:\s+(\d+))?$/i, "Ability $1"],
  [/^play(?:\s+(\d+))?$/i, "Play $1"],
  [/^static(?:\s+(\d+))?$/i, "Static ability $1"],
];
function abilityText(raw, cardName, showCard) {
  // Accents are folded for the NICKNAME test alone: the stamp the builder
  // writes is `name.to_lowercase()`, which keeps them, but a card file writes
  // its shorthand in ASCII ("melies u:" for Méliès U). Never used for an
  // index into the original string.
  const fold = (x) => String(x).normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const lc = (x) => String(x).toLowerCase();
  let s = String(raw == null ? "" : raw).trim();
  if (!s) return "";
  let name = cardName || null;
  let proven = false;
  let lead = "";

  // The server's own composition. The suffix proves the stamp, and where the
  // caller had no card in hand it is also where the name comes from.
  const cut = s.lastIndexOf(" — ");
  if (cut > 0) {
    const head = s.slice(0, cut), tail = s.slice(cut + 3);
    const at = lc(head).indexOf(lc(tail) + ": ");
    if (at >= 0) {
      lead = head.slice(0, at);          // a speaker ("Corp: "), kept verbatim
      s = head.slice(at + tail.length + 2);
      if (!name) name = tail;
      proven = true;
    }
  }

  // Peel stamps and subroutine markers until nothing is left to peel. They
  // nest: the log narrates a subroutine as "<Ice>: [sub] <its handle>", and
  // the handle is itself stamped with the same card.
  for (let n = 0; n < 6; n++) {
    const before = s;
    const stamp = name && lc(s).startsWith(lc(name) + ": ")
      ? name
      : seenTitles().find((t) => lc(s).startsWith(lc(t) + ": "));
    if (stamp) {
      s = s.slice(stamp.length + 2);
      if (!name) name = stamp;
      proven = true;
    }
    // "[sub] End the run" is the kernel's own label; the marker is redundant
    // wherever this runs, because every site that draws a subroutine draws
    // the ↳ itself. A bare "[sub]" is a label, not a marker, and stays.
    const m = /^\[sub\]\s+(?!\d+$)(.+)$/i.exec(s);
    if (m) { s = m[1]; proven = true; }
    if (s === before) break;
  }
  if (!proven) return sym(String(raw == null ? "" : raw));

  // The card file's own shorthand for the same card ("jackson: draw 2"),
  // which is only ever a run of words the card's name already contains — so a
  // printed keyword ("Interface: Break 1 sentry subroutine", "Pump: +1
  // strength", "Terminal: The action phase ends") is left where it stands.
  if (name) {
    const i = s.indexOf(": ");
    if (i > 0) {
      const words = ` ${fold(name).replace(/[^a-z0-9]+/g, " ").trim()} `;
      const nick = fold(s.slice(0, i)).replace(/[^a-z0-9]+/g, " ").trim();
      if (nick && words.includes(` ${nick} `)) s = s.slice(i + 2);
    }
  }

  for (const [re, to] of AB_KIND) {
    if (re.test(s)) { s = s.replace(re, to).trim(); break; }
  }

  // An identity's ability is usually named after the identity's own subtitle,
  // lowercased by the stamp. Where the whole of what is left is a run of the
  // card's own words, the card's own capitalisation is the right one —
  // "ip recovery" is A Teia's "IP Recovery", not a typo.
  if (name) {
    const bare = (x) => fold(x).replace(/[^a-z0-9]+/g, " ").trim();
    const w = name.split(/\s+/), target = bare(s);
    outer:
    for (let i = 0; i < w.length; i++) {
      for (let j = i + 1; j <= w.length; j++) {
        const cand = w.slice(i, j).join(" ");
        if (bare(cand) === target) { s = cand; break outer; }
      }
    }
  }

  for (const [re, to] of AB_SHORTHAND) s = s.replace(re, to);
  s = s.replace(/^([^\p{L}]*)(\p{Ll})/u, (_, pre, c) => pre + c.toUpperCase());
  s = s.replace(/(:\s+)(\p{Ll})/gu, (_, sep, c) => sep + c.toUpperCase());
  s = sym(s);
  return lead + (showCard && name ? `${name} — ${s}` : s);
}

/* Every card title this game has shown, longest first — the only evidence a
   log line gives that a "<something>: " prefix is a card's name and not part
   of a sentence. It GROWS: a line about a card that has since left the table
   is still a line about that card. */
let titleSeen = new Set();
let titleList = [];
function seenTitles() { return titleList; }
function noteTitles(v, depth) {
  if (!v || depth > 8) return;
  if (Array.isArray(v)) { v.forEach((x) => noteTitles(x, depth + 1)); return; }
  if (typeof v !== "object") return;
  if (typeof v.title === "string" && v.title && !titleSeen.has(v.title)) {
    titleSeen.add(v.title);
    titleList = [...titleSeen].sort((a, b) => b.length - a.length);
  }
  Object.values(v).forEach((x) => noteTitles(x, depth + 1));
}

/* A player-supplied string on its way into innerHTML — display names are the
   only such strings the board renders (§12.6: everything else uses nodes). */
function esc(s) {
  return String(s == null ? "" : s)
    .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

/* ── screens ─────────────────────────────────────────────────────────── */
function show(id) {
  // Leaving the deck editor takes its focus with it. An intent that outlives
  // the screen it was made on is one nobody can see and nobody can cancel —
  // and coming back to a card already armed would make the FIRST click a
  // commit, which is the whole thing the two clicks exist to prevent.
  if (id !== "screen-builder") clearBuilderFocus();
  document.querySelectorAll(".screen").forEach((s) => s.classList.remove("active"));
  $(id).classList.add("active");
}

/* ── networking ──────────────────────────────────────────────────────── */
function connect(path, onopen) {
  seenCids = new Set();
  titleSeen = new Set();
  titleList = [];
  sectionCache = {};
  // A fresh connection is a fresh log: follow it from the bottom, with
  // nothing outstanding to be told about (see `logFollow`).
  logFollow = true;
  logSeenK = null;
  const proto = location.protocol === "https:" ? "wss" : "ws";
  ws = new WebSocket(`${proto}://${location.host}${path}`);
  ws.onopen = () => { wsRetry = 0; onopen && onopen(); };
  ws.onmessage = (ev) => handle(JSON.parse(ev.data));
  ws.onclose = () => {
    // A dropped socket is a tunnel, not a decision. The room and the game
    // both live server-side behind a token, so come back and say who we
    // are: the seat is still advertised, the game is still ours. Backoff
    // caps quickly because the common case is a phone waking up.
    showDisconnected();
    const wait = Math.min(1000 * Math.pow(2, wsRetry++), 8000);
    setTimeout(() => {
      if (ws && ws.readyState === WebSocket.OPEN) return;
      const saved = JSON.parse(localStorage.getItem("jinteki_local") || "null");
      connect(path, () => {
        if (saved && saved.token) send({ type: "resume", token: saved.token });
        else if (mode === "cr") send({ type: "lobby-list" });
      });
    }, wait);
  };
}
let wsRetry = 0;
function showDisconnected() {
  if (!document.getElementById("screen-game").classList.contains("active")) {
    toast("Connection closed");
    return;
  }
  const o = $("gameover-overlay");
  o.style.display = "flex";
  o.innerHTML = `<h1>DISCONNECTED</h1>
    <div class="why">The connection dropped. Your game is saved server-side — reconnect to resume.</div>
    <button class="big go" onclick="location.reload()">Reconnect</button>`;
}
function send(obj) {
  if (ws && ws.readyState === 1) ws.send(JSON.stringify(obj));
  else toast("Not connected");
}
function act(command, args) {
  // The intent has been sent. It belongs to the game now, not to the player —
  // CR 9.2.7f resolves a chosen option to the end — so nothing stays armed
  // across a command, and the ring goes out the moment the tap lands rather
  // than when the new state gets back.
  armed = null;
  raised = null;
  Object.keys(fans).forEach((k) => { fans[k].lit = false; });
  repaintArmed();
  send({ type: "action", command, args: args || {} });
}

function handle(m) {
  switch (m.type) {
    case "session":
      localStorage.setItem("jinteki_local",
        JSON.stringify({ token: m.token, side: m.side, engine: m.engine || "local" }));
      if (m.engine === "cr") mode = "cr";
      if (m.side) mySide = m.side;
      // A waiting seat or a ready-check table that just became a game is a
      // game now.
      crWaitToken = null;
      crWaitId = null;
      crLobbySeated(false);
      $("crlobby-mine").textContent = "";
      crPairingClear();
      break;
    case "state":
      S = m.state;
      ACTIONS = m.actions || [];
      // Every card this game has named, kept so `abilityText` can tell a
      // handle's "<card name>: " stamp from a colon inside a sentence.
      noteTitles(S, 0);
      // The clocks are the SERVER's (server-authoritative): every push
      // carries the remaining times as of the moment it was sent, and the
      // client only counts the local interval down from that snapshot —
      // the 1s server sync corrects any drift.
      TIMING = S.timing ? { at: performance.now(), d: S.timing } : null;
      if (m.mode === "bridge" && m.side && m.side !== "spect") mySide = m.side;
      // An armed thing is an intent AIMED AT A QUESTION, and it must die
      // with that question: if a new decision arrived while something was
      // still armed, a first tap on it would be taken for the confirming
      // second — one tap, committed, on a question the player never armed
      // anything for.
      //
      // So the key has to name the QUESTION and nothing else. `decision-seq`
      // is the server's per-decision stamp, minted once where the question is
      // put (`cr::present`) and carried unchanged by every re-send of it — a
      // timed game re-pushes the same decision once a second, and a key that
      // moved with the frame would disarm the board between the two taps of
      // every board target, which is exactly what makes an install into a
      // remote impossible to finish. Older/bridge servers that do not send
      // one fall back to the prompt's own content, which is at least stable
      // across a repaint.
      {
        const p = myPrompt();
        const dkey = S["decision-seq"] != null
          ? `d${S["decision-seq"]}`
          : (p ? `${p.msg}|${(p.choices || []).map((c) => c.uuid).join(",")}` : "");
        if (dkey !== lastDecisionKey) {
          lastDecisionKey = dkey;
          armed = null;
          raised = null;
          Object.keys(fans).forEach((k) => { fans[k].lit = false; });
        }
      }
      show("screen-game");
      enterGameChrome();
      render();
      break;
    case "connected":
      $("lobby-status").textContent = "connected — pick or create a game";
      send({ type: "lobbies" });
      break;
    case "lobbies": renderLobbies(m.list || []); break;
    case "lobby": renderLobbyState(m.lobby); break;
    /* The CR lobby (our own server, human vs human). Distinct message types
       from the bridge's, identical row shape, so the renderers stay thin. */
    case "lobby-list": renderCrLobbies(m.list || []); break;
    case "lobby-waiting": crWaiting(m); break;
    case "lobby-pairing": crPairingRender(m); break;
    case "lobby-gone": crPairingGone(); break;
    case "decks": renderDecks(m.list || []); break;
    case "reply":
      if (m.purpose === "join" || m.purpose === "watch") {
        if (m.data !== 200) { toast("Join refused: " + JSON.stringify(m.data)); }
        else if (m.purpose === "join") send({ type: "decks" });
      }
      if (m.purpose === "deck") $("lobby-status").textContent = "deck selected — waiting for start";
      break;
    case "toast": toast(sym(m.toast && (m.toast.message || m.toast["message"]) || "…")); break;
    case "error":
      if (m.cr_readiness) {
        // SYS-D-12 on the socket: the gate is evaluated at every start, so a
        // refusal here is the freshest truth there is. Show the whole gap.
        CR_READY = m.cr_readiness;
        renderCrReady();
        showCrGap();
      } else if (m.error === "session expired") {
        localStorage.removeItem("jinteki_local");
        show("screen-home");
        toast("Previous game expired — start a new one");
      } else if (m.error.startsWith("deck contains cards without implemented behavior")) {
        // Strict-mode refusal: show the per-card reasons in full, not a
        // truncated toast (ACCOUNTS-AND-DECKS.md §6.3).
        showStrictRefusal(m.error);
      } else {
        toast("⚠ " + m.error);
      }
      break;
    case "disconnected": toast("Disconnected from reference server"); show("screen-home"); break;
    default: break;
  }
}

/* ── home wiring ─────────────────────────────────────────────────────── */
$("pick-runner").onclick = () => pickSide("runner");
$("pick-corp").onclick = () => pickSide("corp");
function pickSide(s) {
  mySide = s;
  document.querySelectorAll("#card-vsbot .seg")
    .forEach((b) => b.classList.toggle("on", b.dataset.side === s));
}
pickSide("runner");

/* ── eternal mode (CR engine) ────────────────────────────────────────────
   The default mode: the two eternal decks on the Comprehensive Rules VM.
   The completeness gate (SYS-D-12) is the server's, evaluated at every start;
   this screen only reports it, so the mode goes live the moment the card
   layer closes — no deploy, no flag. */
let crSide = "runner";
let CR_READY = null;

$("cr-pick-runner").onclick = () => pickCrSide("runner");
$("cr-pick-corp").onclick = () => pickCrSide("corp");
function pickCrSide(s) {
  crSide = s;
  document.querySelectorAll("#card-eternal .seg")
    .forEach((b) => b.classList.toggle("on", b.dataset.side === s));
}
pickCrSide("runner");

async function loadCrReady() {
  try { CR_READY = await api("/api/cr-readiness"); }
  catch (e) { CR_READY = null; }
  renderCrReady();
}

function renderCrReady() {
  const box = $("cr-ready");
  const btn = $("btn-cr");
  box.textContent = "";
  if (!CR_READY) { box.textContent = "card readiness unavailable"; return; }
  const frac = `${CR_READY.complete}/${CR_READY.total}`;
  const pct = CR_READY.total ? Math.round((CR_READY.complete / CR_READY.total) * 100) : 0;
  const line = el("div", "cr-frac");
  line.appendChild(el("b", CR_READY.ready ? "ok" : "warn", frac));
  line.appendChild(el("span", "", ` cards implemented${CR_READY.ready ? " — ready" : ""}`));
  box.appendChild(line);
  const bar = el("div", "cr-bar");
  const fill = el("div", "cr-bar-fill");
  fill.style.width = pct + "%";
  if (CR_READY.ready) fill.classList.add("ok");
  bar.appendChild(fill);
  box.appendChild(bar);
  (CR_READY.decks || []).forEach((d) => {
    box.appendChild(el("small", "hint",
      `${d.title} — ${d.complete}/${d.distinct} (${d.copies} cards)`));
  });
  btn.textContent = CR_READY.ready ? "vs Bot" : `Not yet — ${frac}, see what's missing`;
  btn.classList.add("go");
  btn.classList.toggle("alt", !CR_READY.ready);
  $("cr-seed").style.display = CR_READY.ready ? "" : "none";
  // Both doors open together, and neither before the card layer closes.
  $("btn-cr-lobby").style.display = CR_READY.ready ? "" : "none";
}

/* ── THE SEED IS A STRING, THE WHOLE WAY DOWN ─────────────────────────────
 *
 * A game seed is a u64 on the server. JavaScript's only number is a double,
 * so 2^53 is the last integer this client can hold exactly — and `parseInt`
 * turned the seed 9661175140325481871 into 9661175140325482000 and jacked in
 * to a DIFFERENT GAME from the one whose seed its player had pasted in to
 * replay. Without a word, and with the wrong number echoed back to them in
 * the log as if it were the one they asked for.
 *
 * So a seed is NEVER a Number here. The box's text goes on the wire as text,
 * and the one parse to u64 happens on the server (`local::seed_from_wire`),
 * where u64 is a type you can actually hold. BigInt does the range check
 * because it is the only thing in this language that can compare nineteen
 * digits without dropping any of them.
 *
 * Returns `undefined` for an empty box — the seed IS optional, and an absent
 * one is a random game — the digit string when the box holds a seed, and
 * `false` when it holds something else, which every caller reads as "it has
 * been said out loud; start nothing". Nonsense is refused rather than
 * silently rounded down to a game nobody asked for.
 */
const U64_MAX = 18446744073709551615n;
function seedFromBox(id) {
  const t = $(id).value.trim();
  if (t === "") return undefined;
  if (!/^\d+$/.test(t) || BigInt(t) > U64_MAX) {
    toast("A seed is a whole number from 0 to " + U64_MAX);
    return false;
  }
  return t;
}

$("btn-cr").onclick = () => {
  if (!CR_READY || !CR_READY.ready) { showCrGap(); return; }
  // The seed is read BEFORE the socket: a refused seed must not leave a
  // connection — and a game screen — behind it.
  const seed = seedFromBox("cr-seed");
  if (seed === false) return;
  mode = "cr";
  mySide = crSide;
  connect("/ws/local", () => {
    send({
      type: "start",
      engine: "cr",
      side: crSide,
      seed,
      // Dev/test hook: `?timing=dev` (or `main:…,action:…,…`) starts this
      // game with clocks. Absent = untimed, as every game is today.
      timing: timingFromQuery(),
    });
  });
};

/* The honest "not yet": every card that is not playable, with the exact
   printed sentences the card vocabulary cannot say yet. */
function showCrGap() {
  show("screen-cr-gap");
  const head = $("crgap-head");
  head.textContent = "";
  if (!CR_READY) { head.appendChild(el("div", "zline", "readiness unavailable")); return; }
  head.appendChild(el("h3", "", `${CR_READY.complete}/${CR_READY.total} cards implemented`));
  head.appendChild(el("div", "hint",
    "No card is playable until its behaviour is implemented — a game that looks legal and is not " +
    "would be worse than no game. These are the cards still to write, and what about them cannot " +
    "yet be said."));
  (CR_READY.problems || []).forEach((p) => head.appendChild(el("div", "import-bad", p)));
  const list = $("crgap-list");
  list.textContent = "";
  (CR_READY.missing || []).forEach((m) => {
    const row = el("div", "deck-row");
    const t = el("div", "t");
    const l1 = el("div", "", `${m.copies}× ${m.title}`);
    l1.appendChild(el("span", "badge-impl warn", m.deck === "andromeda" ? "Andromeda" : "Gauntlet"));
    t.appendChild(l1);
    (m.unimplemented || []).forEach((s) => t.appendChild(el("div", "ztext", "“" + sym(s) + "”")));
    row.appendChild(t);
    list.appendChild(row);
  });
  if (!(CR_READY.missing || []).length) {
    list.appendChild(el("div", "deck-row", "Nothing missing — start a game."));
  }
}

/* ── Find a Game: the eternal lobby, with a ready check ──────────────────
   Creating picks a side (and a deck for it); joining a listed lobby — or
   "Play anyone", which autopairs with the oldest compatible seat — puts
   both players at a ready-check table. Both ready, the SERVER counts
   5-4-3-2-1 and drops both seats into the game; unready or leave cancels.
   The completeness gate is the SAME gate — the server refuses a create
   exactly as it refuses a bot start, so the honest screen is one screen.

   Self-contained: `openFindGame()` (exported on window) is the whole entry
   point, for whatever shell ends up mounting it. */
let crWaitToken = null;
let crWaitId = null;
let crPairing = null;      // the ready-check table this client sits at
let CR_LOBBIES = [];   // the open games as last listed — what "Play anyone" joins

/* The lobby deck picker: GET /api/decks (contract {"decks":[{key,name,
   builtin,legal,side…}]}) filtered per side. Until that catalog exists —
   it is landing separately — the pickers fall back to the two eternal
   decks, sent as no key at all, which is also what the server builds. */
const CR_DEFAULT_DECK = { runner: "Mezzie's Andromeda", corp: "Mezzie's Making Stars" };
let LOBBY_DECKS = { runner: [], corp: [] };

async function loadLobbyDecks() {
  let by = { runner: [], corp: [] };
  try {
    const r = await api("/api/decks");
    (Array.isArray(r && r.decks) ? r.decks : []).forEach((d) => {
      if (!d || !d.key || d.legal === false) return;
      const side = d.side === "corp" ? "corp" : d.side === "runner" ? "runner" : null;
      if (side) by[side].push({ key: d.key, name: d.name || d.key });
    });
  } catch (e) { /* no catalog is not an error — the fallback stands */ }
  ["runner", "corp"].forEach((side) => {
    if (!by[side].length) {
      by[side] = [{ key: null, name: side === "runner" ? "estrike Regular Andromeda" : "Gauntlet" }];
    }
    const sel = $(`crlobby-deck-${side}`);
    sel.textContent = "";
    by[side].forEach((d) => {
      const o = document.createElement("option");
      o.value = d.key || "";
      o.textContent = d.name;
      sel.appendChild(o);
    });
    const dflt = by[side].find((d) => d.name === CR_DEFAULT_DECK[side]);
    if (dflt) sel.value = dflt.key || "";
  });
  LOBBY_DECKS = by;
}

/* The chosen deck key for a side ("" = the default deck, sent as no key). */
function crDeck(side) {
  const sel = $(`crlobby-deck-${side === "corp" ? "corp" : "runner"}`);
  return (sel && sel.value) || undefined;
}

/* ── the timing selector ─────────────────────────────────────────────────
   Four modes; the default is Timed 30 minutes a side + Rope, which is also
   the server's default for a create that says nothing. The object built
   here IS the server's TimingConfig (timing.rs) on the wire. */
function crTiming() {
  const mode = $("crlobby-timing-mode").value;
  const num = (id, dflt) => {
    const v = parseInt($(id).value, 10);
    return Number.isFinite(v) && v > 0 ? v : dflt;
  };
  const t = {};
  if (mode.startsWith("timed")) t.main_clock_secs = num("crlobby-mins", 30) * 60;
  if (mode.endsWith("rope")) {
    t.rope = {
      calm_secs: num("crlobby-rope-calm", 60),
      opening_calm_secs: num("crlobby-rope-opening", 120),
      action_increment_secs: num("crlobby-rope-inc", 10),
      rope_secs: num("crlobby-rope-secs", 30),
    };
  }
  return t;
}
$("crlobby-timing-mode").onchange = () => {
  const mode = $("crlobby-timing-mode").value;
  $("crlobby-mins-label").style.display = mode.startsWith("timed") ? "" : "none";
  $("crlobby-rope-adv").style.display = mode.endsWith("rope") ? "" : "none";
};
function crDeckName(side, key) {
  const d = (LOBBY_DECKS[side] || []).find((x) => (x.key || "") === (key || ""));
  return d ? d.name : key || (side === "corp" ? "Gauntlet" : "estrike Regular Andromeda");
}

function openFindGame() {
  if (!CR_READY || !CR_READY.ready) { showCrGap(); return; }
  mode = "cr";
  show("screen-cr-lobby");
  $("crlobby-status").textContent = "connecting…";
  loadLobbyDecks();
  connect("/ws/local", () => send({ type: "lobby-list" }));
}
window.openFindGame = openFindGame;
$("btn-cr-lobby").onclick = openFindGame;

$("crlobby-back").onclick = () => { if (ws) ws.close(); show("screen-home"); };
$("crlobby-refresh").onclick = () => send({ type: "lobby-list" });
// "Play anyone": the one-button lobby. An open seat is a person already
// waiting — take it, whichever side it is (the joiner was always given the
// leftover side; this only stops making them read the list first). Nothing
// to join means nobody is waiting, so BE the person waiting, on a coin flip
// — 50/50 corp or runner, so two strangers both pressing the button meet in
// the middle instead of both hosting the same side.
// "Play anyone": the one-button lobby, through the SERVER's autopair — it
// seats you at ROPED tables only (the user's rule), takes the oldest
// compatible seat whichever side it is, and when nobody is waiting it opens
// a seat on a 50/50 coin flip so two strangers pressing the button meet in
// the middle instead of both hosting the same side.
/* THREE VERBS, ONE MECHANISM. All three autopair; they differ only in which
   sides the player is willing to take, and the SERVER reads that from which
   sides appear in `decks` (a side is playable iff its key is present). That
   is what makes a preference real: it constrains the JOIN as well as the
   seat this player opens when nobody is waiting.
     Play now  — no preference: both sides, and if nobody is waiting the seat
                 opens on a coin flip so two strangers pressing the same
                 button do not both host the same side.
     as Runner — runner only: joins a table that needs a runner, else waits as one.
     as Corp   — corp only. */
function crFindGame(pref) {
  if (crWaitToken) return; // already seated and waiting — cancel is the verb now
  const decks = {};
  if (pref !== "corp") decks.runner = crDeck("runner") || null;
  if (pref !== "runner") decks.corp = crDeck("corp") || null;
  $("crlobby-status").textContent =
    pref ? `finding a game as ${pref}…` : "finding an opponent…";
  send({
    type: "lobby-anyone",
    decks,
    side: pref || (Math.random() < 0.5 ? "runner" : "corp"),
    timing: crTiming(),
  });
}
$("crlobby-play").onclick = () => crFindGame(null);
$("crlobby-create-runner").onclick = () => crFindGame("runner");
$("crlobby-create-corp").onclick = () => crFindGame("corp");
$("crlobby-cancel").onclick = () => {
  crWaitToken = null;
  crWaitId = null;
  localStorage.removeItem("jinteki_local");
  send({ type: "lobby-cancel" });
  $("crlobby-mine").textContent = "";
  crLobbySeated(false);
  $("crlobby-status").textContent = "open games";
};

/* While you are seated and waiting, the create verbs make no sense — the
   only verb is Cancel. One function owns the swap so the two states cannot
   drift. */
function crLobbySeated(seated) {
  $("crlobby-cancel").style.display = seated ? "" : "none";
  $("crlobby-play").style.display = seated ? "none" : "";
  $("crlobby-create-runner").style.display = seated ? "none" : "";
  $("crlobby-create-corp").style.display = seated ? "none" : "";
}

function crCreate(side) {
  const seed = seedFromBox("cr-seed");
  if (seed === false) return;
  send({
    type: "lobby-create",
    side,
    seed,
    deck: crDeck(side),
    timing: crTiming(),
  });
}

/* Your own seat, taken, waiting for someone to take the other. The token is
   stored exactly like a game's, so a refresh loses nothing (but a CLOSED
   socket withdraws the seat — a dead socket's invitation is a lie). */
function crWaiting(m) {
  mode = "cr";
  crPairingClear();
  crWaitToken = m.token || crWaitToken;
  crWaitId = (m.lobby || {}).gameid || crWaitId;
  if (crWaitToken) {
    localStorage.setItem("jinteki_local",
      JSON.stringify({ token: crWaitToken, side: m.side || (m.lobby || {}).side, engine: "cr" }));
  }
  show("screen-cr-lobby");
  const g = m.lobby || {};
  $("crlobby-status").textContent = "waiting for an opponent…";
  crLobbySeated(true);
  const box = $("crlobby-mine");
  box.textContent = "";
  const row = el("div", "lobby-row");
  const t = el("div", "t");
  t.appendChild(el("b", "", g.title || "your game"));
  const deck = g["deck-name"] || m.deck || "";
  const wtiming = g["timing-label"] ? ` · ${g["timing-label"]}` : "";
  t.appendChild(el("small", "",
    `you are the ${g.side || "?"} — ${deck}${wtiming} · waiting for the ${g["open-side"] || "?"}`));
  row.appendChild(t);
  row.appendChild(el("span", "chip", "waiting"));
  box.appendChild(row);
  send({ type: "lobby-list" });
}

/* ── the ready check ─────────────────────────────────────────────────────
   Two players at one table, each with a Ready toggle; the countdown is the
   server's voice (`pairing.count`), rendered huge and centered. */
function crPairingClear() {
  crPairing = null;
  $("crlobby-pairing").style.display = "none";
  $("crlobby-pairing").textContent = "";
  $("crlobby-count").style.display = "none";
}

function crPairingRender(m) {
  mode = "cr";
  crPairing = m.pairing || {};
  crWaitToken = null;
  crWaitId = null;
  const mine = (crPairing.seats || []).find((s) => s.you) || {};
  if (m.token) {
    localStorage.setItem("jinteki_local",
      JSON.stringify({ token: m.token, side: mine.side, engine: "cr" }));
  }
  show("screen-cr-lobby");
  $("crlobby-mine").textContent = "";
  $("crlobby-cancel").style.display = "none";
  $("crlobby-status").textContent = "ready check";

  const box = $("crlobby-pairing");
  box.textContent = "";
  box.style.display = "";
  box.appendChild(el("b", "pairing-title", crPairing.title || "your game"));
  // The timing the host chose: the joiner reads it here, and readying up
  // is consenting to it.
  if (crPairing["timing-label"]) {
    box.appendChild(el("small", "pair-timing", `Timing: ${crPairing["timing-label"]}`));
  }
  // CR 1.4.2 at the countdown's door: a refused deck reads its problems
  // here — the same sentences the deck builder shows.
  if (crPairing.refusal) {
    const r = crPairing.refusal;
    const strip = el("div", "pair-refusal");
    strip.appendChild(el("b", "", r.message || "This deck cannot be played."));
    (r.problems || []).forEach((pb) => {
      strip.appendChild(el("small", "", pb.message || pb.code || ""));
    });
    box.appendChild(strip);
  }
  (crPairing.seats || []).forEach((s) => {
    const row = el("div", "pair-row" + (s.you ? " me" : ""));
    const t = el("div", "t");
    t.appendChild(el("b", "", `${s.name || "?"}${s.you ? " (you)" : ""}`));
    t.appendChild(el("small", "", `${s.side} · ${crDeckName(s.side, s.deck) || s["deck-name"] || ""}`));
    row.appendChild(t);
    row.appendChild(el("span", "chip" + (s.ready ? " ready" : ""), s.ready ? "READY" : "not ready"));
    box.appendChild(row);
  });
  const actions = el("div", "pair-actions");
  const ready = el("button", "big " + (mine.ready ? "" : "go"), mine.ready ? "Unready" : "Ready");
  ready.onclick = () => send({ type: "lobby-ready", ready: !mine.ready });
  actions.appendChild(ready);
  const leave = el("button", "chip danger", "Leave");
  leave.onclick = () => {
    localStorage.removeItem("jinteki_local");
    crPairingClear();
    send({ type: "lobby-cancel" });
    $("crlobby-status").textContent = "open games";
  };
  actions.appendChild(leave);
  box.appendChild(actions);
  crCountRender(crPairing.count);
}

/* 5…1, big and centered, with the one way out that is honest: unready.
   The overlay's nodes are built ONCE and only the digit changes: a tick
   must never replace the Cancel button mid-press, or the press lands on a
   node that no longer exists and the click is silently swallowed. */
function crCountRender(n) {
  const o = $("crlobby-count");
  if (n == null) { o.style.display = "none"; return; }
  let num = o.querySelector(".count-num");
  if (!num) {
    o.textContent = "";
    num = el("div", "count-num");
    o.appendChild(num);
    const cancel = el("button", "chip cancel-count", "Cancel");
    cancel.onclick = () => send({ type: "lobby-ready", ready: false });
    o.appendChild(cancel);
  }
  if (num.textContent !== String(n)) {
    num.textContent = String(n);
    // Restart the pulse for each spoken number.
    num.style.animation = "none";
    void num.offsetWidth;
    num.style.animation = "";
  }
  o.style.display = "flex";
}

/* The table dissolved under us (the other player left, or the gate closed
   between the count and the start): back to the list, honestly. */
function crPairingGone() {
  if (!crPairing) return;
  crPairingClear();
  localStorage.removeItem("jinteki_local");
  toast("The table broke up — back to the lobby");
  $("crlobby-status").textContent = "open games";
}

function renderCrLobbies(list) {
  const box = $("crlobby-list");
  box.textContent = "";
  // Your own seat is shown above, not offered back to you as a join.
  list = list.filter((g) => g.gameid !== crWaitId);
  // What "Play anyone" reaches for: the same list the rows are drawn from,
  // so the button and the screen can never disagree about what is open.
  CR_LOBBIES = list;
  if (!crWaitToken && !crPairing) $("crlobby-status").textContent =
    list.length ? `${list.length} open game${list.length === 1 ? "" : "s"}` : "open games";
  list.forEach((g) => {
    const row = el("div", "lobby-row");
    const t = el("div", "t");
    t.appendChild(el("b", "", g.title || "eternal game"));
    const age = Math.max(0, g["age-seconds"] | 0);
    const ago = age < 60 ? `${age}s ago` : age < 3600 ? `${Math.round(age / 60)}m ago`
      : `${Math.round(age / 3600)}h ago`;
    const hostDeck = crDeckName(g.side, g.deck) || g["deck-name"] || "";
    const timing = g["timing-label"] ? ` · ${g["timing-label"]}` : "";
    t.appendChild(el("small", "",
      `${g.creator || "?"} as ${g.side || "?"} (${hostDeck}) · needs a ${g["open-side"] || "?"}${timing} · ${ago}`));
    row.appendChild(t);
    const join = el("button", "chip go", "Join");
    join.onclick = () => {
      $("crlobby-status").textContent = "joining…";
      send({ type: "lobby-join", gameid: g.gameid, deck: crDeck(g["open-side"]) });
    };
    row.appendChild(join);
    box.appendChild(row);
  });
  if (!list.length) {
    box.appendChild(el("div", "lobby-row", "No open games — Play anyone, or create one."));
  }
}

$("btn-local").onclick = () => {
  // Before the socket, for the same reason as the CR door above.
  const seed = seedFromBox("seed");
  if (seed === false) return;
  mode = "local";
  connect("/ws/local", () => {
    send({
      type: "start",
      side: mySide,
      seed,
      deck_id: selectedDeck ? selectedDeck.id : undefined,
    });
  });
};

$("btn-bridge").onclick = () => {
  mode = "bridge";
  show("screen-lobby");
  $("lobby-status").textContent = "connecting…";
  const creds = {
    host: $("ref-host").value.trim(),
    username: $("ref-user").value.trim(),
    password: $("ref-pass").value,
  };
  localStorage.setItem("jinteki_bridge", JSON.stringify(creds));
  connect("/ws/bridge", () => {
    send({ type: "connect", ...creds });
  });
};

/* ── session restore on load ─────────────────────────────────────────── */
(function restore() {
  const bridge = JSON.parse(localStorage.getItem("jinteki_bridge") || "null");
  if (bridge) {
    $("ref-host").value = bridge.host || "http://localhost:1042";
    $("ref-user").value = bridge.username || "";
    $("ref-pass").value = bridge.password || "";
  }
  const saved = JSON.parse(localStorage.getItem("jinteki_local") || "null");
  if (saved && saved.token) {
    mode = saved.engine === "cr" ? "cr" : "local";
    if (saved.side) mySide = saved.side;
    connect("/ws/local", () => send({ type: "resume", token: saved.token }));
  }
  loadCrReady();
})();

$("lobby-back").onclick = () => { if (ws) ws.close(); show("screen-home"); };
$("lobby-refresh").onclick = () => send({ type: "lobbies" });
$("create-corp").onclick = () => { amHost = true; send({ type: "create", side: "Corp", title: $("new-title").value }); };
$("create-runner").onclick = () => { amHost = true; send({ type: "create", side: "Runner", title: $("new-title").value }); };
$("lobby-start").onclick = () => send({ type: "start" });

function renderLobbies(list) {
  const box = $("lobby-list");
  box.innerHTML = "";
  list.forEach((g) => {
    const row = document.createElement("div");
    row.className = "lobby-row";
    const players = (g.players || []).map((p) => `${(p.user && p.user.username) || "?"} (${p.side || "?"})`).join(" vs ");
    row.innerHTML = `<div class="t"><b>${g.title || "game"}</b><small>${players || "empty"} · ${g.format || ""} ${g.started ? "· in progress" : ""}</small></div>`;
    const join = document.createElement("button");
    join.className = "chip go"; join.textContent = g.started ? "Watch" : "Join";
    join.onclick = () => {
      lobbyGameid = g.gameid;
      send({ type: g.started ? "watch" : "join", gameid: g.gameid });
      if (!g.started) $("lobby-status").textContent = "joined — pick a deck";
    };
    row.appendChild(join);
    box.appendChild(row);
  });
  if (!list.length) box.innerHTML = `<div class="lobby-row"><div class="t">No open games — create one.</div></div>`;
}

function renderLobbyState(lobby) {
  if (!lobby) return;
  lobbyGameid = lobby.gameid || lobbyGameid;
  const started = !!lobby.started;
  const nplayers = (lobby.players || []).length;
  $("lobby-status").textContent = started
    ? "game started"
    : `in lobby: ${nplayers}/2 players`;
  $("lobby-start").style.display = amHost && nplayers === 2 && !started ? "" : "none";
  if (!started && nplayers >= 1) send({ type: "decks" });
}

function renderDecks(list) {
  const box = $("deck-picker");
  if (!Array.isArray(list) || !list.length) { box.style.display = "none"; return; }
  box.style.display = "";
  box.innerHTML = `<b>Pick a deck</b>`;
  list.slice(0, 40).forEach((d) => {
    const b = document.createElement("button");
    b.className = "chip";
    const idt = d.identity && (d.identity.title || d.identity) || "";
    // [untested]: every card of this deck is complete and behaviour-tested,
    // but nobody has played the DECK end to end. Said out loud, next to the
    // name, so choosing it is the player's call and not a silent gamble.
    b.textContent = `${d.name || "deck"}${d.untested ? " [untested]" : ""} — ${idt}`;
    if (d.untested) b.classList.add("untested");
    b.onclick = () => { send({ type: "deck", "deck-id": d._id }); box.style.display = "none"; };
    box.appendChild(b);
  });
}

/* ── rendering the board ─────────────────────────────────────────────── */
function me() { return S[mySide] || {}; }
function opp() { return S[mySide === "corp" ? "runner" : "corp"] || {}; }
function myPrompt() { return me()["prompt-state"]; }
function isSelectMode() {
  const p = myPrompt();
  return p && (p.select === true || p["prompt-type"] === "select");
}
/* Is this card a legal target for the select question being asked? The
   candidate list is the server's, and it is the SAME list the prompt row is
   drawn from, so the board's gold and the sheet's gold can never disagree
   (UX.md THE LAW §3). In bridge mode there is no list — the reference server
   is the authority there, so any card may be offered. */
function isSelectCandidate(cid) {
  if (!isSelectMode()) return false;
  if (mode === "bridge") return true;
  const p = myPrompt();
  const cards = p["select-cards"];
  if (Array.isArray(cards)) return cards.some((c) => c.cid === cid);
  return ACTIONS.some((a) => a.command === "select" && a.cid === cid);
}
function actionsFor(cid) { return ACTIONS.filter((a) => a.cid === cid); }

/* THE LAW §3, in one place, so nothing that draws a card can disagree with
   anything else that draws the same card:
     GOLD  (.selectable) — a legal TARGET for the question being asked
     GREEN (.usable)     — an ability on this card you can use RIGHT NOW
     TEAL  (.legal)      — an ordinary action is available on it
   A target is the only question on the table while it is being asked, so a
   candidate is gold and never green. Identities are cards and go through
   this too — their chip in the seat rail is the only copy of them a phone
   draws at all, so if the ladder lived only inside `cardEl` an identity's
   own ability would be usable nowhere. */
function glowClass(cid) {
  if (cid == null) return "";
  if (isSelectMode()) {
    if (!isSelectCandidate(cid)) return "";
    const p = myPrompt() || {};
    // WHITE (.staged) — marked by you, not yet done, still yours to take
    // back. The third colour, and the same meaning it has on an identity:
    // the game is waiting on your word.
    if ((p["select-picked"] || []).includes(cid)) {
      // A staged card is WHITE and nothing else — `.picked` is the gold
      // "counted towards the announcement" mark, and gold over white would
      // say both things at once.
      return p["select-kind"] === "discard" ? "staged" : "selectable picked";
    }
    return "selectable";
  }
  if (promptChoicesFor(cid).length) {
    // THE LAW §3: gold is a legal TARGET, green an ability. A choice that
    // lives on a card is usually "use this ability" — green — but when the
    // decision is a destination (install onto which host?) the card is a
    // target, and the server says so (`target-choices`).
    return (myPrompt() || {})["target-choices"] ? "selectable" : "usable";
  }
  // GREEN is "an ABILITY on this card you can use right now", and a card
  // ability whose cost starts with [click] is exactly that — CR 5.2.1 calls
  // it an action, but it is still the card's own printed ability, and the
  // player reading the board is asking "can I use this card", not "which CR
  // window is this". Jackson Howard's "[click]: Draw 2 cards" used to draw
  // the same teal as "you may install this from HQ", while the card went
  // GREEN only in the paid window where its one option was to remove itself
  // from the game — so green meant "exile me" on the very card whose click
  // ability the player was hunting for. Teal keeps its own job: an ordinary
  // action ABOUT the card (play it, install it, advance it, rez it).
  const acts = actionsFor(cid);
  if (acts.some((a) => a.command === "ability")) return "usable";
  if (acts.length) return "legal";
  return "";
}

/* ── the BOARD QUESTION: a decision whose candidates the board is already
   drawing — installed cards, cards in your own hand, servers. For these the
   board is the prompt: candidates gold, the armed one WHITE, the second tap
   commits. NO sheet appears — the sentence that used to float over the board
   lives in the game log ("… choosing target for install (Simulchip)"), and
   the choices that are not places on the board (Pass, Your rig, Done) dock
   in the bottom action rail. */
function boardQuestion(p) {
  if (!p || p["prompt-type"] === "waiting") return false;
  if (p.card && p.card.title) return false;          // a reader, not a reminder
  if (p.arrange && p.arrange.length > 1) return false;
  if (p.select) return p["select-onboard"] === true;
  // A §9.2 window: every offer is a verb and a card, so the cards answer it
  // — on the board where the board shows their face, in the effects rail
  // where it does not (an unrezzed asset, the Corp's own facedown agenda).
  // The pass docks in the action rail with the other things that end a
  // window. Nothing here is a sentence, so nothing here needs a sheet.
  if (p["window-cards"] === true) return true;
  if (p["choices-onboard"] === true) return true;
  // A server choice: every server is on the board by definition.
  if ((p.choices || []).some((ch) => ch.server)) return true;
  return false;
}

/* CR 4.6.6: a server's ROOT, drawn as one TIGHT STACK — newest on top.

   The root is not a list. At the table you slide the new card onto the ones
   already there, and what you see is a stack with the last card face-on and
   the earlier ones peeking out from under it. A column of separated cards
   said something false about the game (that these are four places) and cost
   the vertical budget four times over, which on a landscape phone is the
   budget that runs out first.

   The tuck leaves the TOP sliver of every covered card showing: enough for
   its cost disc, its counters and the first line of its name, so the stack
   is countable and each member is nameable and tappable. Later siblings are
   positioned and paint over earlier ones by DOM order, so "newest on top"
   needs no z-index at all — only the ARMED card is lifted, because that is
   the one the next tap acts on and it has to be unmistakable.

   The cards the board can only draw as backs are ALSO in the effects rail
   whenever the game is asking about them (THE LAW §1), so the sliver is
   never the only way to reach an unreadable card. */
function rootStack(content) {
  const box = el("div", "root-stack");
  // A card hosted on one of these is drawn ON it (CR 1.13.1), not as another
  // card in the root — the root's tuck would bury it, and it was never
  // installed there.
  const own = (content || []).filter((c) => !drawnElsewhere(c));
  own.forEach((c) => box.appendChild(hostBox(c, { side: "corp" })));
  // A single card is a card, not a stack: no tuck, no class to reason about.
  if (own.length > 1) box.classList.add("tucked");
  return box;
}

/* ── WHICH SERVER? IS A QUESTION ABOUT SERVERS (THE LAW §3) ──────────────
 *
 * "Run ▾" used to open a list: one button per server, at a fixed point near
 * the bottom of the screen, in a sheet with no bound and no scroll. A corp
 * with five remotes therefore asked the Runner to choose from a list that
 * ran off the bottom of the phone and could not be reached — the servers
 * past the fourth were on screen, glowing, and unreachable through the very
 * control that was supposed to pick them.
 *
 * The board already knows how to ask this. Every other question about a
 * server is asked ON the servers — gold for a candidate, white for the one
 * the next tap acts on, second tap commits — and the run is the most
 * server-shaped question in the game. So "Run" now ARMS the board: the
 * runnable servers light up, one tap focuses, the next runs, and "Cancel"
 * (and tapping the table, and Escape) says never mind.
 *
 * A Map of the board's own server keys to the act that runs them, or null
 * when no run is being chosen. It is client-side state: the server offered
 * these runs already and is not waiting on anything. */
let runPick = null;
function setRunPick(m) {
  runPick = m;
  setArmed(null);
  if (S) render();
}
function cancelRunPick() { if (runPick) setRunPick(null); }

/* The uuid choices for one server column ("Server 2", "Protecting Server 2").
   Keys are the board's own server keys; "new" is the remote that does not
   exist yet, drawn as a placeholder column. */
function serverChoices() {
  const p = myPrompt();
  const m = new Map();
  if (!p || p["prompt-type"] === "waiting") return m;
  (p.choices || []).forEach((ch) => {
    if (!ch.server) return;
    if (!m.has(ch.server)) m.set(ch.server, []);
    m.get(ch.server).push(ch);
  });
  return m;
}

/* What the armed thing is CALLED, for the rail's confirm hint.

   Where the armed card carries exactly ONE offered option, the hint names
   the OPTION and not the card: "Score AstroScript Pilot Program", not
   "AstroScript Pilot Program". 9.2.7f makes a chosen option resolve to the
   end, so the tap that takes it is the last moment anything can be called
   off — and a gate that says "tap again" without saying to WHAT is not a
   gate (the Jackson Howard trap: its only paid-window option removes it from
   the game, on a card players reach for to draw two). Naming the act is what
   makes the ring a gate, which is why a card with SEVERAL options still gets
   a sheet: one ring cannot name two acts. */
function armedName() {
  if (armed == null) return "";
  if (typeof armed === "string" && armed.startsWith("srv:")) {
    const k = armed.slice(4);
    return k === "new" ? "a new remote" : SERVER_NAME(k);
  }
  const p = myPrompt() || {};
  const offered = promptChoicesFor(armed);
  if (offered.length === 1) {
    const ch = offered[0];
    return abilityText(ch.value, ch.card && ch.card.title, false) || "this card";
  }
  const inSel = (p["select-cards"] || []).find((c) => c.cid === armed);
  if (inSel) return inSel.title || "the facedown card";
  const inCh = (p.choices || []).find((ch) => ch.cid === armed && ch.card);
  if (inCh) return inCh.card.title || "the facedown card";
  const el2 = document.querySelector(`.card[data-cid="${armed}"] .cname`);
  return (el2 && el2.textContent) || "this card";
}

/* WHITE — a third thing, and not one of the three above (UX.md THE LAW §3).
   Gold and green answer "what can I do"; white answers "is the game waiting
   on ME". It rides the identities, both of them, continuously: the seat whose
   identity is shimmering white owes the next word, including when that word
   is just which action to take on their own turn. */
function hasPriority(side) { return !!S && S.priority === side; }

function dirty(key, val) {
  const s = JSON.stringify(val);
  if (sectionCache[key] === s) return false;
  sectionCache[key] = s;
  return true;
}

/* One section of the board. A renderer that throws must not take the rest of
   the frame with it: the prompt sheet is the most data-driven thing here, and
   for a while a shadowed variable in it meant the End Turn button, the run
   controls and the log all silently stopped redrawing behind an exception
   nobody saw. A half-drawn board you can still act on beats a frozen one. */
const renderFailed = new Set();
function section(name, fn) {
  try { fn(); }
  catch (e) {
    console.error(`render(${name}) failed`, e);
    if (!renderFailed.has(name)) { renderFailed.add(name); toast(`Display error in ${name} — the game is fine`); }
  }
}

function render() {
  if (!S) return;
  // Seat orientation: YOUR territory renders on YOUR half of the board,
  // adjacent to your bar and hand; the opponent's on theirs.
  $("board").classList.toggle("flipped", mySide === "corp");
  // A preview whose card was just redrawn out from under the pointer has no
  // `mouseleave` coming: reap it here, before anything rebuilds a subtree.
  section("hover", reapHoverPreview);
  section("bars", renderBars);
  section("timing", renderTiming);
  section("servers", () => {
    // `runPick` is in the key because it is a QUESTION the servers are being
    // asked (which one do you want to run?) — the columns wear the gold for
    // it exactly as they do for a question the server sent, and a key that
    // does not name it leaves the board silent while the rail says "tap the
    // server you want".
    if (dirty("servers", [(S.corp || {}).servers, S.run, ACTIONS, myPrompt(), S.priority, armed,
      runPick && [...runPick.keys()].join(",")])) renderServers();
  });
  section("rig", () => {
    if (dirty("rig", [(S.runner || {}).rig, ACTIONS, myPrompt(), S.priority, armed])) renderRig();
  });
  // The chrome that shares the bottom of the screen goes FIRST: the hand
  // measures the band those rails leave it (§8), and measuring last frame's
  // rails would lay the cards out for a bar that is no longer there.
  section("actions", renderChips);
  section("run controls", renderRunControls);
  section("play area", renderPlayRail);
  section("hand", () => {
    if (dirty("hand", [me().hand, raised, ACTIONS, myPrompt(), armed])) renderHand();
  });
  section("the prompt", renderPrompt);
  section("access", renderAccessReveal);
  section("end turn", renderTurnBtn);
  section("log", renderLog);
  section("phase", renderPhasePill);
  section("focus", renderFocus);
  section("game over", renderGameOver);
  // Last: it reads whatever the fans above left focused, and a prompt that
  // has just closed must not leave its card on the right-hand panel.
  section("preview", paintFanPreview);
}

/* A rotation changes which fan geometry applies (the card shrinks below 640px
   of height) and whether the right-hand preview fits at all, and neither is
   something a state push will come along and fix. Debounced, because iOS
   fires `resize` several times through the rotation animation. */
let relayoutTimer = null;
window.addEventListener("resize", () => {
  clearTimeout(relayoutTimer);
  relayoutTimer = setTimeout(() => {
    if (!S) return;
    // Bars FIRST: the seat rail's box is in vh and its base font is a media
    // query, so a rotation changes both — and the hand measures the band the
    // rails leave it, so it must see the rail's new size, not last frame's.
    section("bars", renderBars);
    section("hand", renderHand);
    section("the prompt", renderPrompt);
    section("preview", paintFanPreview);
  }, 150);
});

function statBump(key, val, elm) {
  if (prev[key] !== undefined && prev[key] !== val) elm.classList.add("bump");
  prev[key] = val;
}

function sideStats(st, side) {
  const tags = st.tag ? (st.tag.total ?? st.tag.base ?? 0) : 0;
  const bp = st["bad-publicity"] ? (st["bad-publicity"].base ?? 0) : 0;
  const mu = st.memory ? ` · MU ${st.memory.available ?? "?"}/${st.memory.limit ?? (st.memory.base ?? 4)}` : "";
  const extra = side === "runner" ? `${tags ? " · TAG " + tags : ""}${mu}` : (bp ? ` · BP ${bp}` : "");
  return { extra };
}

function renderBars() {
  const top = $("opp-bar"), bot = $("my-bar");
  const o = opp(), m = me();
  const oSide = mySide === "corp" ? "runner" : "corp";
  // Two people at one table: a seat with nobody in it says so, rather than
  // looking like an opponent who is thinking very hard.
  const gone = S["opponent-connected"] === false;
  const who = (o.user && o.user.username) || "opponent";
  // A waiting prompt IS the opponent deciding — the sheet that used to say
  // so is gone (the log says it now), so the pulse must carry it alone.
  const waitingP = !!myPrompt() && myPrompt()["prompt-type"] === "waiting";
  const thinking = gone
    ? `<span class="thinking offline">${esc(who)} disconnected — game held</span>`
    : (!S.winner && (waitingP || (S["active-player"] === oSide && !myPrompt())) ? `<span class="thinking">thinking…</span>` : "");
  top.innerHTML = barHtml(o, oSide, true) + thinking;
  bot.innerHTML = barHtml(m, mySide, false);
  fitSideStats();
  const credEl = bot.querySelector(".cred");
  if (credEl) statBump("mycred", m.credit, credEl);
  const oc = top.querySelector(".cred");
  if (oc) statBump("oppcred", o.credit, oc);
}

/* ── every stat of each side FITS (UX.md lesson 13) ──────────────────────
   The seat rail lives in a fixed strip the board's inset already reserves,
   so its box is not negotiable — and neither is its content: a clipped
   Archives count or an identity cut to "Nebula T…" is information a player
   then plays without. Everything inside the rail is sized in em off the
   bar's own font, so ONE number decides whether it all fits; this measures
   the stack against the stylesheet's own max-width/max-height (the box IS
   the budget — reading it here means the two can never disagree) and
   writes that number back as `--sscale`. Down-scale only, floored at 9px:
   below the floor the rail switches to `.cram` — tightest packing, the
   name folding to two lines — and refits, because a smear nobody can read
   drops information exactly as surely as clipping it did. The same shape
   as the fan's `fit.step`: measure the room, write one custom property,
   let the stylesheet place what the arithmetic sized. Runs on every state
   push (digit counts change width) and on resize (the box is in vh). */
const STATS_FLOOR_PX = 9;
function fitSideStats() {
  for (const id of ["opp-bar", "my-bar"]) {
    const bar = document.getElementById(id);
    if (!bar || !bar.firstElementChild) continue;
    bar.classList.remove("cram");
    bar.style.setProperty("--sscale", "1");
    const cs = getComputedStyle(bar);
    const boxW = parseFloat(cs.maxWidth) || bar.clientWidth || 1;
    const boxH = parseFloat(cs.maxHeight) || Infinity;
    const base = parseFloat(cs.fontSize) || 11.5;
    const floor = Math.min(1, STATS_FLOOR_PX / base);
    let scale = fitStatsScale(bar, boxW, boxH, 0);
    if (scale < floor) {
      // The floor is where shrinking stops being reading: switch to the
      // tightest packing and fit THAT. In cram the name's clamp is chased
      // only down to the floor — its two-line ellipsis is the deal the
      // floor struck — but the BOX is chased wherever it leads, below the
      // floor if it must be: a stat clipped by the box is dropped outright,
      // and an 8px stat still beats an absent one. In practice the floor
      // yields only on viewports far shorter than any phone held either way.
      bar.classList.add("cram");
      bar.style.setProperty("--sscale", "1");
      scale = fitStatsScale(bar, boxW, boxH, floor);
    }
    bar.style.setProperty("--sscale", String(Math.round(scale * 1000) / 1000));
  }
}
/* The content is em-sized, so its need shrinks about linearly with the
   scale: a few passes of measure-and-ratio converge, and every pass reads
   the layout the stylesheet actually produced rather than predicting it.
   (`scrollWidth`/`scrollHeight` state the whole need even though the bar's
   `overflow: hidden` is clipping the excess while we measure.)

   The one need the bar cannot state is the clamped name's: `.cram`'s
   two-line clamp swallows its own overflow, so the bar measures as fitting
   while the name is quietly losing its tail. The element itself still
   knows (`scrollHeight` past the clamp), so a clamped name counts as
   overflow too — and since no ratio can be read off a clamp, the scale
   steps down a notch at a time until the two lines hold the whole name, or
   the floor is reached and the ellipsis is finally earned. */
function fitStatsScale(bar, boxW, boxH, whoFloor) {
  let scale = 1;
  for (let i = 0; i < 8; i++) {
    const boxOver = bar.scrollWidth > boxW + 1 || bar.scrollHeight > boxH + 1;
    const who = bar.querySelector(".who");
    const whoOver = !!who && who.scrollHeight > who.clientHeight + 1 && scale > whoFloor + 0.001;
    if (!boxOver && !whoOver) break;
    const byBox = Math.min(boxW / Math.max(1, bar.scrollWidth), boxH / Math.max(1, bar.scrollHeight));
    // A box overflow states its own ratio; a clamped name states nothing
    // (the clamp swallows the overflow), so it is chased a notch at a time.
    scale = Math.max(0.1, scale * (boxOver && byBox < 0.999 ? byBox : 0.93));
    if (!boxOver) scale = Math.max(scale, whoFloor);
    bar.style.setProperty("--sscale", String(Math.round(scale * 1000) / 1000));
  }
  return scale;
}

function barHtml(st, side, isOpp) {
  const idt = st.identity || {};
  let name = (idt.title || side).split(":")[0];
  // Across the table sits a person, when it is a person: say who.
  const uname = st.user && st.user.username;
  if (isOpp && uname && uname !== "bot" && uname !== "you") name += ` · ${esc(uname)}`;
  const s = sideStats(st, side);
  const clicks = "●".repeat(Math.max(0, st.click || 0)) || "–";
  const art = idt.code ? ` style="background-image:url(${cardImgUrl(idt.code)})"` : "";
  // The identity IS a card, and the seat rail's chip is the only copy of it
  // that is drawn in every layout — the identity card column is hidden
  // outright on a phone (`.identity-col { display: none }` under 640px), so
  // the chip is the one that has to carry both signals:
  //   white = this seat has priority (whose decision it is, continuously)
  //   green = this identity has an ability that can be used right now
  // Both come from the same places every other card's do, so they cannot
  // say something the board does not.
  const glow = idt.cid != null ? glowClass(idt.cid) : "";
  const chip = ["idchip", hasPriority(side) ? "priority" : "", glow,
    idt.cid != null && armed === idt.cid ? "armed" : ""]
    .filter(Boolean).join(" ");
  // The OPPONENT's main clock rides their seat rail (near their identity —
  // your own lives in the bottom-right time cluster). Text and classes are
  // written by `timingTick`, off the same snapshot as everything timed.
  const clock = isOpp && S.timing && S.timing.main
    ? `<span class="stat clock" data-clock="${side}">⏱ --:--</span>`
    : "";
  // MTGA-style corner cluster: tappable identity art + compact stat chips.
  return `
    <span class="${chip}" data-side="${side}"><span class="idthumb"${art}></span><span class="who">${name}</span></span>
    ${clock}
    <span class="stat cred" title="credits">⬡ ${st.credit ?? 0}</span>
    <span class="stat" title="clicks remaining">${clicks}</span>
    <span class="stat zones">
      <span title="${ZONE(side, "hand").hint}">${ZONE(side, "hand").label} ${st["hand-count"] ?? (st.hand || []).length}</span>
      <span title="${ZONE(side, "deck").hint}">${ZONE(side, "deck").label} ${st["deck-count"] ?? 0}</span>
      <span class="tappable" data-stat="discard" data-side="${side}"
        title="${ZONE(side, "discard").hint}">${ZONE(side, "discard").label} ${(st.discard || []).length}</span>
    </span>
    <span class="stat tappable" data-stat="ap" data-side="${side}"
      title="agenda points — tap to see the agendas">AP ${st["agenda-point"] ?? 0}${s.extra}</span>`;
}

/* ── the game timer (timed games only; untimed games show none of this) ──
 *
 * Server-authoritative: the server owns every deadline and pushes the
 * remaining times with each state (plus a 1s sync while anything burns); the
 * client only counts down LOCALLY from the last snapshot, so a hostile or
 * confused client can change nothing — at worst it displays a lie to itself.
 *
 * Three displays, per the law of the overlay (UX.md THE LAW §2 — nothing
 * reflows):
 *   · the OPPONENT's main clock: a chip on their seat rail (near identity).
 *   · your OWN cluster, bottom-right corner: tiny main clock, the rope bar
 *     while a rope of YOURS burns, your calm bank when it is running low,
 *     and your banked ⌛ count. Your own only — the opponent's ⌛ count is
 *     never shown (nor sent).
 *   · the running clock is visually alive; red under a minute. The rope is
 *     subtle until its last 3 seconds.
 *
 * THE ROPE IS NOT DRAWN WHILE THE BANK IS POSITIVE. It is a reservoir
 * (`timing.rs`): a player who is playing never empties it, and a countdown
 * shown to a player who is not in trouble is just nagging — so nothing at
 * all appears until the bank is nearly gone, and the bar and the vignette
 * only when `visible` says the rope is genuinely burning. */
function renderTiming() {
  const cl = $("time-cluster");
  const t = S && S.timing;
  if (!t) { cl.style.display = "none"; return; }
  cl.style.display = "flex";
  if (!cl.firstChild) {
    cl.innerHTML =
      `<span class="ttokens" style="display:none">⌛0</span>` +
      `<span class="tbank" style="display:none">⏳--</span>` +
      `<span class="tfuse" style="display:none"><i></i></span>` +
      `<span class="stat clock tclock" style="display:none">⏱ --:--</span>`;
  }
  const clock = cl.querySelector(".tclock");
  clock.style.display = t.main ? "" : "none";
  clock.setAttribute("data-clock", mySide);
  // Banked timeouts (⌛): rope games only; the count is already yours-only.
  const tok = cl.querySelector(".ttokens");
  if (t.timeouts != null) {
    tok.style.display = "";
    tok.textContent = `⌛${t.timeouts}`;
    tok.classList.toggle("some", t.timeouts > 0);
  } else {
    tok.style.display = "none";
  }
  timingTick();
}

function fmtClock(ms) {
  const s = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/* The local countdown between server snapshots. Writes text, width and
   classes only — never layout (THE LAW §2). */
function timingTick() {
  if (!S || !S.timing || !TIMING) return;
  const t = TIMING.d;
  const dt = performance.now() - TIMING.at;
  const over = !!S.winner;
  if (t.main) {
    for (const side of ["corp", "runner"]) {
      let ms = t.main[side + "_ms"] ?? 0;
      const running = !over && t.main.running === side;
      if (running) ms = Math.max(0, ms - dt);
      document.querySelectorAll(`[data-clock="${side}"]`).forEach((el) => {
        el.textContent = `⏱ ${fmtClock(ms)}`;
        el.classList.toggle("live", running);
        el.classList.toggle("low", ms < 60_000);
      });
    }
  }
  // The reservoir: YOUR calm bank draining, and — only once it is gone —
  // your rope burning down. The cluster's small copy, the unmissable
  // mid-screen bar and the rim of the screen breathing dark red all belong
  // to the ROPE, and none of them exists while the bank is positive. The
  // opponent's reservoir is their problem; every one of these shows YOURS.
  const bankEl = document.querySelector("#time-cluster .tbank");
  const fuse = document.querySelector("#time-cluster .tfuse");
  const mid = $("rope-mid");
  const rim = $("rope-vignette");
  if (!bankEl || !fuse || !mid || !rim) return;
  const hideAll = () => {
    bankEl.style.display = "none";
    fuse.style.display = "none";
    fuse.classList.remove("alarm");
    mid.style.display = "none";
    rim.style.display = "none";
  };
  const mine = !over && t.rope && t.rope.side === mySide;
  if (!mine) { hideAll(); return; }
  // The local countdown spends the bank first and only then the rope —
  // the same order the server settles them in, so the two never disagree.
  const bank = Math.max(0, t.rope.bank_ms - dt);
  const spill = Math.max(0, dt - t.rope.bank_ms);
  const ropeMs = Math.max(0, t.rope.rope_ms_left - spill);
  const burning = bank <= 0;
  if (!burning) {
    // Calm. Nothing at all, unless the bank is nearly out — then one small
    // number in the corner, which is a warning and not a countdown.
    fuse.style.display = "none";
    fuse.classList.remove("alarm");
    mid.style.display = "none";
    rim.style.display = "none";
    if (bank < BANK_WARN_MS) {
      bankEl.style.display = "";
      bankEl.textContent = `⏳${Math.ceil(bank / 1000)}s`;
      bankEl.classList.remove("alarm");
    } else {
      bankEl.style.display = "none";
    }
    return;
  }
  const alarm = ropeMs < 3000;
  const pct = `${Math.max(0, Math.min(100, (ropeMs / Math.max(1, t.rope.rope_total_ms)) * 100))}%`;
  // On the rope the corner number counts the ROPE, because that is what is
  // about to happen to you. Same chip, same place; the bank is zero anyway.
  bankEl.style.display = "";
  bankEl.textContent = `⏳${Math.ceil(ropeMs / 1000)}s`;
  bankEl.classList.toggle("alarm", alarm);
  fuse.style.display = "";
  fuse.classList.toggle("alarm", alarm);
  fuse.firstElementChild.style.width = pct;
  mid.style.display = "";
  mid.classList.toggle("alarm", alarm);
  mid.firstElementChild.style.width = pct;
  rim.style.display = "";
  rim.classList.toggle("alarm", alarm);
}
/* How little calm time is left before the corner says so. Below this the
   player is about to be roped and would rather know; above it, silence. */
const BANK_WARN_MS = 15000;
setInterval(timingTick, 150);

/* Dev/test door to a timed game (the lobby owns the real config UI):
   `?timing=dev` on the URL, or
   `?timing=main:300,calm:15,opening:20,inc:5,rope:10` (any subset; naming any
   rope knob turns the rope on with defaults for the rest). The names are the
   reservoir's: `calm` is the bank's cap, `opening` what you start holding,
   `inc` what one completed action pays back, `rope` how long the rope burns
   once the bank is spent. Absent = untimed, exactly today's behavior. */
function timingFromQuery() {
  const q = new URLSearchParams(location.search).get("timing");
  if (!q) return undefined;
  if (q === "dev") {
    return {
      main_clock_secs: 300,
      rope: { calm_secs: 10, opening_calm_secs: 15, action_increment_secs: 5, rope_secs: 8 },
    };
  }
  const t = {};
  const rope = {};
  q.split(",").forEach((kv) => {
    const [k, v] = kv.split(":");
    const n = parseInt(v, 10);
    if (!Number.isFinite(n)) return;
    if (k === "main") t.main_clock_secs = n;
    if (k === "calm") rope.calm_secs = n;
    if (k === "opening") rope.opening_calm_secs = n;
    if (k === "inc") rope.action_increment_secs = n;
    if (k === "rope") rope.rope_secs = n;
  });
  if (Object.keys(rope).length) t.rope = rope;
  return Object.keys(t).length ? t : undefined;
}

/* Netrunner names each zone once per side, and players use those names and
   no others: the Corp's are HQ / R&D / Archives, the Runner's are the grip /
   the stack / the heap (CR 4.2-4.4 define one zone type per pair). Showing
   "Hand" and "Deck" to a Runner is a translation nobody asked for, so the
   client speaks the printed names. The kernel keeps `Zone::Discard(Side)`
   because the RULE is one rule; only the label differs. */
const ZONE_NAMES = {
  corp:   { hand: ["HQ", "cards in HQ"],
            deck: ["R&D", "cards in R&D"],
            discard: ["Archives", "Archives — tap to look"] },
  runner: { hand: ["Grip", "cards in the grip"],
            deck: ["Stack", "cards in the stack"],
            discard: ["Heap", "the heap — tap to look"] },
};
function ZONE(side, which) {
  const z = (ZONE_NAMES[side] || ZONE_NAMES.runner)[which];
  return { label: z[0], hint: z[1] };
}

// Both discard piles are public information (CR 4.4.2), so either side's is
// readable by either player — the Runner's heap especially, since installing
// and shuffling out of it is ordinary play.
document.addEventListener("click", (e) => {
  // Matched on the ATTRIBUTE, not on `.stat[data-stat=…]`: the discard span
  // lives inside `.stat.zones` and carries `tappable` alone, so the compound
  // selector matched nothing and both discard piles were unopenable — a
  // reader with a `title` promising a tap and no tap behind it. The sibling
  // AP handler happens to work only because that span is itself a `.stat`.
  const stat = e.target.closest('[data-stat="discard"]');
  if (!stat || !S) return;
  const side = stat.dataset.side;
  const st = S[side] || {};
  zoomPile(st.discard || [], `${ZONE(side, "discard").label} (${(st.discard || []).length})`);
});

// Tap an identity chip. It is a card, so it answers like one: an ability
// that is on offer is taken, a legal target is picked, and otherwise the tap
// reads the card — exactly what tapping the identity card itself does,
// through the same `onCardTap`, because a phone draws no identity card.
document.addEventListener("click", (e) => {
  const chip = e.target.closest(".idchip");
  if (!chip || !S) return;
  const st = S[chip.dataset.side];
  if (!st || !st.identity) return;
  const c = st.identity;
  // The identity answers like any card, so it ARMS like any card: when the
  // tap would answer a question (a target pick, an offered ability), the
  // first tap is the white ring and the second is the one that commits.
  const answerable = isSelectCandidate(c.cid) || promptChoicesFor(c.cid).length > 0;
  if (answerable) { armTap(BOARD_ARM, c.cid, () => onCardTap(c, { identity: true }, chip)); return; }
  onCardTap(c, { identity: true }, chip);
});

// Tap AP to see the agendas behind the number (both sides' score areas are
// public information).
document.addEventListener("click", (e) => {
  const stat = e.target.closest('.stat[data-stat="ap"]');
  if (!stat || !S) return;
  const side = stat.dataset.side;
  const st = S[side] || {};
  const who = side === "corp" ? "Corp" : "Runner";
  const verb = side === "corp" ? "scored" : "stolen";
  zoomPile(st.scored || [], `${who} agendas (${verb}) — ${st["agenda-point"] ?? 0} points`);
});

const SERVER_ORDER = (k) => ({ archives: 0, rd: 1, hq: 2 }[k] ?? 10 + parseInt(k.replace("remote", ""), 10));
const SERVER_NAME = (k) => (k === "hq" ? "HQ" : k === "rd" ? "R&D" : k === "archives" ? "Archives" : "Server " + k.replace("remote", ""));

function renderServers() {
  const wrap = $("servers");
  const scroll = wrap.scrollLeft, scrollY = wrap.scrollTop;
  wrap.innerHTML = "";
  const corp = S.corp || {};
  const servers = corp.servers || {};
  const runServer = S.run && S.run.server ? String(S.run.server[0]).replace(":", "") : null;
  const runPos = S.run ? S.run.position : null;
  // A question about SERVERS is asked on the servers (THE LAW §3): the
  // candidate columns wear the gold, the armed one the white ring, and the
  // second tap commits — the same grammar as a card, because a server is a
  // place on the table exactly as a card is.
  const srvChoices = serverChoices();
  // Corp identity gets its own column (the runner's lives in the rig).
  if (corp.identity) {
    const idcol = document.createElement("div");
    idcol.className = "server identity-col";
    const nm = document.createElement("div");
    nm.className = "sname";
    nm.textContent = "Identity";
    idcol.appendChild(nm);
    const idEl = cardEl(corp.identity, { side: "corp", identity: true });
    // The same white the seat rail's chip carries, on the card where the
    // layout is wide enough to draw one. One signal, two places it can land.
    if (hasPriority("corp")) idEl.classList.add("priority");
    idcol.appendChild(idEl);
    wrap.appendChild(idcol);
  }
  Object.keys(servers).sort((a, b) => SERVER_ORDER(a) - SERVER_ORDER(b)).forEach((key) => {
    const srv = servers[key];
    const col = document.createElement("div");
    col.className = "server" + (runServer === key ? " run-target" : "");
    if (srvChoices.has(key)) {
      col.classList.add("selectable");
      if (armed === "srv:" + key) col.classList.add("armed-target");
      wireServerTarget(col, key, srvChoices.get(key));
    } else if (runPick && runPick.has(key)) {
      // A run being chosen is the same question in the same clothes.
      col.classList.add("selectable");
      if (armed === "srv:" + key) col.classList.add("armed-target");
      wireRunTarget(col, key, runPick.get(key));
    }
    const name = document.createElement("div");
    name.className = "sname"; name.textContent = SERVER_NAME(key);
    col.appendChild(name);

    // central box or content
    if (key === "hq" || key === "rd" || key === "archives") {
      const box = document.createElement("div");
      box.className = "central";
      const n = key === "hq" ? (corp["hand-count"] ?? 0) : key === "rd" ? (corp["deck-count"] ?? 0) : (corp.discard || []).length;
      box.innerHTML = `<b>${n}</b><span>${key === "hq" ? "cards" : key === "rd" ? "cards" : "cards"}</span>`;
      // While the column is a candidate, the tap ANSWERS; the pile stays
      // readable from the seat rail's Archives stat.
      box.onclick = () => { if (key === "archives" && !srvChoices.has(key)) zoomPile(corp.discard || [], `Archives (${(corp.discard || []).length})`); };
      col.appendChild(box);
      col.appendChild(rootStack(srv.content || []));
    } else {
      const content = srv.content || [];
      if (content.length === 0) {
        const box = document.createElement("div");
        box.className = "central"; box.innerHTML = `<span>empty</span>`;
        col.appendChild(box);
      }
      col.appendChild(rootStack(content));
    }

    // Compact ice: MTGA aura-stack style slivers, innermost at top.
    // "protected by ? ? Ice Wall ?" — unrezzed reads as ?, tap a sliver to
    // inspect it (rezzed shows the real card; unrezzed zooms a facedown).
    const stack = document.createElement("div");
    stack.className = "ice-stack";
    const ices = srv.ices || [];
    ices.forEach((c, i) => {
      const isCurrent = runServer === key && runPos != null && i === runPos - 1 &&
        S.run && (S.run.phase === "encounter-ice" || S.run.phase === "approach-ice");
      const sliver = document.createElement("div");
      const rezzed = !!c.rezzed && c.title;
      // THE LAW §4/§5: the stack collapses to chips for space, and a chip is
      // STILL A CARD. It therefore carries the same glow ladder every card
      // does and answers the same questions — without this an ice was the one
      // place the server could say "the board is showing it" and the board
      // would light nothing and accept nothing, leaving a choose-an-ice
      // prompt with no answer on screen anywhere.
      const glow = glowClass(c.cid);
      sliver.className = "ice-sliver" + (rezzed ? " rezzed" : "") + (isCurrent ? " current" : "") +
        (glow ? " " + glow : "");
      const subsN = (c.subroutines || []).length;
      // Counters ride the sliver whether or not the ice is rezzed: WHERE a
      // counter sits is open information (an advanced Ice Wall is advanced
      // for everyone), and the sliver is the only copy of this card drawn.
      sliver.innerHTML = `<span class="iname">${rezzed ? c.title : "?"}</span>` + sliverBadges(c) +
        (rezzed ? `<span class="imeta">${c.strength ?? ""}${subsN ? " · " + "↳".repeat(subsN) : ""}</span>` : "");
      // A tap answers where there is something to answer; otherwise it reads.
      const answerable = isSelectCandidate(c.cid) || promptChoicesFor(c.cid).length > 0;
      const press = pressToRead(sliver, 380, () => zoomCard(c));
      sliver.addEventListener("pointerup", () => {
        // A press that opened the reader is not also a tap, and a finger that
        // travelled was panning the server row — neither answers a question
        // as final as arming a target.
        if (press.fired() || press.travelled()) return;
        if (answerable) {
          // The same two taps every card takes: a sliver is still a card
          // (THE LAW §5), and the choice it answers is just as final.
          armTap(BOARD_ARM, c.cid, () => onCardTap(c, { ice: true }, sliver));
        } else zoomCard(c);
      });
      if (armed != null && c.cid === armed) sliver.classList.add("armed");
      // THE LAW §5: a chip is still a card — hover reads it on a pointer
      // device, exactly as hovering the card it stands for would.
      if (hoverCapable) {
        sliver.addEventListener("mouseenter", () => showHoverPreview(c, sliver));
        sliver.addEventListener("mouseleave", hideHoverPreview);
      }
      stack.appendChild(sliver);
      // Ice hosts cards too (a Boomerang, a trojan): they are AT this ice
      // (CR 1.13.1), and the rig no longer draws them, so the sliver has to
      // — or the card would be nowhere on the board at all. Under the chip
      // they belong to, in the same compact stack the rig uses.
      const iceKids = hostedOn(c.cid);
      if (iceKids.length) {
        const hs = el("div", "host-stack on-ice");
        iceKids.forEach((k) => hs.appendChild(cardEl(k, { side: "runner", hosted: true })));
        stack.appendChild(hs);
      }
    });
    if (ices.length) {
      const n = document.createElement("div");
      n.className = "sname";
      n.textContent = "⛨ " + ices.length;
      col.appendChild(n);
    }
    col.appendChild(stack);
    wrap.appendChild(col);
  });
  // "A new remote server" is a candidate with no column to wear the gold —
  // so it gets one: a placeholder at the row's end, exactly where the server
  // it would create will appear. Same grammar: gold, tap to arm, tap to
  // commit. Drawn only while the choice is on offer.
  if (srvChoices.has("new")) {
    const col = document.createElement("div");
    col.className = "server newremote selectable";
    if (armed === "srv:new") col.classList.add("armed-target");
    const name = document.createElement("div");
    name.className = "sname"; name.textContent = "New remote";
    col.appendChild(name);
    const box = document.createElement("div");
    box.className = "central"; box.innerHTML = `<span>＋</span>`;
    col.appendChild(box);
    wireServerTarget(col, "new", srvChoices.get("new"));
    wrap.appendChild(col);
  }
  wrap.scrollLeft = scroll; wrap.scrollTop = scrollY;
  wireServerScroll(wrap);
  updateServerChevrons(wrap);
}

/* A server column answering a question: the card grammar, on a place.
   First tap arms (white ring, Cancel appears in the rail), a tap on the
   armed column commits. One server can carry TWO choices — Asa Group's "the
   root of or protecting the same server" — and then the committing tap asks
   which, in a small sheet beside the column: that is a real fork the board
   cannot draw two ways. */
function wireServerTarget(col, key, chs) {
  col.addEventListener("pointerup", (e) => {
    // Cards and ice answer for themselves (their own arming gate).
    if (e.target.closest && e.target.closest(".card, .ice-sliver")) return;
    armTap(BOARD_ARM, "srv:" + key, () => {
      if (chs.length === 1) { act("choice", { choice: { uuid: chs[0].uuid } }); return; }
      const r = col.getBoundingClientRect();
      openSheet(chs.map((ch) => [
        abilityText(ch.value, ch.card && ch.card.title, true),
        () => act("choice", { choice: { uuid: ch.uuid } }),
      ]), Math.min(r.left, window.innerWidth - 200), Math.min(r.bottom + 6, window.innerHeight - 60 * chs.length - 20));
    });
  });
}

/* The run's version of the same two taps. The commit clears the pick BEFORE
   it acts: the run that follows repaints the board from a state in which
   nothing is being chosen any more, and a stale candidate glow on a server
   the Runner is already inside would be a lie about what is being asked. */
function wireRunTarget(col, key, go) {
  col.addEventListener("pointerup", (e) => {
    if (e.target.closest && e.target.closest(".card, .ice-sliver")) return;
    armTap(BOARD_ARM, "srv:" + key, () => {
      runPick = null;
      setArmed(null);
      go();
    });
  });
}

/* ── the server row SCROLLS, and says so ─────────────────────────────────
   More remotes than the viewport is a row wider than the screen, and
   `overflow-x: auto` alone was the whole story: touch could pan it and a
   trackpad could too, but nothing SAID so — no scrollbar until mid-scroll,
   no edge cue — and a mouse had no way in at all. So the clipped edge now
   carries a chevron (tap: one viewport-width of servers, smoothly), shown
   only while there is something past that edge, and a mouse wheel over the
   row scrolls it the only axis it has. This is the player's hand moving a
   window over the board, not the board moving (THE LAW §2): no card
   changes place, and at small server counts nothing appears at all. */
function wireServerScroll(wrap) {
  if (wrap.__scrollwired) return;
  wrap.__scrollwired = true;
  wrap.addEventListener("scroll", () => updateServerChevrons(wrap), { passive: true });
  wrap.addEventListener("wheel", (e) => {
    // A vertical wheel over a row that only scrolls sideways: give it the
    // axis. Shift+wheel and trackpads already pan natively and keep doing
    // so (their deltaX dominates, so this leaves them alone).
    //
    // Once a glacier deck stacks a column deeper than the half (see
    // `.servers { overflow-y: auto }`), the row has a vertical axis of its
    // own and the wheel belongs to THAT — the axis the pointer is over is
    // the axis the player means. Only when there is nowhere to go down does
    // the wheel get borrowed sideways.
    const canY = wrap.scrollHeight > wrap.clientHeight + 1;
    const canX = wrap.scrollWidth > wrap.clientWidth + 1;
    if (canY) return;                                  // the browser owns it
    if (!canX) return;
    if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
    wrap.scrollLeft += e.deltaY;
    e.preventDefault();
    // Belt over the scroll listener's braces: every path that MOVES the
    // row also refreshes the cue, so a platform that throttles or drops
    // scroll events (embedded views were seen doing exactly that) still
    // shows the truth after the move and the snap settle.
    setTimeout(() => updateServerChevrons(wrap), 200);
  }, { passive: false });
}
/* Both axes, and a cue on every clipped edge.

   Ice compresses to slivers first — that is right and stays — but a glacier
   deck can stack six on one server, and six legible slivers are taller than
   the Corp half however small the rest gets. The sliver has a floor (92px
   wide on a landscape phone, sized so "Tollbooth" reads as a name), and the
   answer past that floor is to PAN THE REGION, not to squeeze further: a
   window the player moves over the board, never the board rearranging
   itself (THE LAW §2). */
function updateServerChevrons(wrap) {
  const host = wrap.parentElement;
  if (!host) return;
  let L = host.querySelector(".srvchev.left");
  if (!L) {
    const mk = (cls, glyph, dx, dy) => {
      const b = el("button", "srvchev " + cls, glyph);
      b.onclick = () => {
        // "auto", not "smooth": embedded/zoomed webviews were seen dropping
        // smooth programmatic scrolls outright (the row simply did not
        // move), and an instant jump that always happens beats an animation
        // that sometimes does not. The snap rule still settles the landing.
        wrap.scrollBy({
          left: dx * Math.max(120, wrap.clientWidth - 80),
          top: dy * Math.max(80, wrap.clientHeight - 60),
          behavior: "auto",
        });
        // Refresh after the proximity snap settles — see the wheel
        // handler's note on dropped scroll events.
        setTimeout(() => updateServerChevrons(wrap), 250);
      };
      host.appendChild(b);
      return b;
    };
    L = mk("left", "‹", -1, 0);
    mk("right", "›", 1, 0);
    mk("up", "⌃", 0, -1);
    mk("down", "⌄", 0, 1);
  }
  const R = host.querySelector(".srvchev.right");
  const U = host.querySelector(".srvchev.up");
  const D = host.querySelector(".srvchev.down");
  const maxX = wrap.scrollWidth - wrap.clientWidth;
  const maxY = wrap.scrollHeight - wrap.clientHeight;
  L.style.display = wrap.scrollLeft > 4 ? "" : "none";
  if (R) R.style.display = wrap.scrollLeft < maxX - 4 ? "" : "none";
  if (U) U.style.display = wrap.scrollTop > 4 ? "" : "none";
  if (D) D.style.display = wrap.scrollTop < maxY - 4 ? "" : "none";
}

/* Focused decision panel: when a run reaches ice, show exactly what the
   decision is about — the current ice, its strength and subroutines — with
   a peek toggle back to the global board (user-directed UX). */
let focusPeek = false;
function renderFocus() {
  let panel = document.getElementById("focus-panel");
  if (!panel) {
    panel = document.createElement("div");
    panel.id = "focus-panel";
    panel.className = "focus-panel";
    document.getElementById("screen-game").appendChild(panel);
  }
  const run = S.run;
  const phase = run && String(run.phase || "").replace(":", "");
  const wantFocus = run && (phase === "encounter-ice" || phase === "approach-ice");
  if (!wantFocus || focusPeek) {
    panel.style.display = "none";
    return;
  }
  const key = String(run.server[0]).replace(":", "");
  const srv = ((S.corp || {}).servers || {})[key] || {};
  const ice = (srv.ices || [])[run.position - 1];
  if (!ice) { panel.style.display = "none"; return; }
  panel.style.display = "flex";
  const known = !!ice.title; // corp always knows its own ice
  const rezzed = !!ice.rezzed;
  const subs = (ice.subroutines || [])
    .map((s) => `<div class="fsub ${s.broken ? "fbroken" : ""}">↳ ${abilityText(s.label, ice.title, false)}</div>`)
    .join("");
  const hint = rezzed
    ? subs
    : mySide === "corp"
      ? `<div class="fsub">Your ice — decide whether to rez it.</div>`
      : `<div class="fsub">The Corp may rez it as you approach.</div>`;
  panel.innerHTML = `
    <div class="fhead">${phase === "encounter-ice" ? "ENCOUNTER" : "APPROACHING"}</div>
    <div class="fcard">
      <b>${known ? ice.title : "Unrezzed ice"}</b>
      ${known && ice.strength != null ? `<span class="fstr">STR ${ice.strength}</span>` : ""}
    </div>
    ${rezzed ? subs : hint}
    <button class="chip" id="focus-peek">Peek board</button>`;
  document.getElementById("focus-peek").onclick = () => {
    focusPeek = true;
    renderFocus();
    setTimeout(() => { focusPeek = false; renderFocus(); }, 2600);
  };
}

/* ── A HOSTED CARD IS ON ITS HOST (CR 1.13.1) ────────────────────────────
 *
 * "The hosted card is considered to be in the same location as the card
 * hosting it" — which is why the server sends every card's `host` and the
 * board must never draw one loose. It used to: the zone lists are flat, so a
 * Corp operation Cupellation had taken out of Archives was pushed into the
 * Runner's RESOURCE row (the rig's fallback bucket) and drawn full size,
 * next to the resources the Runner had actually installed. Five of them and
 * the rig was a wall of cards belonging to the other player.
 *
 * They are drawn where they are, in the idiom every card game with
 * attachments has used for thirty years: the host is the TOP CARD and what
 * it carries is tucked behind it, stepped so each shows an edge — Magic's
 * aura and equipment. Still cards (THE LAW §5): they press to read and they
 * carry their own counters. Never loose, and never mistakable for something
 * installed in its own right.
 *
 * `hostOnBoard` names the sites that ACTUALLY draw a `hostBox` — the rig,
 * a server's root, a piece of ice. It is not a fallback and not a taste
 * question: a card is skipped where it appears loose only because something
 * else is drawing it, and a card whose host is drawn nowhere would
 * otherwise leave the table altogether, which is the one outcome worse than
 * drawing it in the wrong place. */
function eachBoardList(fn) {
  const runner = S.runner || {}, corp = S.corp || {};
  const rig = runner.rig || {};
  ["program", "hardware", "resource"].forEach((k) => fn(rig[k]));
  Object.values(corp.servers || {}).forEach((s) => { fn(s && s.content); fn(s && s.ices); });
  fn(corp["play-area"]);
  fn(runner["play-area"]);
}
function hostedOn(cid) {
  if (cid == null || !S) return [];
  const out = [];
  eachBoardList((list) => (list || []).forEach((c) => { if (c && c.host === cid) out.push(c); }));
  return out;
}
/* Is this host drawn as a `hostBox` — the rig, a server's root, a piece of
   ice? Those are the three sites that put a card's carried cards on it, and
   they are exactly the sites whose loose copies may therefore be skipped.
   The identity chip and the effects rail are not among them, so a card
   hosted THERE keeps its own place rather than vanishing. */
function hostOnBoard(cid) {
  if (cid == null || !S) return false;
  const runner = S.runner || {}, corp = S.corp || {};
  const rig = runner.rig || {};
  const has = (l) => (l || []).some((c) => c && c.cid === cid);
  return ["program", "hardware", "resource"].some((k) => has(rig[k]))
    || Object.values(corp.servers || {}).some((s) => s && (has(s.content) || has(s.ices)));
}
function drawnElsewhere(c) { return !!c && c.host != null && hostOnBoard(c.host); }
/* The host and everything it carries, as ONE thing the row lays out — the
   host on top, the carried cards stepped behind it. Returns the bare card
   when it is carrying nothing, so the common case adds no box and no class
   to reason about.

   The step shrinks with the count (and so does the box's reserved peek), so
   a card holding five occupies no more of the row than a card holding one:
   the footprint is the HOST's, always, which is what makes a rig of hosts
   readable at phone width. */
const HOST_PEEK = 8;       // the least room a host reserves under itself
const HOST_CHIP = 16;      // a carried card's bar: art sliver + its own name
const HOST_H = 64;         // the host's own square (`.card` in style.css)
function hostBox(c, opts) {
  const host = cardEl(c, opts);
  const kids = hostedOn(c.cid);
  if (!kids.length) return host;
  const box = el("div", "hosting");
  const stack = el("div", "host-stack");
  kids.forEach((k) => {
    const kid = cardEl(k, { side: (opts && opts.side) || "runner", hosted: true });
    // §11 again, at the deepest truncation there is: the counters ride the
    // end of the bar and the NAME gives up exactly the room they need —
    // measured from how many there are, because a fixed reserve is either
    // too small for three kinds or wasted on the bars carrying none.
    const n = counterItems(k).length;
    if (n) kid.style.setProperty("--namepad", `${n * 12 + 4}px`);
    stack.appendChild(kid);
  });
  // Behind first, host last: the host is the top card in the DOM as well as
  // in z-index, so a tap that lands on both resolves to the host.
  box.appendChild(stack);
  box.appendChild(host);
  // The bars start a third of the way down the host and run downward. Up to
  // two fit inside the host's own square; past that the box reserves the
  // overflow so the bars never land on the neighbouring card. Legibility is
  // what decides this — every carried card says its name (THE LAW §5) — and
  // the room is reserved rather than taken, so nothing reflows mid-game.
  const inside = Math.round(HOST_H * 0.66) - 2;
  const need = kids.length * (HOST_CHIP + 1);
  box.style.setProperty("--peek", `${Math.max(HOST_PEEK, need - inside + 2)}px`);
  return box;
}

function renderRig() {
  const rigEl = $("rig");
  rigEl.innerHTML = "";
  const runner = S.runner || {};
  const rig = runner.rig || {};
  [["program", "PRG"], ["hardware", "HW"], ["resource", "RES"]].forEach(([k, label]) => {
    const row = document.createElement("div");
    row.className = "rig-row";
    row.innerHTML = `<span class="rowlabel">${label}</span>`;
    (rig[k] || [])
      .filter((c) => !drawnElsewhere(c))
      .forEach((c) => row.appendChild(hostBox(c, { side: "runner" })));
    if (k === "program" && runner.identity) {
      const idEl = cardEl(runner.identity, { side: "runner", identity: true });
      idEl.classList.add("identity-col");
      if (hasPriority("runner")) idEl.classList.add("priority");
      row.appendChild(idEl);
    }
    rigEl.appendChild(row);
  });
}

/* ══ THE FAN ═════════════════════════════════════════════════════════════
 *
 * A row of cards that USES THE ROOM IT HAS, with peeks and a rail for when it
 * runs out. ONE implementation, used by the hand and by every prompt that
 * asks you to pick one of many, because they are the same object seen twice —
 * and the prompt row was the proof: it was still a plain flex strip that
 * scrolled sideways, which is exactly the failure the hand was rebuilt to
 * fix. Rebirth (CR 1.5.4a) offers up to 22 identities; a choose-1-of-22 as a
 * horizontal scroll bar is not a choice, it is a search.
 *
 * ELASTIC, not fixed. The first version was a hard nine-card window at a 16px
 * step, sized for a hand of forty; it then drew a hand of FIVE as the same
 * 116px clump in the middle of a band five times that wide, which is what a
 * player saw and objected to. Nine-in-212px is the WORST case, not the
 * layout. So the row is laid out from the band it actually has:
 *
 *   · measure the free band — for the hand, what the bottom-corner rails and
 *     the seat rail leave between them; for a prompt row, the sheet's width;
 *   · show as many cards as the band holds at the TIGHTEST step we allow,
 *     and only then start windowing with peeks and a rail;
 *   · spread those cards as far as the band allows, up to a real GAP between
 *     them — five cards on a wide band are five cards side by side, nine on
 *     a narrow one overlap as they must.
 *
 * The step is continuous in the count, so there is no cliff between "a few"
 * and "many": one card fewer is a slightly wider step, never a relayout into
 * a different mode.
 *
 * THE COST, stated: when a hand is big enough to be compressed, an unfocused
 * card is a strip far below the 48px a tap target has to be. So tapping a
 * specific unfocused card is NOT the interaction there. Moving the focus is —
 * the rail's 44px chevrons, the peeks, hover-scroll on a pointer — and the
 * focused card, lifted and scaled clear of its neighbours, is what you tap.
 * This is Hearthstone's own big-hand behaviour and it is a deliberate
 * deviation, recorded in UX.md §8.
 *
 * WHAT A DRAG IS. Nothing, unless the zone is one you may REARRANGE. Dragging
 * used to scrub the window: the row translated under the thumb and the focus
 * ran along with it, and a list that moves while you are trying to read it is
 * a wobble, not a control (the player's word). A card list is STATIC. The one
 * place a drag means something is CR 8.3.3's arrangement, where the order IS
 * the answer and the card you are holding is the only thing that moves —
 * `makeDraggable`, and nothing else. */
const FAN_MIN_STEP = 16;   // tightest overlap we will draw
const FAN_GAP = 4;         // clear air between cards once there is room
/* How far a press may wander and still be a READ. A thumb is not a mouse: at
 * 8px a long press on a phone was being cancelled by the hand holding it. */
const FAN_SLOP = 14;

/* Per-fan state, keyed by caller ("hand", "prompt"). `repaint` is the
 * caller's own redraw: a fan move is a LOCAL change and must not push a
 * state or redraw the board (THE LAW §2). */
const fans = {};
function fanOf(key) {
  // `lit` is whether the anchor is DRAWN as focused. The anchor itself is
  // always a real index (the window has to sit somewhere); being lifted is a
  // separate question, and its answer starts as "no card is".
  if (!fans[key]) fans[key] = { focus: 0, total: 0, lit: false, repaint: null };
  return fans[key];
}
/* Move the anchor and do the housekeeping, but draw nothing. Split out
 * because a tap that arms a card has to repaint the WHOLE board — the card
 * that was armed a moment ago may be in the rig, and only its own section
 * will clear it — and letting the fan draw itself first would draw it twice. */
function fanSetFocus(key, i) {
  const f = fanOf(key);
  const next = Math.max(0, Math.min(i, Math.max(0, f.total - 1)));
  if (next === f.focus) return false;
  f.focus = next;
  fanStopHover();
  // The caller's own housekeeping: the hand drops its raised card, or the
  // focus would be dragged straight back to it by `pin` on the next draw.
  if (f.onMove) f.onMove();
  return true;
}
function fanGoto(key, i) {
  const f = fanOf(key);
  if (!fanSetFocus(key, i)) return;
  if (f.repaint) f.repaint();
}
function fanMove(key, d) { fanGoto(key, fanOf(key).focus + d); }

/* A drag must never also play a card. The only drag left is the arrangement
 * row's (CR 8.3.3), and its pointerup lands on the card it was carrying —
 * which must not be read as a tap on it. A WINDOW, not a one-shot flag: the
 * release may land on a card, on a peek, or on nothing at all, and a flag
 * consumed by the first of those would let the others through. */
/* THE PRESS IN FLIGHT, kept outside the element it started on.
 *
 * A fan redraws whenever the pointer moves onto a different card, so the node
 * a press began on is routinely gone before the finger comes up: the press
 * was recorded in that node's closure, the replacement node knows nothing
 * about it, and the release is discarded as "a pointer that arrived here
 * mid-gesture". The tap vanishes. It cost the FIRST click on any card you
 * moved the mouse to — the second worked, because by then nothing was
 * redrawing — which reads as an unreliable board rather than as a bug.
 *
 * Keyed by CARD, because that is the thing that survives a redraw. The
 * element is only where the events happened to land. */
const PRESS = { cid: null, timer: null, x: 0, y: 0, long: false };

/* ── A DRAG IS NOT A PRESS ───────────────────────────────────────────────
 *
 * Every long-press in this UI is a timer started on `pointerdown`, and a
 * timer nobody cancels fires. Scrolling a list is a pointerdown followed by
 * a lot of movement and, eventually, a release somewhere else entirely — so
 * every attempt to scroll the builder's pool, or a pile, or the deck list,
 * spent 420ms holding still-enough and then opened a full-screen reader on
 * whatever card the finger had started on. The player asked to scroll and
 * got a card in their face; on a phone that is most of the gestures they
 * make. `cardEl` already watched for this (it has the fan's tap grammar to
 * protect); the ice slivers, `attachZoom` and the builder grid did not.
 *
 * Three ways a pending press is called off, because no one of them is
 * enough on its own:
 *   1. MOVEMENT past `FAN_SLOP`, measured from where the finger landed.
 *      This is the one that always works — it needs no cooperation from the
 *      browser and it fires while the gesture is still being made.
 *   2. `pointercancel` / `touchcancel`, which is what a browser sends when
 *      it takes a pan gesture over for itself. It arrives without any
 *      `pointermove` on the element at all when the scroller is native, and
 *      it is why the stylesheet names `touch-action: pan-y` on the
 *      scrollers — an axis the browser has claimed is a gesture it will tell
 *      us it claimed.
 *   3. `scroll` on ANY ancestor, in the capture phase (scroll events do not
 *      bubble, but they do capture). Momentum scrolling, a scrollbar drag, a
 *      wheel, a keyboard PageDown: something moved under the finger, so
 *      whatever the press was aimed at is not where it was aimed any more.
 *
 * A press that has ALREADY opened its reader is untouched by all three —
 * cancelling is only ever about the timer, never about the overlay. And a
 * press cancelled here is not a tap either: the same slop test guards the
 * `pointerup` handlers, so one gesture still resolves to exactly one meaning.
 */
const PENDING_PRESSES = new Set();
function cancelPendingPresses() {
  // Copied first: a canceller removes itself from the set as it runs.
  for (const cancel of [...PENDING_PRESSES]) cancel();
  PENDING_PRESSES.clear();
}
document.addEventListener("scroll", cancelPendingPresses, true);
document.addEventListener("pointercancel", cancelPendingPresses, true);
document.addEventListener("touchcancel", cancelPendingPresses, true);

/* ── A CARD OWNS ITS OWN PRESS ───────────────────────────────────────────
 *
 * Long-press is THE read gesture (THE LAW §5), and on a desktop browser the
 * same press — and every right-click — also raises the platform's context
 * menu: "Open image in new tab" over the card the player was holding down to
 * read. On iOS the equivalent is the share/copy callout and a text selection
 * started mid-hold. Both hand the gesture to somebody else halfway through.
 *
 * So anything that STANDS FOR A CARD eats `contextmenu`. One delegated
 * listener rather than one per element: the press machines (`cardEl` and
 * `pressToRead`) each used to attach their own, which covered the board and
 * the builder's grid but not the surfaces that draw a card without a press
 * timer at all — the reader's own art (right-clicking the zoom overlay's
 * <img> was the loudest one), the play rail, the previews, the identity
 * chips. The rule belongs to the CARD, not to the timer that happens to be
 * watching it, and it is written once here.
 *
 * Scoped, never global: the deck-name field, the search box, the log and the
 * chat keep the browser's menu, because copy/paste/spellcheck on real text
 * is not ours to take. The stylesheet's half of the same rule is the `*`
 * block at the top of style.css (`-webkit-touch-callout: none`,
 * `user-select: none`) with `input, textarea` exempted the same way. */
const CARDLIKE = ".card, .ice-sliver, .idchip, .zoom-card, .hover-preview, " +
  ".fan-preview, .cardpick, .arrangepick, #play-rail, .brow, .bthumb, .zart";
const KEEPS_MENU = "input, textarea, [contenteditable], .log-drawer, .log-lines";
document.addEventListener("contextmenu", (e) => {
  const t = e.target;
  if (!t || !t.closest) return;
  if (t.closest(KEEPS_MENU)) return;
  if (t.closest(CARDLIKE)) e.preventDefault();
});

/* ONE press machine for everything that stands for a card but is not drawn by
 * `cardEl` — the ice slivers, the pile and deck-editor rows, the builder's
 * grid. They had three near-copies of this timer between them and three
 * different sets of things that cancelled it; this is the one set.
 *
 * `isConnected` is kept from every one of those copies: a press whose element
 * is replaced mid-hold (a tap on a pile row rebuilds the overlay under the
 * finger) never hears its own release, and a stranded timer opening a second
 * reader over the first is the "spawns, races itself, and pops again" bug.
 * Returns the two questions a caller's `pointerup` has to ask. */
function pressToRead(elm, ms, fire) {
  let t = null, fired = false, moved = false, sx = 0, sy = 0;
  const stop = () => {
    if (t) clearTimeout(t);
    t = null;
    PENDING_PRESSES.delete(stop);
  };
  elm.addEventListener("pointerdown", (e) => {
    stop();
    fired = false; moved = false;
    sx = e.clientX; sy = e.clientY;
    PENDING_PRESSES.add(stop);
    t = setTimeout(() => {
      t = null;
      PENDING_PRESSES.delete(stop);
      fired = true;
      if (elm.isConnected) fire();
    }, ms);
  });
  elm.addEventListener("pointermove", (e) => {
    if (moved || (!t && !fired)) return;
    if (Math.abs(e.clientX - sx) > FAN_SLOP || Math.abs(e.clientY - sy) > FAN_SLOP) {
      moved = true;
      stop();
    }
  });
  ["pointerup", "pointerleave", "pointercancel"].forEach((ev) =>
    elm.addEventListener(ev, stop));
  // The read gesture is ours, not the platform's callout — see the delegated
  // `contextmenu` rule above, which now covers every card-like surface
  // including the ones that never start a press timer.
  return {
    /** Did this press already open the reader? Then the release is not a tap. */
    fired: () => fired,
    /** Did the finger travel? Then it was a scroll, and neither is this. */
    travelled: () => moved,
  };
}

let fanTapUntil = 0;
function fanSuppressesTap() { return performance.now() < fanTapUntil; }
function fanDragHappened() { fanTapUntil = performance.now() + 350; }

/* ── the pointer: the focus follows the mouse ────────────────────────────
 * MTG Arena's hand does not wait to be told which card you mean — the card
 * under the pointer rises out of the fan and is readable where it lies. So
 * hovering a card FOCUSES it. Nothing scrolls: with the whole list in the
 * window there is nothing to scroll to, and a hand that slid sideways every
 * time the mouse crossed it would be unusable.
 *
 * Edge hover-scroll exists only for the case it was invented for — a list
 * LONGER than the window — and only in the outer band. It reads the pointer's
 * position off the HOST rather than hanging off the edge cards, because
 * moving the window destroys the element the mouse was over and a
 * `mouseleave` that never fires would strand the timer running forever
 * (the hover preview's bug, which is not worth having twice). */
let fanHover = { key: null, dir: 0, timer: null };
function fanStopHover() {
  if (fanHover.timer) clearInterval(fanHover.timer);
  fanHover = { key: null, dir: 0, timer: null };
}
function fanHoverEdge(host, key, e) {
  if (!hoverCapable) return;
  const f = fanOf(key);
  // Nothing off-window means nothing to scroll to.
  if (f.total <= f.size) { fanStopHover(); return; }
  const r = host.getBoundingClientRect();
  const edge = Math.max(16, r.width * 0.14);
  const dir = e.clientX < r.left + edge ? -1 : e.clientX > r.right - edge ? 1 : 0;
  if (dir === fanHover.dir && key === fanHover.key) return;
  fanStopHover();
  if (!dir) return;
  fanHover = { key, dir, timer: null };
  fanHover.timer = setInterval(() => {
    if (!host.isConnected) { fanStopHover(); return; }
    const g = fanOf(key);
    if ((dir < 0 && g.focus <= 0) || (dir > 0 && g.focus >= g.total - 1)) { fanStopHover(); return; }
    fanGoto(key, g.focus + dir);
  }, 340);
}

/* ── the drag that isn't ─────────────────────────────────────────────────
 * A fan used to be draggable: the thumb scrubbed the window, the row
 * translated under it by up to half a step, and the focus ran along with the
 * travel. It read as a wobble — a list that shifts while you are trying to
 * read it — and it has been DELETED rather than tuned, because two ways to
 * move one row is how a wobble gets in. THE LIST IS STATIC. Nothing in a fan
 * moves under a pointer, ever.
 *
 * A drag means something in exactly one place: a zone you may REARRANGE (CR
 * 8.3.3), where the order is the answer and the only thing that moves is the
 * card you are carrying. That is `makeDraggable`, on the arrangement row, and
 * it is not this. Everywhere else a drag is simply not a gesture: it does not
 * scroll, does not move the focus, does not translate anything.
 *
 * What is left here is the pointer's own convenience — hovering the outer
 * band of a fan LONGER than its window walks the focus that way, which no
 * touch device ever sees. */
function fanGestures(host, key) {
  // Attach ONCE per host: `renderFan` runs on every repaint and re-wiring
  // here would stack a listener per repaint.
  if (host.__fanwired) return;
  host.__fanwired = true;
  host.addEventListener("pointermove", (e) => fanHoverEdge(host, key, e));
  host.addEventListener("pointerleave", fanStopHover);
}

/* ── the band, and the layout that fills it ──────────────────────────────
 * The free width this fan may lay cards into. For a prompt row it is the
 * sheet's own width, declared in CSS. For the HAND it is what the fixed
 * chrome sharing the bottom of the screen leaves between it: the action bar,
 * the run controls, the seat rail, the turn dial, the right-hand preview.
 * Those are MEASURED rather than assumed, because their budgets are ceilings
 * — the action bar may use up to `50vw - 122px` and almost never does, and
 * assuming the ceiling is what left a hand of five as a clump in the middle
 * of an empty band.
 *
 * Only chrome that shares the fan's own rows counts: the seat rail sits above
 * the hand on a tall screen and beside it on a short one, and which of those
 * is true is a question for the layout, not for a constant. */
const FAN_OBSTACLES = ["action-bar", "run-controls", "my-bar", "turn-btn-wrap", "fan-preview", "play-rail"];
function fanBand(host, key) {
  const cap = parseFloat(getComputedStyle(host).maxWidth);
  if (key !== "hand") return isFinite(cap) ? cap : (host.clientWidth || window.innerWidth);
  const r = host.getBoundingClientRect();
  // The fan's own rows, with room for the focused card's lift above them.
  const top = r.top - 24, bottom = r.bottom;
  const mid = window.innerWidth / 2;
  const pad = 12;
  let half = mid - pad;
  for (const id of FAN_OBSTACLES) {
    const e = document.getElementById(id);
    if (!e) continue;
    const b = e.getBoundingClientRect();
    if (b.width < 1 || b.height < 1) continue;      // hidden or empty
    if (b.bottom <= top || b.top >= bottom) continue; // not on the fan's rows
    half = Math.min(half, b.left >= mid ? b.left - pad - mid : mid - (b.right + pad));
  }
  if (isFinite(cap)) half = Math.min(half, cap / 2);
  return Math.max(FAN_MIN_STEP * 2, 2 * half);
}

/* How wide the row's ink is, derived from what the stylesheet actually lays
   out so the two cannot disagree about whether something fits:
     · each slot's margin box is exactly `step` wide (`.fanrow > *`), with the
       card's own `fw` of ink centred on it — so the END cards overhang by
       (fw-step)/2 each, which is `over` in total;
     · a peek is `fpeek` wide and stands off by exactly that same overhang
       (`.fanpeek`'s margin), so the overhang lands IN the stand-off: the two
       are the same pixels and are counted once.
   Peeks are assumed on BOTH sides whenever the window is short of the list,
   even though the focus may currently be at one end and show only one. A row
   whose width depended on where the focus sat would change size as the focus
   moved, which is the wobble this fan does not have. */
function fanInk(size, step, peeks, fw, peek) {
  return size * step + Math.max(0, fw - step) + (peeks ? 2 * peek : 0);
}
/* As many cards as the band holds, spread as far as it allows. Continuous in
   the count: one card fewer is a slightly wider step, never a new mode. */
function fanFit(total, band, fw, peek) {
  let size = Math.max(1, total);
  while (size > 1 && fanInk(size, FAN_MIN_STEP, size < total, fw, peek) > band) size--;
  const peeks = size < total;
  const wide = fw + FAN_GAP;
  let step = wide;
  if (size > 1 && fanInk(size, wide, peeks, fw, peek) > band) {
    // Monotone in `step`, so bisect: no algebra to get subtly wrong, and it
    // stays right if the stylesheet's stand-off ever changes shape.
    let lo = FAN_MIN_STEP, hi = wide;
    for (let i = 0; i < 24; i++) {
      const m = (lo + hi) / 2;
      if (fanInk(size, m, peeks, fw, peek) <= band) lo = m; else hi = m;
    }
    step = lo;
  }
  return { size, step: Math.round(step * 100) / 100, peeks };
}

/* ── ONE fan, and only one ───────────────────────────────────────────────
 * The hand and every "choose one of these" prompt are the SAME widget. Not
 * two that look alike: one function draws both, and a caller cannot answer a
 * single question about how the fan behaves. It says WHAT is in the fan —
 * which cards, what each one is captioned, which colour it wears — and the
 * fan says everything else: the window, the peeks, the tilt, the lift, which
 * slot is focused, and the two-tap that focuses and then commits.
 *
 * It was not always so, and the drift is exactly what a player felt: the
 * prompt's cards were built by the prompt, which never passed them the fan's
 * index, so `cardEl` could not tell a focused card from a resting one and a
 * SINGLE tap on a 16px strip answered the question. The hand asked twice for
 * the same act. Two code paths, two behaviours, one of them a misplay
 * waiting to happen — so now there is one path and the parameters below are
 * the only things that may differ.
 *
 *   key       — which fan's focus this is
 *   cardOf    — item -> the card to draw (default: the item IS the card)
 *   slotOpts  — (item, idx) -> { label, extra, glow }: decoration, no behaviour
 *   cardOpts  — base opts for `cardEl`; the fan adds `fanKey`/`fanIndex` itself
 *   rail      — where the rail goes (an element), or null for none
 *   pin       — a cid the focus must sit on (a raised hand card)
 * The geometry comes from CSS custom properties, so it lives in one place. */
function renderFan(host, items, opts) {
  const key = opts.key;
  const f = fanOf(key);
  f.total = items.length;
  f.repaint = opts.repaint || null;
  f.onMove = opts.onMove || null;
  host.classList.add("fan");
  host.innerHTML = "";
  const css = getComputedStyle(host);
  const cardOf = opts.cardOf || ((it) => it);
  if (!items.length) {
    f.size = 0;
    renderFanRail(opts.rail, key, 0, 0);
    fanPreviewSet(key, null);
    paintFanPreview();
    return;
  }
  // The layout is computed from the room this fan actually has, every time it
  // is drawn: a card played is a card's worth of extra room for the rest, and
  // a rotation is a different band. The STEP is written back to the stylesheet
  // as `--fstep`, which lays out the slot margins and the peeks' stand-off —
  // so the arithmetic that decided it fits and the rules that place it are
  // reading the same number.
  const fw = parseFloat(css.getPropertyValue("--fw")) || 64;
  const peek = parseFloat(css.getPropertyValue("--fpeek")) || 10;
  // The layout arithmetic is about boxes; what is PAINTED is a little wider,
  // because the focused card is scaled 1.22 and the outer cards are rotated.
  // Both are bounded — half the focus scale is 0.11*fw, and the tilt of an end
  // card adds fh*sin(4°) — so the band gives that back before it is divided
  // up, rather than the row quietly growing past what was measured for it.
  const bleed = Math.min(16, Math.round(fw * 0.13));
  const fit = fanFit(items.length, fanBand(host, key) - 2 * bleed, fw, peek);
  f.size = fit.size;
  f.step = fit.step;
  host.style.setProperty("--fstep", fit.step + "px");

  f.focus = Math.max(0, Math.min(f.focus, items.length - 1));
  if (opts.pin != null) {
    const pi = items.findIndex((it) => cardOf(it).cid === opts.pin);
    if (pi >= 0) f.focus = pi;
  }
  const size = f.size;
  const half = Math.floor(size / 2);
  const start = Math.max(0, Math.min(f.focus - half, items.length - size));
  const end = Math.min(items.length, start + size);
  const shown = items.slice(start, end);
  const mid = (shown.length - 1) / 2;
  // A SHALLOW arc, MTGA's rather than a card-table fan: at nine cards a steep
  // one dips the outer cards straight off the bottom of the screen, and every
  // degree of tilt also widens the row's ink against a band that has none to
  // spare. The focused card is lifted AND scaled clear of its neighbours: at
  // a 16px step the resting cards are strips, so the focused one is the card
  // you are actually reading.
  // …and it flattens out as the row spreads: an arc is what a fan of
  // overlapping cards looks like, and a row of cards standing clear of each
  // other on a table is straight. `spread` is 0 when they overlap fully and 1
  // when they do not touch, so the tilt goes with the crowding.
  const spread = Math.max(0, Math.min(1, (f.step - FAN_MIN_STEP) / Math.max(1, fw - FAN_MIN_STEP)));
  const tilt = (parseFloat(css.getPropertyValue("--ftilt")) || 1.2) * (1 - spread);
  const arc = (parseFloat(css.getPropertyValue("--farc")) || 1) * (1 - spread);
  // The focus scale is the stylesheet's, because the stylesheet also has to
  // reserve the headroom the scale grows into: one number, read twice.
  const focusScale = parseFloat(css.getPropertyValue("--ffocus")) || 1.22;

  // Nothing here ever moves under a pointer: the row is laid out once and
  // stays exactly where it is (THE LAW §2, and the player's "no wobble").
  // `.overlapped` is the stylesheet's cue that a resting card is a LEFT
  // strip: it comes from the same step arithmetic that laid the row out, so
  // the badges' re-anchoring (see `.badges` in the stylesheet) can never
  // disagree with whether the cards actually overlap.
  const row = el("div", "fanrow" + (fit.step < fw ? " overlapped" : ""));
  // Cleared first, and re-set below only if a card is actually lit. Without
  // this the panel keeps reading whatever was focused last, so "nothing is
  // selected" would still have a card on display insisting otherwise.
  fanPreviewSet(key, null);
  shown.forEach((it, i) => {
    const idx = start + i;
    const c = cardOf(it);
    // The anchor and the FOCUS are two different things now. `f.focus` is
    // where the window sits — it always points somewhere, or the fan would
    // not know which cards to show. Being focused is a card having been
    // singled out by the player, and that is `armed`, which can be nothing at
    // all. Before this they were one variable, so a fan always had a card
    // lifted whether or not anyone had asked for one, and there was no state
    // for "I have changed my mind" to return to.
    const focused = idx === f.focus && f.lit;
    const so = (opts.slotOpts ? opts.slotOpts(it, idx) : null) || {};
    const slot = el("div", "cardpick" + (so.extra ? " " + so.extra : ""));
    // `fanKey`/`fanIndex` are the fan's to give, never the caller's to forget:
    // they are what makes the first tap focus and the second act.
    const node = cardEl(c, Object.assign({}, opts.cardOpts, { fanKey: key, fanIndex: idx }));
    if (so.glow) node.classList.add(so.glow);
    slot.appendChild(node);
    if (so.label) slot.appendChild(el("div", "cardpick-label", so.label));
    if (opts.pin != null && opts.pin === c.cid) {
      slot.classList.add("raised");
    } else {
      const off = i - mid;
      const lift = Math.abs(off) * arc + (focused ? -8 : 0);
      slot.style.transform = `rotate(${(off * tilt).toFixed(2)}deg) translateY(${lift}px)` +
        (focused ? ` scale(${focusScale})` : "");
      if (focused) slot.classList.add("focused");
    }
    // MTGA: the card under the pointer IS the card you mean, and it rises out
    // of the fan to be read where it lies. Suspended while a card is raised —
    // the raise owns the focus until it is put down. It rides the CARD, which
    // is the element that opts back into pointer events inside a sheet that
    // has none.
    // Hover LIGHTS a card; only a click ARMS one. They used to be the same
    // thing, which quietly made every play on a mouse a single click: the
    // pointer arrived, the card became the focus, and the click that followed
    // went straight past the two-tap gate into the action. Reading with the
    // pointer must never be able to commit anything (§5, and the long-press
    // has the same rule).
    if (hoverCapable && opts.pin == null) {
      node.addEventListener("mouseenter", () => {
        if (f.lit && f.focus === idx) return;   // already reading this one
        f.lit = true;
        fanSetFocus(key, idx);
        if (f.repaint) f.repaint();             // `lit` alone is a redraw too
      });
    }
    // Whatever is focused is what the right-hand preview is reading (§4).
    if (focused) fanPreviewSet(key, c, so.label || null);
    row.appendChild(slot);
  });
  if (start > 0) row.prepend(fanPeek(cardOf(items[start - 1]), "left", opts, key));
  if (end < items.length) row.append(fanPeek(cardOf(items[end]), "right", opts, key));
  host.appendChild(row);

  renderFanRail(opts.rail, key, items.length, size);
  fanGestures(host, key);
  paintFanPreview();
}

function fanPeek(c, side, opts, key) {
  const w = el("div", "fanpeek " + side);
  w.appendChild(cardEl(c, opts.cardOpts || {}));
  w.onclick = (e) => {
    e.stopPropagation();
    if (fanSuppressesTap()) return;
    fanMove(key, side === "left" ? -1 : 1);
  };
  return w;
}

/* The rail: two chevrons and the plain truth about where you are.
 *
 * It used to carry a pip strip you could drag — a green slider, which is what
 * a player called it when he asked for it to be taken away. It read as a
 * control, so it was tried; it moved a window that four other gestures move
 * better, so it did nothing he wanted. An affordance that looks like a
 * control and is not worth using is worse than no affordance: it spends a
 * player's attention and gives nothing back. What is left is a BUTTON at
 * each end and a `n/total` label, which is information and does not pretend
 * to be anything else. The list is still reached four other ways: the
 * chevrons, the peeks either side, and hover-scroll on a pointer — and,
 * where the band can hold the whole list, no journey to make at all. */
function renderFanRail(rail, key, total, size) {
  if (!rail) return;
  const f = fanOf(key);
  // The rail exists for the list the band could not hold whole. When the
  // layout got everything on screen there is nothing to navigate to, and a
  // control for a journey of zero cards is the same lie as the pip strip was.
  if (total <= size) { rail.style.display = "none"; return; }
  rail.style.display = "flex";
  rail.innerHTML = "";

  const left = el("button", "railbtn", "‹");
  left.disabled = f.focus <= 0;
  left.onclick = () => fanMove(key, -1);

  const right = el("button", "railbtn", "›");
  right.disabled = f.focus >= total - 1;
  right.onclick = () => fanMove(key, 1);

  const count = el("div", "railcount", `${f.focus + 1}/${total}`);
  rail.append(left, count, right);
}

/* ── the preview: the focused card, at reading size, where there is room ──
 * A nine-card fan shows a 16px strip of each card and 78px of the focused
 * one: a picture, not a page. A pointer device already answers this — the
 * card under the mouse is previewed at the top right — but that preview is
 * switched off on a short screen, which is precisely the phone held
 * landscape where the fan is smallest. So in landscape the FOCUSED card is
 * drawn large on the right, and it follows the focus wherever the focus goes:
 * a tap, a chevron, a peek, a hover.
 *
 * When the focused option is an ABILITY rather than a card to be chosen, the
 * card shown is the one the ability LIVES ON — the server sends it with the
 * choice — and the option's own words are printed under it. "Use the second
 * ability" names nothing on its own; the card it belongs to names everything.
 *
 * It is `pointer-events: none`, so it can never eat a tap meant for the board
 * (THE LAW §3b), and it is bounded to the strip left of the turn dial and
 * clear of the play rail's column: three fixed things on one edge have to be
 * told about each other or the shortest viewport loses one of them. */
const fanPreviewOf = { hand: null, prompt: null };
function fanPreviewSet(key, card, label) {
  fanPreviewOf[key] = card ? { card, label: label || null } : null;
}
/* Landscape, and wide enough that a 150px panel is not half the board. The
   media query is the authority; this is the same question asked in JS. */
function fanPreviewFits() {
  return window.matchMedia("(orientation: landscape) and (min-width: 640px)").matches;
}
function paintFanPreview() {
  let host = document.getElementById("fan-preview");
  // The live decision owns the preview: a prompt fan is the question being
  // asked, and the hand is only what you are holding while it is asked.
  const it = fanPreviewOf.prompt || fanPreviewOf.hand;
  if (!it || !fanPreviewFits() || !S || S.winner) {
    // A CLASS, never an inline display: the stylesheet's media query is the
    // authority on whether the panel fits at all, and an inline style would
    // outrank it and strand the panel on screen through a rotation.
    if (host) host.classList.remove("on");
    return;
  }
  if (!host) {
    host = document.createElement("div");
    host.id = "fan-preview";
    host.className = "fan-preview";
    document.getElementById("screen-game").appendChild(host);
  }
  // §12.6: card text is the card layer's, never a user string — and the
  // option's own label is appended as TEXT, never as markup.
  host.innerHTML = `<div class="zoom-card small">${cardInfoHtml(it.card)}</div>`;
  if (it.label) host.querySelector(".zoom-card").appendChild(el("div", "fanpre-eff", it.label));
  host.classList.add("on");
}

function handRailEl() {
  let rail = document.getElementById("hand-rail");
  if (!rail) {
    rail = el("div", "fanrail");
    rail.id = "hand-rail";
    document.getElementById("screen-game").appendChild(rail);
  }
  return rail;
}

/* The hand IS the fan, with nothing of its own but where its rail hangs and
   what a tap on a focused card means. Every other question — geometry, focus,
   gestures, the two taps — is `renderFan`'s, and a prompt asking you to pick
   one of many gets exactly the same answers. */
function renderHand() {
  renderFan($("hand"), me().hand || [], {
    key: "hand",
    rail: handRailEl(),
    pin: raised,
    repaint: renderHand,
    // Moving the window puts a different card under your thumb, so the one
    // you had raised is no longer the one you are looking at.
    onMove: () => { raised = null; closeSheet(); },
    cardOpts: { side: mySide, hand: true },
  });
}

function cardEl(c, opts) {
  opts = opts || {};
  const el = document.createElement("div");
  // A FACEDOWN CARD IS A CARD BACK FOR EVERYONE — its owner included.
  // CR 1.21.1: a facedown card is "oriented so that the face containing the
  // card's information is not visible"; CR 4.6.6f even forbids the root of a
  // remote giving away what type of card sits in it. The owner IS entitled
  // to the face — CR 1.21.2a, "a player may look at facedown cards they
  // control at any time" (both players; the rule is symmetric) — but that
  // look is the reader (hover / long-press), never the board. `c.facedown`
  // is the table orientation from the server (it now travels with the face
  // for the controller); `rezzed === false` is the same fact as the bridge
  // (jnet) shape spells it for installed Corp cards, so the back survives a
  // server that only sent the older shape. `opts.reveal` is the reader-like
  // surfaces (prompt fans, the play rail) where a look is the point — and it
  // can never over-reveal, because a face the viewer is not entitled to
  // never arrives (CR 4.6.3: facedown cards are secret information).
  const facedown =
    (!!c.facedown ||
      (c.rezzed === false && opts.side === "corp" && !opts.hand && !opts.identity)) &&
    !(opts.reveal && c.title);
  const isNew = !seenCids.has(c.cid);
  if (isNew) seenCids.add(c.cid);
  const showCost = opts.hand && c.cost != null;
  const showStr = c.strength != null && !facedown;
  el.className = "card" + (isNew ? " deal" : "") + (opts.ice ? " ice" : "") + (facedown ? " facedown" : "") +
    (opts.side === "corp" ? " corp-card" : " runner-card") +
    (opts.identity ? " identity" : "") +
    (opts.current ? " current-ice" : "") +
    // The corner discs live INSIDE the box now, so the text has to make room
    // for the ones that are actually there — and only for those.
    (showCost ? " hascost" : "") + (showStr ? " hasstr" : "");
  el.dataset.cid = c.cid;
  // ARMED: the one card a second tap acts on. Distinct from the fan's lift,
  // which only says "this is the one you are reading" — the ring says "and
  // the next tap on it commits".
  if (armed != null && c.cid === armed) el.classList.add("armed");

  el.innerHTML = `
    ${showCost ? `<div class="cost">${c.cost}</div>` : ""}
    <div class="cname" title="${esc(c.title || "")}">${facedown ? "" : tileName(c.title, opts.hosted)}</div>
    ${opts.ice && c.subroutines ? `<div class="subs">${c.subroutines.map((s) => `<span class="${s.broken ? "broken" : ""}">↳</span>`).join("")}</div>` : ""}
    <div class="ctype">${facedown ? "" : (c.type || "")}</div>
    ${showStr ? `<div class="cstr">${c.strength}</div>` : ""}
    ${counterBadges(c)}`;
  // Real card art from the NetrunnerDB CDN (code travels with every card);
  // text scaffold stays as fallback when the image can't load.
  const code = c.code || (c.images && c.images.en && ""); // jnet also carries :code
  if (!facedown && code) {
    const img = new Image();
    img.onload = () => {
      el.style.backgroundImage = `url(${cardImgUrl(code)})`;
      el.classList.add("art");
    };
    // The first request for a card our cache has not filled yet goes out to
    // upstream behind the route, so it can be slow or lose once; a miss used
    // to leave the text scaffold up until the next state push happened to
    // redraw this card, which read as "pictures not picturing". One quiet
    // retry covers the transient; a second miss keeps the scaffold, which
    // still says everything the card says (UX.md deviations).
    img.onerror = () => {
      if (img.__retried) return;
      img.__retried = true;
      setTimeout(() => { if (el.isConnected) img.src = cardImgUrl(code); }, 800);
    };
    img.src = cardImgUrl(code);
  }

  // legality glow (local mode) / select hint
  const glow = glowClass(c.cid);
  if (glow) el.classList.add(...glow.split(" "));
  else if (mode === "bridge" && !facedown && opts.hand) el.classList.add("legal");

  // tap + long-press (mobile read gesture) + hover preview (desktop)
  //
  // The press is tracked in MODULE state keyed by the card, not in this
  // closure keyed by the element — see `PRESS` — because the element does not
  // survive long enough to be a reliable place to keep it.
  el.__cancelPress = () => {
    PENDING_PRESSES.delete(el.__cancelPress);
    if (PRESS.cid !== c.cid) return;
    clearTimeout(PRESS.timer); PRESS.timer = null; PRESS.cid = null;
  };
  el.addEventListener("pointerdown", (e) => {
    clearTimeout(PRESS.timer);
    PRESS.cid = c.cid; PRESS.long = false;
    PRESS.x = e.clientX; PRESS.y = e.clientY;
    // The card's own `pointermove` catches a finger that wanders; this catches
    // the two cases where no move is ever delivered here — a native scroller
    // taking the pan (`pointercancel`) and an ancestor scrolling under the
    // card (`scroll`, captured at the document). A card in the builder's grid
    // or a pile reader sits in exactly such a scroller.
    PENDING_PRESSES.add(el.__cancelPress);
    // `isConnected`: a re-render that replaces this card mid-press strands
    // the timer (the replacement never hears the pointerup), and a stranded
    // timer opening a reader nobody asked for is the double-spawn race.
    PRESS.timer = setTimeout(() => {
      PRESS.timer = null;
      PENDING_PRESSES.delete(el.__cancelPress);
      PRESS.long = true;
      // A press that has already opened its reader is DONE, not pending:
      // nothing below may reach back and close what the player asked for.
      if (el.isConnected) zoomCard(c);
    }, 420);
  });
  // A press that WANDERS is still a press — a thumb is not a mouse, and at
  // 8px the read gesture was being cancelled by the hand holding the phone.
  // Past `FAN_SLOP` it is a page pan (or, in an arrangement row, a drag), and
  // those are not reads.
  el.addEventListener("pointermove", (e) => {
    if (!PRESS.timer || PRESS.cid !== c.cid) return;
    if (Math.abs(e.clientX - PRESS.x) > FAN_SLOP || Math.abs(e.clientY - PRESS.y) > FAN_SLOP) {
      clearTimeout(PRESS.timer); PRESS.timer = null;
    }
  });
  el.addEventListener("pointerup", (e) => {
    clearTimeout(PRESS.timer); PRESS.timer = null;
    PENDING_PRESSES.delete(el.__cancelPress);
    const wasPressed = PRESS.cid === c.cid; PRESS.cid = null;
    // A TAP is a press and a release in about the same place, on the same
    // card. Anything else is a drag or a page pan — and in a fan a drag does
    // NOTHING (§6), which includes not quietly moving the focus to whichever
    // card the finger happened to be over when it lifted. A release with no
    // press of its own (the pointer arrived here mid-gesture) is not a tap
    // either. Same hazard as the long-press that used to commit a choice: a
    // gesture must resolve to exactly one meaning.
    if (!wasPressed) return;
    if (Math.abs(e.clientX - PRESS.x) > FAN_SLOP || Math.abs(e.clientY - PRESS.y) > FAN_SLOP) return;
    if (fanSuppressesTap()) return;
    if (PRESS.long) return;
    // TWO TAPS, MTGA's rule: the first brings the card to focus, the second
    // acts on it. At a 16px step the resting cards in a fan are strips, and a
    // strip is far below the 48px a tap target has to be — so a single tap
    // there would be a misplay waiting to happen, on a decision (play a card,
    // discard to hand size) that cannot be taken back. Once it is focused it
    // is 78px wide and lifted clear, and THAT is the thing you tap.
    //
    // It applies EVERYWHERE, not only in fans. A card on the board is big
    // enough to hit, but size was never the whole reason: 9.2.7f makes a
    // chosen option resolve to the end, so the tap that chooses it is the
    // last moment anything can be called off. An installed card offering one
    // ability used to spend that moment on the way down — one tap, cost paid,
    // ability resolving, nothing asked. Now the board arms like the hand
    // does, and the gap between the two taps is where you get to change your
    // mind.
    if (opts.fanKey != null) fanOf(opts.fanKey).lit = true;
    if (opts.fanKey != null && fanOf(opts.fanKey).focus !== opts.fanIndex) {
      armed = c.cid;                     // set before the draw, not after
      fanSetFocus(opts.fanKey, opts.fanIndex);
      repaintArmed();                    // one draw, for the whole board
      return;
    }
    // SEVERAL named options on this card: the SHEET is the gate and the ring
    // would only be a third tap, because one ring cannot name two acts. One
    // option keeps the ring — `armedName` names it there, so the ring says
    // both "the next tap commits" and what it commits to, which is the whole
    // job the sheet was doing. (Cards with no options keep the ring too:
    // there is nothing to name, and 9.2.7f still makes the choice final.)
    if (!opts.hand && promptChoicesFor(c.cid).length > 1) { onCardTap(c, opts, el); return; }
    armTap(BOARD_ARM, c.cid, () => onCardTap(c, opts, el));
  });
  el.addEventListener("pointerleave", el.__cancelPress);
  el.addEventListener("pointercancel", el.__cancelPress);
  // The iOS callout, the desktop context menu and the text selection are all
  // suppressed by the one delegated `contextmenu` rule (`CARDLIKE`) — a card
  // owns its press wherever it is drawn, not only where a press machine is
  // watching it.
  if (hoverCapable) {
    el.addEventListener("mouseenter", () => {
      // Inside a fan, hovering a card focuses it and the right-hand panel is
      // already reading it: two previews of one card, in two places, is one
      // preview too many.
      if (opts.fanKey != null && fanPreviewFits()) return;
      showHoverPreview(c, el);
    });
    el.addEventListener("mouseleave", hideHoverPreview);
  }
  return el;
}

/* Card art comes from OUR server, never from somebody else's CDN.
   `card-images.netrunnerdb.com` 403s a client whose User-Agent it dislikes
   and drops requests when a builder grid asks it for two hundred images at
   once — and every request that loses lands on the player as a BLANK card,
   which is precisely what THE LAW §1 forbids. `/img/card/<id>.jpg` serves
   from a local cache the server pre-warms with the whole catalog; the id may
   be a printing code (the game state's) or a catalog NSG id (the builder's),
   because the route resolves both. The text scaffold stays as the fallback
   for a card with genuinely no art, and is now the exception, not the rule. */
function cardImgUrl(code) {
  return `/img/card/${encodeURIComponent(code)}.jpg`;
}

/* ── THE NAME ON A TILE IS A FIXED LENGTH ────────────────────────────────
 *
 * Card names are not a bounded set: "Nebula Talent Management: Making Stars"
 * and "Sure Gamble" are the same field. Letting the tile wrap to whatever
 * arrives makes the name band a different height on every card — the board
 * stops being a grid of equal tiles and starts being a ragged one, and the
 * long names still do not fit.
 *
 * So the tile cuts at a fixed number of LETTERS, not at whatever the box
 * happens to hold: the band is the same height on every card, always, and
 * the cut lands in the same place for the same name every time it is drawn.
 * The full name is one press away (and rides the element's `title` for a
 * mouse) — the tile's job is to be recognised, not to be complete.
 *
 * The subtitle after a colon goes first: "Nebula Talent Management" is what
 * a player calls that card, and ": Making Stars" is what they never say.
 */
const TILE_NAME_MAX = 18;      // a card tile
const BAR_NAME_MAX = 13;       // a carried card's bar, which is one line
function tileName(title, hosted) {
  if (!title) return "";
  const max = hosted ? BAR_NAME_MAX : TILE_NAME_MAX;
  let s = String(title);
  if (s.length > max) {
    const colon = s.indexOf(":");
    if (colon > 3 && colon <= max) s = s.slice(0, colon);
  }
  return esc(s.length > max ? s.slice(0, max - 1).trimEnd() + "…" : s);
}

/* Every counter a card is carrying, on the card. The server already sends
   all of them (CR 1.9.5's kinds); the client used to draw only credits and
   advancement, so a loaded Earthrise Hotel, a Datasucker with virus counters
   and a scored AstroScript all looked bare, and there was no way to tell
   which card held what. Advancement keeps its own badge because 1.18 makes
   it the one counter the Corp acts on directly. */
const COUNTER_BADGES = [
  ["credit", "cred", "⬡", "credits hosted"],
  ["power", "pow", "◈", "power counters"],
  ["virus", "vir", "☣", "virus counters"],
  ["agenda", "agn", "★", "agenda counters"],
  ["bad-publicity", "badpub", "✖", "bad publicity"],
];
/* A card is a BOX, and everything the card is carrying is drawn inside it.
   The badges used to hang off the corner on negative offsets and stack in a
   single column — so on the 64x89 card the nine-card fan draws (52x72 below
   640px of height) a card with three counters ran its counters off the
   bottom edge, and every badge overlapped the card next to it in the fan.
   They now sit inside the top-right corner, fill right-to-left and wrap
   downward, and above two they shrink rather than spread: whatever a card is
   carrying, it fits, at every size this UI draws a card at.

   One list of what a card is carrying, for every size it is drawn at: the
   card's own badges, the ice sliver's inline discs, and the reader's text
   lines all read this, so no size can forget a kind the others show. */
function counterItems(c) {
  const out = [];
  if (c["advance-counter"]) {
    out.push(["adv", "advancement counters", String(+c["advance-counter"])]);
  }
  const k = c.counter || {};
  for (const [key, cls, glyph, hint] of COUNTER_BADGES) {
    if (k[key]) out.push([cls, hint, glyph + (+k[key])]);
  }
  return out;
}
function counterBadges(c) {
  const items = counterItems(c);
  if (!items.length) return "";
  return `<div class="badges${items.length > 2 ? " tight" : ""}">${items
    .map(([cls, hint, text]) => `<div class="badge ${cls}" title="${hint}">${text}</div>`)
    .join("")}</div>`;
}
/* The ice sliver is the deepest truncation this UI draws a card down to
   (THE LAW §4), and it used to be the one size that dropped the counters
   outright: an Ice Wall three advancements tall read exactly like a bare
   one, and a counter the player cannot see is a counter they will misplay
   (§11). A sliver is still a card, so it carries the same discs, inline. */
function sliverBadges(c) {
  const items = counterItems(c);
  if (!items.length) return "";
  return `<span class="sbadges">${items
    .map(([cls, hint, text]) => `<span class="badge ${cls}" title="${hint}">${text}</span>`)
    .join("")}</span>`;
}

/* ── card info (shared by hover preview and long-press zoom) ───────────
   FACES (CR 1.4): a double-sided card's data carries `faces:[{title,text}]`
   (back faces, in face order) and — on a board card — `flipped`, the index
   of the back that is UP. `face` here is a READER index: 0 = front,
   k = faces[k-1]. Every renderer defaults to the face the table is showing,
   so a flipped identity previews and reads as what it now is. */
function cardFaces(c) { return Array.isArray(c.faces) ? c.faces : []; }
/* The face currently showing, as a reader index. Absent/invalid `flipped`
   means the front — a server that does not send flip state gets the front
   by construction. */
function showingFace(c) {
  const n = cardFaces(c).length;
  if (!n || typeof c.flipped !== "number") return 0;
  return c.flipped >= 0 && c.flipped < n ? c.flipped + 1 : 0;
}
/* A back face has no printed title of its own on some cards (SYNC): it is
   still a face, and it still needs a name to be picked by. */
function faceTitle(c, face) {
  if (face === 0) return c.title || "Facedown card";
  const f = cardFaces(c)[face - 1] || {};
  return f.title || "Other face";
}
function cardInfoHtml(c, face) {
  if (face === undefined) face = showingFace(c);
  const back = face > 0 ? cardFaces(c)[face - 1] || {} : null;
  const lines = [];
  // The type is the card's on every face; the subtypes are printed on the
  // front and stay with it.
  if (c.type) lines.push(c.type + (!back && c.subtypes && c.subtypes.length ? " — " + c.subtypes.join(" · ") : ""));
  if (c.cost != null) lines.push("Cost " + c.cost + (c.strength != null ? " · Strength " + c.strength : ""));
  else if (c.strength != null) lines.push("Strength " + c.strength);
  if (c.advancementcost != null) lines.push(`Adv req ${c.advancementcost} · ${c.agendapoints} pts`);
  if (c["trash-cost"] != null) lines.push("Trash cost " + c["trash-cost"]);
  for (const [, hint, text] of counterItems(c)) lines.push(`${hint}: ${text}`);
  // The reader is where the owner's CR 1.21.2a look happens: the board draws
  // this card as a back for everyone, so when a face arrives with the
  // facedown flag, say who is seeing it — the opponent sees only the back.
  if (c.facedown && c.title) lines.push("Facedown — only you can see this face");
  if (c.implementation) lines.push("⚠ " + c.implementation);
  // The art is the front face's; a back face is text-rendered, like the
  // card pool itself (UX.md: no art is a rendering, not a gap).
  const art = c.code && !back
    ? `<img class="zart" src="${cardImgUrl(c.code)}" alt="" onerror="this.remove()">`
    : "";
  // THE CARD PRINTS ITS SUBROUTINES ONCE. The text box already carries every
  // "[subroutine] …" line, and the live list adds exactly one thing the text
  // cannot: whether that subroutine has been broken. Rendering both put IP
  // Block's two subroutines on screen four times — twice as their printed
  // text, then twice more as the builder's auto-labels ("Subroutine",
  // "Subroutine 2"), which name nothing a player could act on. So: lift the
  // printed lines out of the body, and render ONE row per subroutine, in
  // printed order, carrying its own text and its broken state.
  const rawText = (back ? back.text : c.text) || "";
  const subLines = [];
  const bodyLines = [];
  rawText.split("\n").forEach((ln) => {
    if (/^\s*\[subroutine\]/i.test(ln)) subLines.push(ln.replace(/^\s*\[subroutine\]\s*/i, ""));
    else bodyLines.push(ln);
  });
  const subs = back ? [] : (c.subroutines || []);
  const subRow = (text, broken) =>
    `<div class="ztext ${broken ? "zline" : ""}">↳ ${text}${broken ? " (broken)" : ""}</div>`;
  const subHtml = subs.length
    // The i-th live subroutine is the i-th printed one (9.8: they resolve in
    // printed order). Fall back to the label only if a card somehow has more
    // live subroutines than printed lines — gained subroutines, one day.
    ? subs.map((s, i) => subRow(sym(subLines[i] ?? "") || abilityText(s.label, c.title, false), s.broken)).join("")
    : subLines.map((t) => subRow(sym(t), false)).join("");
  return `${art}<h3>${faceTitle(c, face)}</h3>
    <div class="zline">${lines.join("<br>")}</div>
    <div class="ztext">${sym(bodyLines.join("\n"))}</div>
    ${subHtml}`;
}

/* The hover preview is the one thing on screen a click CANNOT close: it is
   `pointer-events: none` by design, so the pointer reads the board through
   it. That is fine while `mouseleave` is guaranteed to fire — and it is not.
   Acting on a card pushes a new state, `renderServers`/`renderRig` rebuild
   their subtrees, and the element the mouse was over is destroyed without
   ever leaving: no `mouseleave`, no hide, and a preview pinned to the top
   right of the board that no amount of clicking will shift. So the preview
   remembers the element it belongs to and goes when that element does. */
let hoverOwner = null;
function showHoverPreview(c, owner) {
  let hp = document.getElementById("hover-preview");
  if (!hp) {
    hp = document.createElement("div");
    hp.id = "hover-preview";
    hp.className = "hover-preview";
    document.body.appendChild(hp);
  }
  hoverOwner = owner || null;
  hp.innerHTML = `<div class="zoom-card small">${cardInfoHtml(c)}</div>`;
  hp.style.display = "block";
}
function hideHoverPreview() {
  hoverOwner = null;
  const hp = document.getElementById("hover-preview");
  if (hp) hp.style.display = "none";
}
/* Its owner left the DOM (or the board redrew under it): the preview goes
   with it. Called once per frame — cheap, and it cannot get stuck. */
function reapHoverPreview() {
  if (hoverOwner && !hoverOwner.isConnected) hideHoverPreview();
}
// A press anywhere is an ACT, not a read: the preview is not what you are
// looking at any more. Capture phase, so nothing can swallow it first.
document.addEventListener("pointerdown", hideHoverPreview, true);
window.addEventListener("blur", hideHoverPreview);

/* ── interactions ────────────────────────────────────────────────────── */
/* The options on offer that live on this card (server sends `cid` on each
   choice that has one). This is what makes the green outline mean something
   you can act on rather than decoration. */
function promptChoicesFor(cid) {
  const p = myPrompt();
  if (!p || cid == null || p["prompt-type"] === "waiting") return [];
  return (p.choices || []).filter((ch) => ch.cid === cid);
}

function onCardTap(c, opts, el) {
  closeSheet();
  if (S.winner) return;

  // A target announcement is answered by tapping the card, wherever that card
  // is drawn — on the board or in the prompt row. One path, so the pick
  // accumulates the same way from either, and tapping again un-picks.
  if (isSelectCandidate(c.cid)) {
    if (native()) act("select", { card: { cid: c.cid } });
    else act("select", { card: c });
    return;
  }

  // A card the current window offers something on: the tap NAMES the option
  // before taking it, one or many.
  //
  // One option used to resolve on the spot, and that was a trap on exactly
  // the cards it should have been most careful with. Jackson Howard is green
  // in a paid window because its only option there is "Remove Jackson Howard
  // from the game: shuffle Archives into R&D" — a player reaching for its
  // "[click]: Draw 2 cards" taps the green card and the permanent is gone,
  // having read nothing. The arming ring (§7) gave them a second tap but
  // never said what the second tap would DO, and a gate that does not name
  // the act is not a gate. Now the sheet always names it, and the tap on the
  // NAME is the commit — still two taps, and both of them informed.
  const offered = promptChoicesFor(c.cid);
  // ONE option needs no sheet: the arming ring already named it in the action
  // rail ("Score AstroScript Pilot Program — tap again to confirm", see
  // `armedName`), and this IS the second tap. Naming the act is what the
  // gate is for; a popup that names one thing is ceremony over the board.
  if (offered.length === 1) {
    act("choice", { choice: { uuid: offered[0].uuid } });
    return;
  }
  if (offered.length) {
    const r = el && el.getBoundingClientRect ? el.getBoundingClientRect() : { left: 40, bottom: 120 };
    openSheet(offered.map((ch) => [
      // The sheet is anchored on the card it was opened from, so the card
      // names itself and the option is just what it does.
      abilityText(ch.value, c.title, false),
      () => act("choice", { choice: { uuid: ch.uuid } }),
    ]), r.left, r.bottom + 6);
    return;
  }

  // While a target is being asked for, a card that is NOT a legal target has
  // no move to make: tapping it reads it rather than offering an action the
  // engine would only refuse.
  if (isSelectMode()) { zoomCard(c); return; }

  if (opts.hand) {
    if (raised === c.cid) { raised = null; renderHand(); return; }
    // "Pressing the focused card plays it." Where the card offers exactly ONE
    // thing — which is nearly every Runner card, and every Corp card with one
    // destination — the tap on the focused card IS the play. A sheet naming a
    // single option is a second tap that asks nothing: the question it would
    // ask has one answer, and the two-tap gate has already been passed. Where
    // there are genuinely several (install into which server, play or
    // install), the sheet still names them, because that IS a question.
    const items = handActions(c);
    if (!items.length) { toast("No legal action for this card"); raised = null; renderHand(); return; }
    if (items.length === 1) { raised = null; items[0][1](); return; }
    raised = c.cid;
    renderHand();
    openSheet(items, window.innerWidth / 2 - 90, window.innerHeight - 330);
    return;
  }
  openBoardSheet(c, el);
}

function handActions(c) {
  const items = [];
  if (native()) {
    actionsFor(c.cid).forEach((a) => {
      // The server may label an affordance itself (the CR engine does: an
      // install declares its destination inside the procedure, 8.5.16b, so
      // the affordance cannot name a server).
      if (a.command === "play") items.push([abilityText(a.label || "Play", c.title, false), () => act("play", { card: { cid: c.cid } })]);
      if (a.command === "runner-install") items.push([abilityText(a.label || "Install", c.title, false), () => act("runner-install", { card: { cid: c.cid } })]);
      if (a.command === "corp-install") items.push([
        a.label ? abilityText(a.label, c.title, false)
          : a.server === "New remote" ? "Install → new remote" : `Install → ${SERVER_NAME(a.server)}`,
        () => act("corp-install", { card: { cid: c.cid }, server: a.server }),
      ]);
    });
  } else {
    // Bridge: jnet's "play" command handles both playing and installing.
    items.push(["Play / Install", () => act("play", { card: c })]);
  }
  return items;
}

function openBoardSheet(c, el) {
  const items = [];
  if (native()) {
    actionsFor(c.cid).forEach((a) => {
      // The sheet opened on this card and sits against it, so the ability
      // says what it does and the card is right there saying whose it is.
      const label = a.label ? abilityText(a.label, c.title, false) :
        a.command === "advance" ? "Advance (● + 1⬡)" :
        a.command === "score" ? "Score" :
        a.command === "rez" ? `Rez (${c.cost ?? "?"}⬡)` :
        a.command === "ability" ? abilityLabel(c, a.ability) :
        a.command;
      items.push([label, () => act(a.command, { card: { cid: c.cid }, ability: a.ability })]);
    });
  } else {
    if (mySide === "corp") {
      if (c.rezzed === false) items.push(["Rez", () => act("rez", { card: c })]);
      items.push(["Advance", () => act("advance", { card: c })]);
      if (c.advancementcost != null) items.push(["Score", () => act("score", { card: c })]);
    }
    (c.abilities || []).forEach((ab, i) => {
      items.push([abilityText(ab.label || `Ability ${i}`, c.title, false), () => act("ability", { card: c, ability: i })]);
    });
    (c["runner-abilities"] || []).forEach((ab, i) => {
      items.push([abilityText(ab.label || `Ability ${i}`, c.title, false), () => act("runner-ability", { card: c, ability: i })]);
    });
  }
  if (!items.length) { zoomCard(c); return; }
  const r = el.getBoundingClientRect();
  openSheet(items, Math.min(r.left, window.innerWidth - 200), Math.min(r.bottom + 6, window.innerHeight - 60 * items.length - 20));
}

function abilityLabel(c, idx) {
  const ab = (c.abilities || [])[idx];
  return ab ? abilityText(ab.label, c.title, false) : `Ability ${idx}`;
}

function openSheet(items, x, y) {
  const sheet = $("action-sheet");
  sheet.innerHTML = "";
  items.forEach(([label, fn]) => {
    const b = document.createElement("button");
    b.className = "chip"; b.textContent = label;
    b.onclick = () => { closeSheet(); raised = null; fn(); };
    sheet.appendChild(b);
  });
  sheet.style.left = Math.max(8, x) + "px";
  sheet.style.top = Math.max(8, y) + "px";
  sheet.style.display = "flex";
  // A SHEET CANNOT HANG OFF THE SCREEN. Its callers place it relative to
  // whatever was tapped and guess at its height; a guess that is wrong by
  // one item puts the last choice below the fold of a phone, where the
  // stylesheet's `max-height` lets it scroll but nothing tells the player
  // there is more. So it is pulled back inside the viewport once it has a
  // real size — measured, not predicted.
  const r = sheet.getBoundingClientRect();
  const maxTop = window.innerHeight - r.height - 8;
  if (r.top > maxTop) sheet.style.top = Math.max(8, maxTop) + "px";
  const maxLeft = window.innerWidth - r.width - 8;
  if (r.left > maxLeft) sheet.style.left = Math.max(8, maxLeft) + "px";
}
function closeSheet() { $("action-sheet").style.display = "none"; }
/* Tapping the table means "none of these".
 *
 * A focused card is an unsent intent, and the way you throw an intent away is
 * to stop pointing at it. Without this the only way out of a focus was to
 * focus something ELSE, so there was no way to end up with nothing selected
 * and the board always claimed you meant some card — usually whichever one
 * the window happened to be sitting on.
 *
 * Everything a player could deliberately hit is exempt. A card handles its
 * own tap (that is the two-tap gate); a button is a thing being pressed, not
 * a place being tapped away to; the sheets, the rails and the readers are all
 * chrome that belongs to the very question the focus is part of. What is left
 * is bare board, and bare board is the answer "none". */
const HOLDS_FOCUS = ".card, .action-sheet, .prompt-sheet, .fanrail, .railbtn, " +
  "button, .fan-preview, .hover-preview, #zoom-overlay, #access-overlay, #reveal-overlay, " +
  // A candidate server column and the identity chip take the same two taps a
  // card does; disarming on the press would eat the second tap's confirm.
  ".server.selectable, .ice-sliver, .idchip";
/* The deck editor's version of the same list, and it is SHORTER on purpose.
 * The board's chrome — the rails, the sheets, the buttons — belongs to the
 * question the focus is part of, so pressing it must not throw the focus
 * away. The editor's neighbours are not that: the filters, the search box,
 * the deck rows and their explicit +/− are all other work, and a player who
 * has reached for one has stopped pointing at a card. Only a card (which
 * answers for itself) and the readers (which are the focused card being
 * looked at, not left) hold it. */
const BUILDER_HOLDS_FOCUS = ".card, #zoom-overlay, #access-overlay, #reveal-overlay, .hover-preview";
document.addEventListener("pointerdown", (e) => {
  const t = e.target;
  if (!(t.closest && t.closest(BUILDER_HOLDS_FOCUS))) clearBuilderFocus();
  if (t.closest && t.closest(".card")) return;   // its own handler decides
  if (!(t.closest && t.closest(".action-sheet"))) closeSheet();
  if (t.closest && t.closest(HOLDS_FOCUS)) return;
  disarm();
  // Bare board is "none of these" for the run being chosen too — the same
  // answer, and the same gesture that gives it to every other question.
  cancelRunPick();
});

/* ── prompts ─────────────────────────────────────────────────────────── */
let promptFanKey = null;
function renderPrompt() {
  const sheet = $("prompt-sheet");
  const p = myPrompt();
  // The preview belongs to whatever is being asked RIGHT NOW: a decision that
  // has gone takes its card off the right-hand panel with it, and the hand's
  // own focus takes the panel back. Every path below either sets this again
  // (a fan, or an option pointed at) or leaves it clear.
  fanPreviewSet("prompt", null);
  if (!p) { sheet.style.display = "none"; hideAccessReader(); return; }
  // Beats in breach order (CR 7.5): every snapshot in `state.accessed` was
  // taken BEFORE the machine stopped on this decision — the stop is what
  // pushed the state — so a reveal still owed to the player is always the
  // EARLIER beat. It presents first, one card at a time, and the decision is
  // drawn when the last of them is acknowledged. Nothing is lost by the
  // wait: the decision is the reason the machine is stopped, and it is still
  // the live question when the reveals are done. Without this, "access A,
  // then a steal prompt on B" showed B's question before A's card — two
  // beats out of order, and no way to tell which card was which.
  if (p["prompt-type"] !== "waiting" && pendingReveals().length) {
    sheet.style.display = "none";
    hideAccessReader();
    return;
  }
  // A decision ABOUT a card puts the card itself in front of you.
  if (p.card && p.card.title) { sheet.style.display = "none"; renderAccessReader(p); return; }
  hideAccessReader();
  // A BOARD QUESTION raises no sheet at all. The candidates are already on
  // the table wearing gold; tapping one arms it WHITE, tapping it again
  // commits; the sentence is a log line and the non-board options are chips
  // in the action rail (`renderChips`). A popup here would cover the very
  // cards it is asking about. EMPTIED, not just hidden — a display:none
  // sheet still holds the previous question's buttons, and a button that
  // exists but cannot be seen is exactly the thing a stray programmatic
  // click (or a screen reader) finds.
  if (boardQuestion(p)) { sheet.style.display = "none"; sheet.innerHTML = ""; return; }
  // Waiting is not a question either: the seat rail already pulses
  // "thinking…", the log says what they are deciding, and a sheet saying
  // "Waiting for the Corp" only stands in front of the board. The one
  // waiting state worth a sheet is a HELD game — a person gone, nothing
  // pulsing, the game stopped for a reason worth stating.
  if (p["prompt-type"] === "waiting" && !/disconnected|held/i.test(p.msg || "")) {
    sheet.style.display = "none";
    sheet.innerHTML = "";
    return;
  }
  sheet.style.display = "flex";
  sheet.classList.toggle("waiting", p["prompt-type"] === "waiting");
  const choices = p.choices || [];
  // UX.md THE LAW §1: a decision about cards renders as CARDS. Two copies of
  // Bloo Moose are two buttons reading "Bloo Moose" and no way to tell them
  // apart; as cards they are simply two cards. This outranks the picker: a
  // list of REAL cards is never a search box, however long it is.
  // A new decision is a new list: the fan starts at its first card rather
  // than wherever the last question left the window. (The uuids carry the
  // per-decision stamp the server puts on them, so this cannot false-negative
  // on two consecutive prompts that happen to look alike.)
  const pkey = `${p.msg}|${choices.map((ch) => ch.uuid).join(",")}`;
  if (promptFanKey !== pkey) { promptFanKey = pkey; fanOf("prompt").focus = 0; }
  promptBody(sheet, p, choices);
  // THE LAW §6, checked rather than trusted: every prompt carries at least
  // one thing the player can do. See `ensureAnswerable`.
  ensureAnswerable(sheet, p);
}

/* Which shape of sheet this decision gets. Every branch ends with the sheet
   drawn; `renderPrompt` is the one that then checks it can be answered. */
function promptBody(sheet, p, choices) {
  if (p.arrange && p.arrange.length > 1) { renderArrangePrompt(sheet, p); return; }
  if (p["select-cards"]) { renderSelectPrompt(sheet, p, choices); return; }
  if (choices.some((ch) => ch.card)) { renderCardPrompt(sheet, p, choices); return; }
  // A long list of CARD NAMES is a SEARCH, not a row of buttons: naming a
  // card (CR 1.15.1b) offers every card the layer knows. A long list of
  // anything else is not — a trace with fifteen credits to spend is sixteen
  // numbers, and a search box that looks each one up in the card database
  // would be a worse prompt than the buttons. The server says which it is;
  // an older server that says nothing keeps the length-only rule.
  const searchable = p.picker !== false;
  if (searchable && choices.length > PICKER_THRESHOLD) {
    renderPickerPrompt(sheet, p, choices);
    return;
  }
  const selectHint = isSelectMode() ? `<div class="pmsg" style="color:var(--gold)">Tap a highlighted card</div>` : "";
  sheet.innerHTML = `<div class="pmsg">${sym(p.msg || "")}</div>${selectHint}<div class="pbtns"></div>`;
  const btns = sheet.querySelector(".pbtns");
  choices.forEach((ch) => {
    const b = document.createElement("button");
    b.className = "chip";
    const v = ch.value;
    // No card is drawn beside these, so an ability names its own card.
    b.textContent = typeof v === "object" && v
      ? sym(v.title || "card")
      : abilityText(v, ch.card && ch.card.title, true);
    b.onclick = () => act("choice", { choice: { uuid: ch.uuid } });
    btns.appendChild(b);
  });
}

/* THE LAW §6: an empty answer is stated, never implied — AND IT IS ALWAYS
   GIVABLE. A sheet that ended up with no card to tap and no button to press
   is not a bad prompt, it is a hung game: it happened, from a live game, when
   CR 1.15.2b clamped an announcement to the zero targets that existed and the
   button that ends a selection was offered only for an explicit "up to".

   The kernel now answers a decision with one legal answer itself
   (`Vm::forced_answer`) and the server's "you may stop here" and its
   willingness to honour it read one predicate — but this check exists
   BECAUSE those are the layers that were wrong. The client must not depend
   on the server never making a mistake, and "the sheet has something in it"
   is a question the client can always ask of itself.

   It is not an exception to §8's two taps: those are for choosing FROM a
   pool, and this is the case where the pool is empty, which has no first tap
   to make. */
function ensureAnswerable(sheet, p) {
  if (p["prompt-type"] === "waiting") return;   // it is not your move: nothing to do IS the answer
  if (sheet.querySelector(".pbtns .chip:not(:disabled)")) return;   // a button you can press
  if (sheet.querySelector(".fanrow .cardpick .card")) return;       // a card you can tap
  // The board is answering it: the cards are lit behind this sheet (§3).
  if (p["choices-onboard"] || p["select-onboard"]) return;
  let btns = sheet.querySelector(".pbtns");
  if (!btns) { btns = el("div", "pbtns"); sheet.appendChild(btns); }
  // Prefer an answer the decision itself named; otherwise the only thing left
  // to say about a choice with nothing in it is that you have seen it.
  const ch = (p.choices || [])[0];
  const b = el("button", "chip go", ch ? abilityText(ch.value, ch.card && ch.card.title, true) : "OK");
  b.onclick = ch ? () => act("choice", { choice: { uuid: ch.uuid } })
                 : () => act("select-done", {});
  btns.appendChild(b);
}

/* CR 4.6.7 and §9.2: the EFFECTS STACK — everything the board cannot show.

   A card BEING PLAYED sits in the play area while it resolves (8.6.7g), and
   an active current stays there until another current replaces it (3.7.1b).
   Both are active and both are open information — and neither was drawn
   anywhere, so a run event mid-resolution had no card on screen to outline,
   and a current did its work invisibly.

   The rail then took a second job (Petty Cash, CR 9.3.3c: "[click]: Play this
   operation from Archives") and now a third, which is the point of the whole
   thing: A CARD THE BOARD DRAWS AS A CARD BACK IS NOT SHOWN. The Corp's own
   installed agenda is facedown until it scores, so a paid window offering
   "Score AstroScript Pilot Program" was asking about a blank rectangle. The
   sheet used to solve that by covering the table with a modal; the rail
   solves it by being the one place the card's face can honestly appear
   (THE LAW §1 without breaking §2 — the rail is pinned and never reflows).

   Three kinds land here, each under a VERB the player can act on:
     • an action the engine offers on a card drawn nowhere ("Play", from a
       pile), tagged with the zone it acts from;
     • a §9.2 window's offers ("Score", "Rez", "Use", "Trash");
     • the play area itself ("Resolving").
   A card the board is already showing FACE UP is not copied here — the board
   answers where the board can (THE LAW §3), and two copies of one card is
   the defect this rail exists to avoid, not to commit. */
function renderPlayRail() {
  const rail = $("play-rail");
  if (!rail) return;
  const p = myPrompt();
  const items = [];   // {side, card, section, tag, choice}
  const taken = new Set();
  const add = (side, c, section, tag, choice) => {
    if (!c || c.cid == null || taken.has(c.cid)) return;
    taken.add(c.cid);
    items.push({ side, card: c, section, tag, choice });
  };

  // A §9.2 window's own offers. `window-cards` is the server saying "every
  // offer here is a verb and a card" — a target announcement over three
  // cards in the stack also carries cards, and that one keeps its panel,
  // because there the question is WHICH and not what to do with it.
  if (p && p["window-cards"]) {
    (p.choices || []).forEach((ch) => {
      if (!ch.card || ch.cid == null) return;
      // Already legible on the board? Then the board answers it, glowing,
      // and a rail copy would be the same card twice.
      if (legibleOnBoard(ch.cid)) return;
      add(myFace(), ch.card, ch.section || "Use", null, ch);
    });
  }

  ((S.corp || {})["play-area"] || []).forEach((c) => add("corp", c, railCurrent(c), null, null));
  ((S.runner || {})["play-area"] || []).forEach((c) => add("runner", c, railCurrent(c), null, null));

  // THE LAW §3 again: an ability can act from a zone the board draws only as
  // a count, so the affordance has nowhere to land. Petty Cash's play out of
  // Archives existed only for a player who thought to open the pile reader.
  const drawn = drawnCids();
  ACTIONS.forEach((a) => {
    if (a.cid == null || drawn.has(a.cid)) return;
    const found = findUndrawnCard(a.cid);
    if (found) add(found[0], found[1], actionSection(a), found[2], null);
  });

  if (!items.length) { rail.style.display = "none"; rail.innerHTML = ""; return; }
  rail.style.display = "flex";
  rail.innerHTML = "";
  // Grouped, in a fixed order, so a player learns where "Score" appears and
  // stops reading the rail. Unknown verbs keep their first-seen order after
  // the known ones rather than being dropped.
  const ORDER = ["Score", "Rez", "Play", "Install", "Use", "Trash", "Resolving"];
  const rank = (s) => { const i = ORDER.indexOf(s); return i < 0 ? ORDER.length : i; };
  const groups = new Map();
  items.forEach((it) => {
    const k = it.section || "Use";
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k).push(it);
  });
  [...groups.keys()].sort((a, b) => rank(a) - rank(b) || 0).forEach((section) => {
    rail.appendChild(el("div", "railsection", section));
    groups.get(section).forEach((it) => {
      const wrap = el("div", "playslot");
      // The rail exists to show a card an action lives on — a read surface
      // for a card the viewer is entitled to: `findUndrawnCard` and the
      // prompt's own `card` only carry faces the state carries, and the
      // state only carries faces §10.2 lets this viewer see.
      // No special tap wiring: `glowClass` already paints a card the prompt
      // offers something on, and `promptChoicesFor` finds the option by cid
      // from wherever the card is drawn. One path, board and rail alike.
      wrap.appendChild(cardEl(it.card, { side: it.side, reveal: true }));
      if (it.tag) wrap.appendChild(el("div", "playtag", it.tag));
      rail.appendChild(wrap);
    });
  });
}

/* 3.7.1b: a current is its own thing and says so; everything else in the
   play area is mid-resolution (8.6.7g). */
function railCurrent(c) {
  const sub = (c.subtypes || []).map((x) => String(x).toLowerCase());
  return sub.includes("current") ? "Current" : "Resolving";
}

/* The verb an offered ACTION reads as. The engine's command is the honest
   source: "play" is a play, an install is an install, everything else is a
   card's own text going off. */
function actionSection(a) {
  switch (a.command) {
    case "play": return "Play";
    case "runner-install": case "corp-install": return "Install";
    case "score": return "Score";
    case "rez": return "Rez";
    default: return "Use";
  }
}

/* Is the board showing this card's FACE — not merely a rectangle where it
   sits? The facedown law draws an unrezzed Corp card as a card back for
   everyone, its owner included, so "the board is drawing it" and "the player
   can read it" stopped being the same question. The rail answers the second
   one; `drawnCids`/`on_screen` answer the first. */
function legibleOnBoard(cid) {
  if (!drawnCids().has(cid)) return false;
  const seek = (list) => (list || []).find((c) => c && c.cid === cid);
  const corp = S.corp || {}, runner = S.runner || {};
  let c = null;
  Object.values(corp.servers || {}).forEach((srv) => {
    c = c || seek(srv.content) || seek(srv.ices);
  });
  const rig = runner.rig || {};
  ["program", "hardware", "resource"].forEach((k) => { c = c || seek(rig[k]); });
  c = c || seek(corp["play-area"]) || seek(runner["play-area"]) || seek(me().hand);
  if (!c) return true;                       // an identity: always legible
  return !(c.facedown || c.rezzed === false);
}

/* Whose side of the table the viewer sits on, for a rail card the prompt
   handed us without one. */
function myFace() { return mySide === "runner" ? "runner" : "corp"; }

/* Every cid the board is drawing AS A CARD somewhere a glow could land:
   server contents and ice, the rig, both play areas, the viewer's own hand,
   both identities. The same zones `on_screen` counts on the server — a
   discard pile, a deck and a score area are counts you tap to open, so a
   card in one of those is nowhere an outline could land. */
function drawnCids() {
  const out = new Set();
  const add = (c) => { if (c && c.cid != null) out.add(c.cid); };
  const corp = S.corp || {}, runner = S.runner || {};
  Object.values(corp.servers || {}).forEach((srv) => {
    (srv.content || []).forEach(add);
    (srv.ices || []).forEach(add);
  });
  const rig = runner.rig || {};
  ["program", "hardware", "resource"].forEach((k) => (rig[k] || []).forEach(add));
  (corp["play-area"] || []).forEach(add);
  (runner["play-area"] || []).forEach(add);
  (me().hand || []).forEach(add);
  add(corp.identity);
  add(runner.identity);
  return out;
}

/* The undrawn zones a viewer can still see into: both discard piles (public,
   CR 4.4.2) and both score areas. Returns [side, card, zone-tag] for the
   rail, or null for a cid the state has no face for. */
function findUndrawnCard(cid) {
  const corp = S.corp || {}, runner = S.runner || {};
  const hit = (list, side, tag) => {
    const c = (list || []).find((x) => x && x.cid === cid && x.title);
    return c ? [side, c, tag] : null;
  };
  return hit(corp.discard, "corp", "archives")
    || hit(runner.discard, "runner", "heap")
    || hit(corp.scored, "corp", "scored")
    || hit(runner.scored, "runner", "scored");
}

/* CR 8.3.3: put these cards in an order. The order IS the answer, so the
   cards are draggable and the answer is sent when the player is done.

   Pointer events, so one implementation serves mouse and touch alike. Drag
   reorders by midpoint: whichever gap the pointer is nearest is where the
   held card goes, which is the behaviour every card game has trained players
   to expect. Long-press preview still works — the drag only begins once the
   pointer has actually moved, so a press that stays put is still a read. */
let arrangeOrder = null;   // cids, topmost first
let arrangeKey = null;

function renderArrangePrompt(sheet, p) {
  const cards = p.arrange || [];
  const key = cards.map((c) => c.cid).join(",");
  if (arrangeKey !== key) { arrangeKey = key; arrangeOrder = cards.map((c) => c.cid); }
  const byCid = new Map(cards.map((c) => [c.cid, c]));

  sheet.innerHTML = `<div class="pmsg">${sym(p.msg || "")}</div>
    <div class="arrangerow"></div>
    <div class="picker-hint">Leftmost is the top of the deck.</div>
    <div class="pbtns"></div>`;
  const row = sheet.querySelector(".arrangerow");
  const btns = sheet.querySelector(".pbtns");

  const paint = () => {
    row.innerHTML = "";
    arrangeOrder.forEach((cid, i) => {
      const c = byCid.get(cid);
      if (!c) return;
      const wrap = el("div", "arrangepick");
      wrap.dataset.cid = String(cid);
      wrap.appendChild(el("div", "arrangeidx", String(i + 1)));
      // Arranging is a look the resolving ability granted (CR 8.3.3): the
      // order IS the answer, so the faces show — `reveal`, as in the prompts.
      wrap.appendChild(cardEl(c, { side: mySide, reveal: true }));
      makeDraggable(wrap, cid, paint);
      row.appendChild(wrap);
    });
  };
  paint();

  const done = el("button", "chip go", "Put them back");
  done.onclick = () => {
    const order = arrangeOrder.slice();
    arrangeKey = null; arrangeOrder = null;
    act("arrange", { order });
  };
  btns.appendChild(done);
}

/* Reorder by dragging: the ONE place in this UI where a drag moves anything.
   The card follows the pointer; the slot it would take is decided by which
   card centre the pointer has passed. Everything else in the row stands
   still — a rearrangement is the card you are carrying, not a list in
   motion. */
function makeDraggable(wrap, cid, repaint) {
  let dragging = false, startX = 0;
  const card = () => wrap.querySelector(".card");
  wrap.addEventListener("pointerdown", (e) => {
    startX = e.clientX; dragging = false;
    wrap.setPointerCapture(e.pointerId);
  });
  wrap.addEventListener("pointermove", (e) => {
    if (!wrap.hasPointerCapture || !wrap.hasPointerCapture(e.pointerId)) return;
    // A press that has not travelled is a READ, not a drag — the same slop the
    // read gesture allows itself, so exactly one of the two can ever win.
    if (!dragging && Math.abs(e.clientX - startX) < FAN_SLOP) return;
    if (!dragging) {
      // The pointer is captured HERE, so the card beneath will never hear
      // another move or the release: tell it to call its read off, or it
      // opens a full-screen reader in the middle of the drag.
      const c = card(); if (c && c.__cancelPress) c.__cancelPress();
    }
    dragging = true;
    wrap.classList.add("dragging");
    const row = wrap.parentElement;
    if (!row) return;
    const sibs = [...row.children].filter((n) => n !== wrap);
    const from = arrangeOrder.indexOf(cid);
    let to = from;
    sibs.forEach((n) => {
      const r = n.getBoundingClientRect();
      const mid = r.left + r.width / 2;
      const j = arrangeOrder.indexOf(+n.dataset.cid);
      if (e.clientX > mid && j > from) to = Math.max(to, j);
      if (e.clientX < mid && j < from) to = Math.min(to, j);
    });
    if (to !== from) {
      arrangeOrder.splice(to, 0, arrangeOrder.splice(from, 1)[0]);
      repaint();
    }
  });
  ["pointerup", "pointercancel"].forEach((ev) =>
    wrap.addEventListener(ev, () => {
      // The release is the drag's, not a tap's — and the card under it never
      // saw the press end either, so its read is called off here too.
      const c = card(); if (c && c.__cancelPress) c.__cancelPress();
      if (dragging) fanDragHappened();
      dragging = false; wrap.classList.remove("dragging");
    }));
}

/* Choosing a card out of a pool is ONE act with ONE sentence, wherever the
   pool is drawn: the hand, a target announcement, a discard, a choose-one.
   Two taps, and the same words for them every time (§7). */
const FAN_PICK_HINT = "Tap a card to focus it, tap it again to choose it.";

/* A prompt whose choices ARE cards, drawn as cards (UX.md THE LAW).
   The hand's fan is the reference, flattened: a fan reads as "yours, held",
   which these are not — they are a set being offered — so they lie flat with
   only a whisper of rotation. Every card is a real `cardEl`, so long-press
   preview on touch and hover preview on a pointer come for free, and a card
   with no art still renders its text scaffold.

   Choices the server could not attach a card to (the viewer is not entitled
   to see it, or the option names no card at all — "No action", "Done") stay
   as chips underneath, so nothing is ever lost by rendering cards. */
function renderCardPrompt(sheet, p, choices) {
  // Where the board is ALREADY drawing every one of these cards, the whole
  // sheet stays down (`boardQuestion` in `renderPrompt` — the board answers,
  // the rail carries the labels), so by the time this renderer runs the
  // cards are somewhere the board cannot show and the fan is their only
  // appearance.
  const withCards = choices.filter((ch) => ch.card);
  const { row, hint, btns } = promptSheetFrame(sheet, p);
  renderFan(row, withCards, {
    key: "prompt",
    rail: sheet.querySelector(".fanrail"),
    repaint: () => renderCardPrompt(sheet, p, choices),
    // A prompt is a decision about cards and renders them as cards (THE LAW
    // §1) — a READ surface, so the viewer's own facedown cards show the face
    // the CR entitles them to look at (1.21.2a). A face they may not see
    // never arrived (the slot is captioned "blind" below), so `reveal`
    // cannot leak anything.
    cardOpts: { side: mySide, reveal: true },
    cardOf: (ch) => ch.card,
    // Decoration only. The tap is the CARD's own (`cardEl` wires it, with the
    // fan's index): one handler, so a long-press to read can never also commit
    // the choice, the first tap focuses, the second answers, and the board and
    // the prompt answer the same way — exactly as the hand does.
    slotOpts: (ch) => ({
      // THE LAW §3: green = an ability you can use now, gold = a legal target.
      // A select prompt is asking for TARGETS, so its cards are gold, and they
      // are the same gold the board paints on the same cards.
      glow: isSelectMode() ? "selectable" : "usable",
      // The slot IS the card, so the caption is only what the option does.
      label: abilityText(ch.value, ch.card && ch.card.title, false),
      // A card the viewer is not entitled to see has nothing on its face, so
      // its caption is the only thing telling two of them apart — it stays.
      extra: ch.card.title ? "" : "blind",
    }),
  });
  // AFTER the fan, not before it: `renderFan` empties its host, so a hint
  // written first was deleted before it was ever seen.
  if (withCards.length) {
    // The same sentence a select prompt gets, because it is the same act:
    // one model for choosing a card out of a pool, everywhere (§7).
    hint.appendChild(el("div", "picker-hint", FAN_PICK_HINT));
  }
  // Everything that did not become a card: the options naming no card at all
  // ("Pass", "No action").
  choices.filter((ch) => !ch.card).forEach((ch) => {
    const b = document.createElement("button");
    b.className = "chip";
    b.textContent = abilityText(ch.value, null, true);
    b.onclick = () => act("choice", { choice: { uuid: ch.uuid } });
    btns.appendChild(b);
  });
}

/* The sheet's own skeleton: sentence, the fan's host, the fan's rail, the
   chips. One frame for both card-shaped prompts, so the rail can never end
   up in one of them and not the other. */
function promptSheetFrame(sheet, p) {
  // The hint has a slot of its OWN, in the sheet's column. It used to be
  // appended into the card row — which is the fan's host, a centred flex ROW —
  // so as soon as a hint and a fan existed together the sentence became a
  // four-word-wide column squeezed in beside the cards, and the cards were
  // pushed off centre to make room for it.
  sheet.innerHTML = `<div class="pmsg">${sym(p.msg || "")}</div>
    <div class="cardprompt"></div>
    <div class="phint"></div>
    <div class="fanrail" style="display:none"></div>
    <div class="pbtns"></div>`;
  return {
    row: sheet.querySelector(".cardprompt"),
    hint: sheet.querySelector(".phint"),
    btns: sheet.querySelector(".pbtns"),
  };
}

/* A target announcement (CR 1.15.2), drawn as the cards it is about.
   The same list drives the board's gold outlines, so the two never disagree;
   the row exists because half the time the candidates are somewhere the board
   cannot show them at all — the stack mid-search, the heap, HQ during an
   access — and a highlight on a card that is not on screen highlights
   nothing. Picking accumulates until the count is met (or the player says
   they are done), exactly as tapping the board does. */
function renderSelectPrompt(sheet, p, choices) {
  // Where every candidate is already drawn on the board, no sheet rises at
  // all (`boardQuestion`): the board asks, the rail carries Done and the
  // rest. This renderer only ever sees the candidates the board CANNOT
  // draw — a stack mid-search, the heap, HQ during an access.
  const cards = p["select-cards"] || [];
  const picked = new Set(p["select-picked"] || []);
  // Discard is NOT a different interaction. It used to be: the taps "marked"
  // cards, which was a verb this UI used nowhere else and an affordance of
  // its own to learn. Choosing a card out of a pool is ONE thing everywhere —
  // tap to focus, tap the focused card to choose it — and 5.5.4c's
  // irreversibility is answered by what it always was: the choice
  // accumulates, in white (THE LAW §3), and a separate button ends it. That
  // button is the multi-pick's "done", not a second way to pick.
  const staging = p["select-kind"] === "discard";
  const ready = p["select-confirm"] === true;
  const { row, hint, btns } = promptSheetFrame(sheet, p);
  if (staging) {
    // The server's sentence already carries "(n of m chosen)"; the last phase
    // replaces it outright, because the question itself has changed.
    if (ready) {
      sheet.querySelector(".pmsg").textContent =
        `Discard ${picked.size} card${picked.size === 1 ? "" : "s"}? Tap one again to keep it.`;
    }
    sheet.classList.toggle("confirming", ready);
  }
  renderFan(row, cards, {
    key: "prompt",
    rail: sheet.querySelector(".fanrail"),
    repaint: () => renderSelectPrompt(sheet, p, choices),
    // Same read surface as renderCardPrompt: the viewer's own facedown
    // cards show the face 1.21.2a entitles them to; unseen ones stay blind.
    cardOpts: { side: mySide, reveal: true },
    // The glow and the ✓ are `cardEl`'s, from the one ladder in `glowClass` —
    // gold for a candidate, WHITE for one you have staged — so the sheet's
    // copy and the board's copy of the same card can never disagree. Nothing
    // here touches the geometry: this fan and the hand are one widget.
    slotOpts: (c, idx) => ({
      extra: (picked.has(c.cid) ? "on" : "") + (c.title ? "" : " blind"),
      label: c.title ? null : `Unseen card ${idx + 1}`,
    }),
  });
  // §6: an empty answer is stated, never implied — a prompt asking for a card
  // when no card qualifies has to SAY so, or it is indistinguishable from a
  // bug.
  if (!cards.length) {
    hint.appendChild(el("div", "picker-hint", "No card qualifies — there is nothing to choose."));
  } else {
    hint.appendChild(el("div", "picker-hint", FAN_PICK_HINT));
  }
  if (staging) {
    // The multi-pick's DONE — the same role `select-done` plays below, worded
    // as what it will do because this one cannot be taken back. Enabled only
    // once the set is the size the rule asks for: a done button that can
    // commit a half-answer is a trap, not a gate.
    const go = el("button", "chip go confirm",
      `Done — discard ${picked.size} card${picked.size === 1 ? "" : "s"}`);
    go.disabled = !ready;
    go.onclick = () => act("select-confirm", {});
    btns.appendChild(go);
    if (picked.size) {
      const clear = el("button", "chip", "Start over");
      clear.onclick = () => act("select-clear", {});
      btns.appendChild(clear);
    }
  }
  if (p["select-done"]) {
    // "Done (0 chosen)" is the wrong sentence for a question that never had
    // an answer to choose — nothing was declined, there was simply nothing
    // there. CR 1.15.2b clamped the announcement to the targets that exist,
    // and acknowledging that is an OK, not a tally.
    const empty = !(p["select-cards"] || []).length;
    const done = el("button", "chip go", empty ? "OK" : `Done (${picked.size} chosen)`);
    done.onclick = () => act("select-done", {});
    btns.appendChild(done);
  }
  // "None", "Pass" and anything else the decision offers besides the cards.
  choices.filter((ch) => !ch.card).forEach((ch) => {
    const b = el("button", "chip", abilityText(ch.value, null, true));
    b.onclick = () => act("choice", { choice: { uuid: ch.uuid } });
    btns.appendChild(b);
  });
}

/* ── the picker: search, preview, THEN commit ────────────────────────────
   Naming is irrevocable and often unhelpful if you misremember what a card
   does, so the pick is a two-step: filter to it, read it, then commit. The
   preview is the same renderer the board uses, so a named card looks like
   every other card you have read this game. */
const PICKER_THRESHOLD = 12;
const PICKER_SHOWN = 40;
let pickerKey = null;   // which prompt the state belongs to
let pickerQuery = "";
let pickerPick = null;  // the chosen choice object, not yet committed
const cardMetaCache = new Map();

function choiceLabel(ch) {
  const v = ch.value;
  return typeof v === "object" && v ? (v.title || "card") : String(v);
}

async function cardMeta(title) {
  if (cardMetaCache.has(title)) return cardMetaCache.get(title);
  let hit = null;
  try {
    const list = await api(`/api/cards?q=${encodeURIComponent(title)}`);
    hit = (list || []).find((c) => c.title === title) || null;
  } catch (_) { /* preview is a courtesy; a lookup failure must not block the pick */ }
  cardMetaCache.set(title, hit);
  return hit;
}

function renderPickerPrompt(sheet, p, choices) {
  const key = `${p.msg}|${choices.length}`;
  // Everything this sheet shows is a function of the DECISION (its message
  // and its fixed choice list) plus local search state — none of which a
  // state push can change. So a re-render for the same decision is a no-op,
  // and it has to be: a timed game re-pushes the same decision once a second,
  // and rebuilding the search box under a player who is typing into it would
  // drop their caret and, on a phone, their keyboard with it.
  if (pickerKey === key && sheet.querySelector(".picker-input")) return;
  if (pickerKey !== key) { pickerKey = key; pickerQuery = ""; pickerPick = null; }

  sheet.innerHTML = `
    <div class="pmsg">${sym(p.msg || "")}</div>
    <input class="picker-input" type="text" autocomplete="off" spellcheck="false"
           placeholder="type to search ${choices.length} cards">
    <div class="picker-body">
      <div class="picker-list"></div>
      <div class="picker-preview"></div>
    </div>
    <div class="pbtns picker-commit"></div>`;

  const input = sheet.querySelector(".picker-input");
  const list = sheet.querySelector(".picker-list");
  const preview = sheet.querySelector(".picker-preview");
  const commit = sheet.querySelector(".picker-commit");

  const paintPreview = async () => {
    if (!pickerPick) {
      preview.innerHTML = `<div class="picker-hint">Pick a card to see it here.</div>`;
      return;
    }
    const title = choiceLabel(pickerPick);
    preview.innerHTML = "";
    preview.appendChild(el("div", "picker-hint", `${title}…`));
    const c = await cardMeta(title);
    // The pick may have moved on while the lookup was in flight.
    if (!pickerPick || choiceLabel(pickerPick) !== title) return;
    if (c) {
      preview.innerHTML = cardInfoHtml(c);
    } else {
      preview.innerHTML = "";
      preview.appendChild(el("h3", "", title));
      preview.appendChild(el("div", "picker-hint", "No card data — naming it still works."));
    }
  };

  const paintCommit = () => {
    commit.innerHTML = "";
    const b = document.createElement("button");
    b.className = "chip go";
    b.disabled = !pickerPick;
    b.textContent = pickerPick ? `Name ${choiceLabel(pickerPick)}` : "Name…";
    b.onclick = () => {
      if (!pickerPick) return;
      const uuid = pickerPick.uuid;
      pickerKey = null; pickerQuery = ""; pickerPick = null;
      act("choice", { choice: { uuid } });
    };
    commit.appendChild(b);
  };

  const paintList = () => {
    const q = pickerQuery.trim().toLowerCase();
    const hits = choices.filter((ch) => choiceLabel(ch).toLowerCase().includes(q));
    list.innerHTML = "";
    hits.slice(0, PICKER_SHOWN).forEach((ch) => {
      const row = document.createElement("button");
      row.className = "picker-row" + (pickerPick && pickerPick.uuid === ch.uuid ? " on" : "");
      row.textContent = choiceLabel(ch);
      // Selecting only previews. Committing is the separate button, because
      // naming cannot be taken back.
      row.onclick = () => { pickerPick = ch; paintList(); paintPreview(); paintCommit(); };
      list.appendChild(row);
    });
    if (!hits.length) {
      // §12.6: the query is a user string, so it reaches the DOM as text,
      // never as markup.
      list.innerHTML = "";
      list.appendChild(el("div", "picker-hint", `Nothing matches “${pickerQuery}”.`));
    } else if (hits.length > PICKER_SHOWN) {
      const more = document.createElement("div");
      more.className = "picker-hint";
      more.textContent = `…and ${hits.length - PICKER_SHOWN} more — keep typing.`;
      list.appendChild(more);
    }
  };

  input.value = pickerQuery;
  input.oninput = () => { pickerQuery = input.value; paintList(); };
  paintList(); paintPreview(); paintCommit();
  // Autofocus only where a keyboard is already present — on a phone it would
  // throw up the on-screen keyboard over the board.
  if (hoverCapable) input.focus();
}

/* ── access reader ───────────────────────────────────────────────────────
   Accessing a card is the moment you most need to SEE it, so the card is
   the prompt: full art and text, the question underneath ("Trash for 3⬡?"),
   and a peek toggle that steps the reader aside so the board can be read
   and stepped back into — no decision is lost while peeking. */
let peekingBoard = false;
let accessFocusCid = null;

function accessOverlayEl() {
  let o = document.getElementById("access-overlay");
  if (!o) {
    o = document.createElement("div");
    o.id = "access-overlay";
    o.className = "zoom-overlay";
    document.getElementById("screen-game").appendChild(o);
  }
  return o;
}

function hideAccessReader() {
  const o = document.getElementById("access-overlay");
  if (o) o.style.display = "none";
  const pb = document.getElementById("peek-back");
  if (pb) pb.remove();
  peekingBoard = false;
  accessFocusCid = null;
}

function renderAccessReader(p) {
  const c = p.card;
  // A new card to look at always starts un-peeked.
  if (accessFocusCid !== c.cid) { accessFocusCid = c.cid; peekingBoard = false; }
  const o = accessOverlayEl();

  if (peekingBoard) {
    o.style.display = "none";
    let pb = document.getElementById("peek-back");
    if (!pb) {
      pb = document.createElement("button");
      pb.id = "peek-back";
      pb.className = "peek-back";
      document.getElementById("screen-game").appendChild(pb);
    }
    pb.textContent = `↩ ${c.title}`;
    pb.onclick = () => { peekingBoard = false; renderAccessReader(p); };
    return;
  }
  const pb = document.getElementById("peek-back");
  if (pb) pb.remove();

  const choices = p.choices || [];
  const yes = choices.find((ch) => /^(pay|rez|steal)/i.test(String(ch.value)));
  const no = choices.find((ch) => /^no action$/i.test(String(ch.value)));
  const tc = p["trash-cost"];
  const q =
    tc != null ? `Trash <b>${c.title}</b> for <span class="cost">${tc}⬡</span>?` :
    p.focus === "rez" ? `Rez <b>${c.title}</b>${c.cost != null ? ` for <span class="cost">${c.cost}⬡</span>` : ""}?` :
    yes && /^steal/i.test(String(yes.value)) ? `Steal <b>${c.title}</b>?` :
    // Any other decision ABOUT one card asks its own question; the reader is
    // only the frame. (7.4.6a's "access it too?" is not an access at all, and
    // saying "you accessed" there would be a lie.)
    p.focus !== "access" ? sym(p.msg || "") :
    `You accessed <b>${c.title}</b>.`;
  const binary = tc != null || p.focus === "rez";

  const btns = [];
  // The reader is showing this very card, so its options need not repeat it.
  if (yes) btns.push([binary ? "Yes" : abilityText(yes.value, c.title, false), "yes", yes.uuid]);
  if (no) btns.push([binary ? "No" : "No action", "no", no.uuid]);
  if (!btns.length) choices.forEach((ch) => btns.push([abilityText(ch.value, c.title, false) || "OK", "no", ch.uuid]));

  o.style.display = "flex";
  o.innerHTML = `<div class="zoom-card">${cardInfoHtml(c)}
    <div class="access-q">${q}</div>
    <div class="access-actions">
      ${btns.map(([l, cls, uuid]) => `<button class="chip ${cls}" data-uuid="${uuid}">${l}</button>`).join("")}
      <button class="chip peek" id="access-peek">Peek board</button>
    </div></div>
    <div class="tapaway">tap away to peek the board</div>`;
  o.querySelectorAll("[data-uuid]").forEach((b) => {
    b.onclick = () => { peekingBoard = false; act("choice", { choice: { uuid: b.dataset.uuid } }); };
  });
  const peek = () => { peekingBoard = true; renderAccessReader(p); };
  document.getElementById("access-peek").onclick = peek;
  // Tapping away from a reader that is ALSO a decision cannot simply throw
  // the decision away — but it must not trap the player either. So it does
  // what the Peek button does: steps aside, leaving the ↩ tab that steps
  // back. Nothing is lost and nothing is stuck.
  dismissOnTapAway(o, (e) => !!e.target.closest(".zoom-card"), peek);
}

/* ── the accessed card, whether or not it asks you anything ──────────────
   CR 7.1.2 entitles the Runner to look at a card they are accessing. Most
   accesses offer no decision at all — an agenda with no counters, an ice, an
   upgrade with no trash cost — so there was no prompt, so nothing was drawn,
   and the only trace of the card was one line in the log drawer: "Runner:
   accesses 24/7 News Cycle from R&D". That is a card you are entitled to see
   rendered as a sentence you have to go looking for.

   The server carries every access as a SNAPSHOT (`state.accessed`), taken at
   the instant the entitlement is live, because by the time a state is pushed
   the access is long over and `vm.st.accessed` is already null — which is why
   sending the live field alone would have shown nothing in exactly the case
   this is about. Each snapshot has a sequence number; the client shows what
   it has not yet dismissed, and dismissing is one tap anywhere. */
let accessSeen = 0;      // the highest reveal sequence already dismissed

function pendingAccesses() {
  if (!S || mySide !== "runner") return [];
  const all = S.accessed || [];
  // A fresh game counts from 1 again. A floor above everything the server is
  // offering can only mean the sequence restarted, and a stale floor would
  // silently swallow the first accesses of the new game.
  const top = all.reduce((m, a) => Math.max(m, a.seq || 0), 0);
  if (top < accessSeen) accessSeen = 0;
  return all.filter((a) => (a.seq || 0) > accessSeen && a.card && a.card.title);
}

/* The reveals still owed to the player, minus the card the mid-access reader
   is about to show full-size itself — one question for two callers: the
   reveal overlay shows these, and `renderPrompt` holds the decision behind
   them, so the two can never disagree about whose beat it is. */
function pendingReveals() {
  const p = myPrompt();
  const shown = p && p.card && p.card.cid;
  return pendingAccesses().filter((a) => a.card.cid !== shown);
}

function revealOverlayEl() {
  let o = document.getElementById("reveal-overlay");
  if (!o) {
    o = document.createElement("div");
    o.id = "reveal-overlay";
    o.className = "zoom-overlay reveal-overlay";
    document.getElementById("screen-game").appendChild(o);
  }
  return o;
}

function renderAccessReveal() {
  const o = document.getElementById("reveal-overlay");
  const hide = () => { if (o) o.style.display = "none"; };
  if (S && S.winner) { hide(); return; }
  const list = pendingAccesses();
  if (!list.length) { hide(); return; }

  // The mid-access reader is already showing one of these, full size, with
  // its question attached. Two overlays for one card is the fault above, not
  // the fix: mark those seen and let the reader do the work.
  const p = myPrompt();
  const shown = p && p.card && p.card.cid;
  const rest = list.filter((a) => a.card.cid !== shown);
  if (rest.length !== list.length) {
    accessSeen = Math.max(accessSeen, ...list.filter((a) => a.card.cid === shown).map((a) => a.seq));
  }
  if (!rest.length) { hide(); return; }

  // ONE card at a time, oldest first. The kernel resolves a breach access by
  // access (CR 7.5 — each access is its own step), and the presentation now
  // keeps that shape: the reveal is a single card, whole — art, text, where
  // it came from — acknowledged before the next appears, so it is never
  // ambiguous which card is in front of the player. "You accessed 3 cards"
  // over a row of thumbnails was a summary of a sequence, and cards from
  // Archives — a whole pile arriving at once — were the worst of it.
  // Decisions interleave in the same order: `renderPrompt` holds a decision
  // behind the reveals that predate it, so "access A, steal B" reads as A,
  // then B, exactly as it resolved.
  rest.sort((a, b) => (a.seq || 0) - (b.seq || 0));
  const cur = rest[0];
  const more = rest.length - 1;
  const ov = revealOverlayEl();
  // Already showing exactly this card: leave it standing. A reveal is a beat
  // the player acknowledges, and in a timed game the once-a-second sync would
  // otherwise tear the overlay down and build it again under their thumb —
  // restarting its entrance every second and replacing the button mid-tap.
  if (ov.style.display === "flex" && ov.dataset.seq === String(cur.seq || 0)) return;
  ov.dataset.seq = String(cur.seq || 0);
  ov.style.display = "flex";
  ov.innerHTML = "";
  const card = el("div", "zoom-card");
  // An eyebrow, not a second title: `cardInfoHtml` prints the card's own
  // name, and printing it twice reads as a bug rather than as emphasis.
  card.appendChild(el("div", "acc-eyebrow",
    (cur.from ? `You accessed — from ${cur.from}` : "You accessed") +
    (more ? ` · ${more} more to see` : "")));
  // §12.6: card text is the card layer's, never a user string.
  const body = document.createElement("div");
  body.innerHTML = cardInfoHtml(cur.card);
  while (body.firstChild) card.appendChild(body.firstChild);
  const ok = el("button", "chip go", more ? `Next card — ${more} more` : "Got it");
  // Dismissing acknowledges THIS card only: the floor rises to its seq, and
  // the re-render brings the next reveal (or the decision that was waiting
  // behind them) on its own beat.
  const done = () => { accessSeen = Math.max(accessSeen, cur.seq || 0); ov.style.display = "none"; render(); };
  ok.onclick = done;
  card.appendChild(ok);
  ov.appendChild(card);
  ov.appendChild(el("div", "tapaway", more ? "tap away for the next card" : "tap away to close"));
  // Tapping away advances, never abandons: each card still gets its own
  // acknowledged beat, and Escape does the same. Nothing here traps anybody
  // — every accessed card stays in the log either way.
  dismissOnTapAway(ov, (e) => !!e.target.closest(".zoom-card"), done);
}

/* ── chips / turn / run ──────────────────────────────────────────────── */
function has(cmd) { return ACTIONS.some((a) => a.command === cmd); }

function renderChips() {
  const bar = $("action-bar");
  bar.innerHTML = "";
  const mk = (label, fn, cls) => {
    const b = document.createElement("button");
    b.className = "chip" + (cls ? " " + cls : "");
    b.textContent = label; b.onclick = fn;
    bar.appendChild(b);
  };
  // A card is armed: say so, and give the way out a NAME. Tapping the table
  // already cancels, but a way out you have to guess at is not one a player
  // under time pressure will find — and this is the last moment before an
  // option is chosen and 9.2.7f makes it resolve to the end.
  if (armed != null && !runPick) {
    const b = document.createElement("button");
    b.className = "chip cancel-armed";
    b.textContent = "Cancel";
    b.onclick = () => { closeSheet(); disarm(); };
    bar.appendChild(b);
  }
  // A BOARD QUESTION docks everything that is not on the board HERE — the
  // rail is chrome the player already owns, and it never stands over a card.
  // The armed hint names what the next tap commits; the chips are the
  // choices that are no place on the table (Pass, Your rig, Done) plus the
  // select machinery (a discard's staging button, "Done (n chosen)").
  const p = myPrompt();
  if (p && boardQuestion(p)) {
    if (armed != null) {
      bar.appendChild(el("span", "armed-hint", `${armedName()} — tap again to confirm`));
    }
    (p.choices || []).filter((ch) => !ch.card && !ch.server).forEach((ch) => {
      // A chip in the rail stands alone: an ability names its own card.
      mk(abilityText(ch.value, ch.card && ch.card.title, true), () => act("choice", { choice: { uuid: ch.uuid } }), "prompt-chip");
    });
    const picked = (p["select-picked"] || []).length;
    // A FLOOR SAYS SO. A mandatory choice renders no "Done" at all until it
    // is met (the server only offers one when the selection is complete), so
    // a player who has armed a card and is looking for the button has
    // nothing on screen telling them the question is not answered yet — the
    // Archangel shape, where one more tap was all that was owed. The count
    // is stated wherever it is short, whether or not a Done exists.
    const floor = (p["select-min"] ?? 0) - picked;
    if (floor > 0 && (p["select-cards"] || []).length) {
      bar.appendChild(el("span", "armed-hint", `Choose ${floor} more`));
    }
    if (p["select-kind"] === "discard") {
      // 5.5.4c is irreversible: the staged set commits through this button
      // and nothing else. Same gate as the sheet it replaces.
      const ready = p["select-confirm"] === true;
      const go = document.createElement("button");
      go.className = "chip go confirm prompt-chip";
      go.textContent = `Done — discard ${picked} card${picked === 1 ? "" : "s"}`;
      go.disabled = !ready;
      go.onclick = () => act("select-confirm", {});
      bar.appendChild(go);
      if (picked) mk("Start over", () => act("select-clear", {}), "prompt-chip");
    } else if (p["select-done"]) {
      // A MINIMUM IS A MINIMUM. "Done" used to send whatever was picked,
      // including nothing, however many the question required — so a Corp
      // who had pointed at a card (armed it, white ring) and then pressed
      // Done sent an EMPTY answer, and Archangel's trace added nothing to
      // the grip. The button now says how many are still owed and cannot be
      // pressed until they are; the only way to send nothing is a question
      // that actually permits it.
      const empty = !(p["select-cards"] || []).length;
      const min = p["select-min"] ?? 0;
      const short = !empty && picked < min;
      const go = document.createElement("button");
      go.className = "chip go prompt-chip";
      go.textContent = empty ? "OK"
        : short ? `Choose ${min - picked} more` : `Done (${picked} chosen)`;
      go.disabled = short;
      go.onclick = () => act("select-done", {});
      bar.appendChild(go);
    }
    return;
  }
  // Choosing a server to run is a question the BOARD is asking (see
  // `runPick`): the rail carries only the hint and the way out, exactly as
  // it does for a board question the server asked.
  if (runPick) {
    bar.appendChild(el("span", "armed-hint", armed != null
      ? `Run on ${armedName()} — tap again to confirm`
      : "Tap the server you want to run"));
    mk("Cancel", () => cancelRunPick(), "cancel-armed");
    return;
  }
  if (native()) {
    if (has("credit")) mk("Gain 1 ⬡", () => act("credit"));
    if (has("draw")) mk("Draw a card", () => act("draw"));
    if (has("remove-tag")) mk("Remove tag (2⬡)", () => act("remove-tag"));
    if (has("purge")) mk("Purge viruses (●●●)", () => act("purge"));
    if (has("trash-resource")) mk("Trash a resource (2⬡)", () => act("trash-resource"));
    const runs = ACTIONS.filter((a) => a.command === "run");
    if (runs.length) mk("Run", () => setRunPick(
      new Map(runs.map((a) => [a.server, () => act("run", { server: a.server })]))));
  } else {
    const myTurn = S["active-player"] === mySide;
    if (myTurn && !myPrompt()) {
      mk("+1 ⬡", () => act("credit"));
      mk("Draw", () => act("draw"));
      if (mySide === "runner") mk("Run", () => {
        const servers = Object.keys((S.corp || {}).servers || {});
        setRunPick(new Map(servers.map((k) => [k, () => act("run", {
          server: k === "hq" || k === "rd" || k === "archives"
            ? k.toUpperCase().replace("RD", "R&D")
            : "Server " + k.replace("remote", ""),
        })])));
      });
      if (mySide === "runner") mk("Untag", () => act("remove-tag"));
    }
  }
}

function renderTurnBtn() {
  const btn = $("turn-btn");
  let label = null, cmd = null, ready = false;
  if (native()) {
    if (has("start-turn")) { label = "START TURN"; cmd = "start-turn"; ready = true; }
    else if (has("end-turn")) {
      label = "END TURN"; cmd = "end-turn";
      ready = ACTIONS.length <= 2 || (me().click ?? 0) === 0;
    }
  } else if (S) {
    const myTurn = S["active-player"] === mySide;
    if (S["end-turn"] && !myTurn) { label = "START TURN"; cmd = "start-turn"; ready = true; }
    else if (myTurn && !myPrompt() && !S.run) { label = "END TURN"; cmd = "end-turn"; ready = (me().click ?? 0) === 0; }
  }
  if (!label) { btn.style.display = "none"; return; }
  btn.style.display = "";
  btn.textContent = label;
  btn.classList.toggle("ready", ready);
  btn.onclick = () => act(cmd);
}

function renderRunControls() {
  const rc = $("run-controls");
  rc.innerHTML = "";
  const run = S.run;
  const show = run && mySide === "runner" && !myPrompt() &&
    (mode === "bridge" || has("continue") || has("jack-out"));
  if (!show) { rc.style.display = "none"; return; }
  rc.style.display = "flex";
  const mkbtn = (label, cls, fn) => {
    const b = document.createElement("button");
    b.className = "chip " + cls; b.textContent = label; b.onclick = fn;
    rc.appendChild(b);
  };
  if (mode === "bridge" || has("continue")) mkbtn("Continue", "continue", () => act("continue"));
  if (mode === "bridge" ? run.phase === "movement" : has("jack-out")) mkbtn("Jack out", "jackout", () => act("jack-out"));
}

function renderPhasePill() {
  const pill = $("phase-pill");
  const run = S.run;
  if (!run) { pill.style.display = "none"; return; }
  pill.style.display = "";
  const server = SERVER_NAME(String(run.server[0]).replace(":", ""));
  const phase = String(run.phase || "").replace(":", "");
  const nice = {
    "initiation": "Initiating run",
    "approach-ice": `Approaching ice (${run.position})`,
    "encounter-ice": "ENCOUNTER",
    "movement": "Movement — jack out?",
    "approach-server": "Approaching " + server,
    "success": "Breaching " + server,
  }[phase] || phase;
  pill.textContent = `${server}: ${nice}`;
}

/* ── log ─────────────────────────────────────────────────────────────── */

/* THE READER'S PLACE IN THE LOG IS THEIRS.
 *
 * Reported from a real game: "the scroll resets to all the way bottom every
 * time". A timed game pushes state once a second (the timing ticker), every
 * push runs `render`, and `renderLog` used to rebuild the list and slam
 * `scrollTop` to the end — so a player who scrolled up to check what an ice
 * did was dragged back down within the second, every second, and could not
 * read their own history at all.
 *
 * The discipline is the standard one, and it has three parts.
 *
 * FOLLOWING is a state, not a default. While the reader is at the bottom, new
 * lines pull the view down, because that is what being at the bottom means.
 * The moment they scroll away from it they stop following and nothing moves
 * the view but them.
 *
 * THE ANCHOR IS A LINE, NOT A NUMBER. `scrollTop` is not preservable here: the
 * list is a window on the log (`slice(-200)`), the server drops lines off the
 * front of its own copy once it hits its cap, and a collapsed wait line can
 * grow a count and change height. All three change how much content sits ABOVE
 * the reader, and a restored `scrollTop` would land that much off. So the
 * position is remembered as "this line, this many pixels below the top edge"
 * and restored by finding that same line again — which is why every line
 * carries a stable key (`n`, minted once per line by the server and never
 * reused; see `push_line`).
 *
 * AND THE READER IS TOLD. Scrolled-up and unaware that six things happened is
 * its own failure, so a chip appears with the count and takes them back.
 *
 * Nothing here animates and nothing here hijacks: `scrollTop` is assigned
 * directly, only ever to a value the reader asked for, and only ever when the
 * list was actually rebuilt. */
/* (`logFollow` and `logSeenK` are declared with the rest of the session state
   at the top of the file: `connect` resets them, and it runs from a top-level
   restore before this line is ever evaluated.) */
/* Slack, in px, that still counts as "at the bottom": sub-pixel layout and
   fractional device pixels mean an honest bottom is rarely exactly 0. */
const LOG_FOOT = 24;

/* A line's identity, stable across rebuilds. The CR server stamps `n`; the
   older bridge server does not, and its log neither collapses nor drains, so
   the absolute index is stable there and serves. */
function logKey(l, i) { return l && l.n != null ? "n" + l.n : "i" + i; }

function logAtBottom(box) {
  return box.scrollHeight - box.scrollTop - box.clientHeight <= LOG_FOOT;
}

/* Everything currently in the log is now read. */
function logMarkRead() {
  const log = (S && S.log) || [];
  logSeenK = log.length ? logKey(log[log.length - 1], log.length - 1) : null;
}

/* Back to the newest line, and following again. The one path that moves the
   view on the reader's behalf, and it only ever runs from their own tap. */
function logToBottom() {
  const box = $("log-lines");
  logFollow = true;
  box.scrollTop = box.scrollHeight;
  logMarkRead();
  logPaintChip();
}

/* How many lines have arrived since the reader last saw the bottom. Counted
   backwards from the newest, so a log that has been trimmed at the front
   still gives an answer (an anchor that fell off the front means everything
   on screen is new, which is the honest count). */
function logUnread() {
  const log = (S && S.log) || [];
  if (logSeenK === null) return 0;
  let n = 0;
  for (let i = log.length - 1; i >= 0; i--) {
    if (logKey(log[i], i) === logSeenK) return n;
    n++;
  }
  return n;
}

function logPaintChip() {
  const chip = document.getElementById("log-new");
  if (!chip) return;
  const n = logFollow ? 0 : logUnread();
  chip.textContent = `↓ ${n} new`;
  chip.style.display = n > 0 ? "" : "none";
}

$("log-tab").onclick = () => {
  $("log-drawer").classList.add("open");
  // Opening the log is asking for the latest of it.
  logToBottom();
};
$("log-close").onclick = () => $("log-drawer").classList.remove("open");
$("concede-btn").onclick = () => {
  if (confirm("Concede the game?")) act("concede");
  $("log-drawer").classList.remove("open");
};
$("say-send").onclick = () => { send({ type: "say", msg: $("say-input").value }); $("say-input").value = ""; };
$("log-new").onclick = logToBottom;
/* The reader's own scrolling is the ONLY thing that decides whether they are
   following. Passive: this listener never cancels the gesture. */
$("log-lines").addEventListener("scroll", () => {
  const box = $("log-lines");
  const at = logAtBottom(box);
  if (at === logFollow) return;
  logFollow = at;
  if (at) logMarkRead();
  logPaintChip();
}, { passive: true });

function renderLog() {
  const box = $("log-lines");
  const log = S.log || [];
  // Chat exists where there is somebody to say it to.
  const human = mode === "bridge" || (mode === "cr" && S["opponent-bot"] === false);
  $("say-row").style.display = human ? "" : "none";
  // A push that did not change the log does not touch the list at all. Most
  // pushes in a timed game are exactly that — a clock tick — and rebuilding
  // for them is what turned "scroll up and read" into a fight with the
  // server. (It also kept dropping any text the reader had selected.)
  if (!dirty("log", log)) { logPaintChip(); return; }

  const shown = log.slice(-200);
  const first = log.length - shown.length;

  // WHERE THE READER IS, measured before anything moves: the topmost line
  // still on screen and how far its top edge sits below the viewport's, plus
  // the distance from the bottom as a fallback for when that line is gone.
  let anchor = null;
  if (!logFollow) {
    anchor = { k: null, dy: 0, fromBottom: box.scrollHeight - box.scrollTop };
    for (const el of box.children) {
      if (el.offsetTop + el.offsetHeight > box.scrollTop) {
        anchor.k = el.dataset.k;
        anchor.dy = el.offsetTop - box.scrollTop;
        break;
      }
    }
  }

  box.innerHTML = "";
  const byKey = new Map();
  shown.forEach((l, i) => {
    const d = document.createElement("div");
    const user = typeof l.user === "object" && l.user ? l.user.username : l.user;
    // The log records the move a player made, and a move made with a card
    // ability arrives carrying that ability's developer handle. A person's
    // chat line is their own words and is never touched; a system line goes
    // through `abilityText`, which rewrites only what it can prove is a
    // handle and hands everything else (the narration, "Runner: runs HQ.")
    // straight back. The speaker is peeled off first so the handle's own
    // "<card name>: " stamp is at the front where the proof looks for it.
    const chat = user && user !== "__system__";
    const raw = l.text || "";
    const spk = chat ? null : /^(Corp|Runner): ([\s\S]+)$/.exec(raw);
    const body = chat ? sym(raw)
      : spk ? `${spk[1]}: ${abilityText(spk[2], null, true)}`
      : abilityText(raw, null, true);
    // A wait line the server folded into its predecessor carries how many
    // times it was said. The count is the whole point of the fold: the fact
    // is stated once, and how often it was true is still on the record.
    const times = l.count > 1 ? ` ×${l.count}` : "";
    d.textContent = (chat ? user + ": " : "") + body + times;
    const k = logKey(l, first + i);
    d.dataset.k = k;
    byKey.set(k, d);
    box.appendChild(d);
  });
  // Tapping the newest line is the other way back to following — the reader
  // is already looking at the end of the log and says so.
  if (box.lastElementChild) box.lastElementChild.onclick = logToBottom;

  if (logFollow) {
    box.scrollTop = box.scrollHeight;
    logMarkRead();
  } else if (anchor) {
    const el = anchor.k != null ? byKey.get(anchor.k) : null;
    // The anchor line survived: put it back exactly where it was. It did not
    // (trimmed off the front): hold the distance to the newest line instead,
    // which is the closest thing to "unchanged" once the history under them
    // has been thrown away.
    box.scrollTop = el
      ? Math.max(0, el.offsetTop - anchor.dy)
      : Math.max(0, box.scrollHeight - anchor.fromBottom);
  }
  logPaintChip();
}

/* ── zoom / gameover / toast ─────────────────────────────────────────── */

/* Nothing that merely SHOWS you something may ever hold the board hostage.
   An overlay opened to read a card is closed by tapping away from it, on
   every input the platform has, and by Escape where there is a keyboard.
   `click` alone was not enough: a long-press opens the reader with the
   pointer already down, so the release lands on the overlay without a
   matching press and no `click` is ever synthesised — and on iOS a tap on a
   plain div is not guaranteed to produce one at all. `pointerdown` is the
   event every device does fire, so that is the one that closes.

   `hit` gets first refusal on the tap: it returns true when the tap meant
   something inside the overlay (a pile row to read), false to dismiss.

   BUT A DRAG IS NOT A TAP, and the reader is a SCROLLER. Acting on
   `pointerdown` meant the first touch of a scroll gesture was already an
   answer: a finger put down on a pile row opened that card before it had
   moved a pixel, and a finger put down anywhere else closed the reader and
   `preventDefault`ed the pan that was about to happen. A heap longer than
   the screen therefore could not be read AT ALL on a phone — the only
   gesture that reaches its bottom half was the one gesture that dismissed
   it. (On a desktop the wheel scrolls without a pointerdown, which is
   exactly why this survived: it worked for every mouse and for no finger.)

   So the press is RECORDED on `pointerdown` and only ACTED ON at
   `pointerup`, and only if the pointer stayed inside `FAN_SLOP` — the same
   slop, and the same law, the board's cards already follow: one gesture,
   one meaning. `tracking` is what makes a long-press safe: a reader opened
   mid-hold sees the release of a press it never saw begin, and a release
   without its own press is nobody's tap. Nothing is `preventDefault`ed any
   more — the browser owns the pan, which is what makes the list scroll. */
function dismissOnTapAway(o, hit, onClose) {
  const close = () => {
    o.style.display = "none";
    o.__dismiss = null;
    if (onClose) onClose();
  };
  o.__dismiss = close;
  o.onclick = null;
  let tracking = false, sx = 0, sy = 0;
  o.onpointerdown = (e) => { tracking = true; sx = e.clientX; sy = e.clientY; };
  o.onpointercancel = () => { tracking = false; };
  o.onpointerup = (e) => {
    if (!tracking) return;
    tracking = false;
    if (Math.abs(e.clientX - sx) > FAN_SLOP || Math.abs(e.clientY - sy) > FAN_SLOP) return;
    if (hit && hit(e)) return;
    close();
  };
}

// Escape closes whatever reader is open, topmost first. A keyboard is not
// the phone case, but a stuck overlay on a laptop is the same bug.
document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  for (const id of ["zoom-overlay", "access-overlay", "reveal-overlay"]) {
    const o = document.getElementById(id);
    if (o && o.style.display !== "none" && o.__dismiss) { o.__dismiss(); return; }
  }
  // Nothing open to dismiss, so Escape means the same as tapping the table:
  // whatever was singled out, is not any more. On a keyboard this is the
  // fastest way to take back a card you did not mean to reach for — on the
  // board and in the deck editor alike, because one key cannot mean "cancel
  // what you were pointing at" on one screen and nothing on the other.
  closeSheet();
  disarm();
  cancelRunPick();
  clearBuilderFocus();
});

/* Opening a reader is IDEMPOTENT: one press, one preview. Asking for the
   card the reader is already showing must not tear it down and pop it in
   again — that replay is what a stranded press timer looked like, and even
   with the timers guarded, two paths asking for one card is one animation
   too many. Re-asking is a no-op; anything else replaces the content. */
let zoomShowing = null;
function zoomCard(c) {
  hideHoverPreview();
  const o = $("zoom-overlay");
  // The overlay is born inside #screen-game; reparent it to <body> once so
  // the reader can cover ANY screen — the deck editor long-presses cards
  // too, and a reader opened under a hidden screen is a reader that never
  // happened. Idempotent, same move as showStrictRefusal's.
  if (o.parentElement !== document.body) document.body.appendChild(o);
  const key = c.cid != null ? "c" + c.cid : "t" + (c.title || "");
  if (o.style.display === "flex" && zoomShowing === key) return;
  zoomShowing = key;
  o.style.display = "flex";
  // The reader opens on the face the table is showing (a flipped identity
  // opens on its back); the switcher is how you look at the others.
  paintZoomFace(o, c, showingFace(c));
  // The switcher lives INSIDE the reader chrome: a tap on it turns the
  // card over, every other tap still closes (THE LAW §3b).
  dismissOnTapAway(o, (e) => !!e.target.closest(".facetabs"));
}

/* The FACE SWITCHER (CR 1.4): one tab per face when a card has several
   backs (Biotech, Méliès), a single ↺ toggle when it has exactly one
   (Nebula, Hoshiko, …). Switching repaints the whole reader — title, type
   line, text, art region — inside the same overlay: no reflow, no second
   reader. Single-faced cards get no chrome at all. */
function faceTabsHtml(c, face) {
  const backs = cardFaces(c);
  if (!backs.length) return "";
  if (backs.length === 1) {
    const other = face === 0 ? 1 : 0;
    return `<div class="facetabs"><button class="chip facetab" data-face="${other}">↺ ${esc(faceTitle(c, other))}</button></div>`;
  }
  const tab = (i) =>
    `<button class="chip facetab${i === face ? " on" : ""}" data-face="${i}">${esc(faceTitle(c, i))}</button>`;
  return `<div class="facetabs">${[...Array(backs.length + 1).keys()].map(tab).join("")}</div>`;
}
function paintZoomFace(o, c, face) {
  o.innerHTML = `<div class="zoom-card">${faceTabsHtml(c, face)}${cardInfoHtml(c, face)}</div>
    <div class="tapaway">tap anywhere to close</div>`;
  o.querySelectorAll(".facetab").forEach((b) => {
    b.onclick = () => paintZoomFace(o, c, +b.dataset.face);
  });
}

/* ── A PILE IS A GRID OF CARDS, IN THE ORDER THEY WERE PUT THERE ─────────
 *
 * It used to be a LIST: one row per card, a thumbnail and a name. A list is
 * the wrong object twice. A pile of thirty is thirty rows — a column taller
 * than any screen, which is how a heap became something you scroll rather
 * than something you read — and a row is not what a card looks like
 * anywhere else in this UI (THE LAW §5: cards render as cards).
 *
 * A grid of tiles fits a thirty-card heap on one screen, reads at a glance,
 * and is the same square tile the board draws, so a card looks like itself
 * wherever it is.
 *
 * ORDER IS THE POINT. The kernel appends to a discard pile as cards arrive
 * (`Zone::Discard(s) => push(id)`, never sorted, never reversed anywhere)
 * and the server ships that array untouched, so index order IS placement
 * order. The grid reads oldest-first, newest-last, and says so — because
 * "what did they just trash" and "what is at the bottom of this" are
 * different questions and a pile that does not commit to an order answers
 * neither. 4.4.2's "top of the pile" is the most recently placed card, which
 * is the last tile. */
function zoomPile(cards, title) {
  const o = $("zoom-overlay");
  zoomShowing = null;
  o.style.display = "flex";
  o.innerHTML = `<div class="zoom-card pile"><h3>${title}</h3>
    ${cards.length > 1 ? `<div class="pilehint">oldest first · newest last</div>` : ""}
    <div class="pilegrid"></div></div>
    <div class="tapaway">tap anywhere to close</div>`;
  const grid = o.querySelector(".pilegrid");
  if (!cards.length) {
    grid.appendChild(el("div", "zline", "none yet"));
  }
  cards.forEach((c, i) => {
    const cell = el("div", "pilecell");
    cell.dataset.i = String(i);
    // The card as the board draws it — square, cropped, named — so the pile
    // is legible without opening anything, and identical to what the player
    // already knows how to read.
    cell.appendChild(cardEl(c, { side: c.side || "corp", reveal: true, pile: true }));
    grid.appendChild(cell);
    if (!c.title) return;
    // THE LAW §5: every display mode previews.
    attachZoom(cell, c);
    if (hoverCapable) {
      cell.addEventListener("pointerenter", () => showHoverPreview(c, cell));
      cell.addEventListener("pointerleave", hideHoverPreview);
    }
  });
  // Tap a tile to read that card; tap anywhere else to close.
  dismissOnTapAway(o, (e) => {
    const cell = e.target.closest(".pilecell");
    const c = cell ? cards[+cell.dataset.i] : null;
    if (c && c.title) { zoomCard(c); return true; }
    return false;
  });
}

/* THE GAME ENDING IS NOT THE END OF LOOKING AT IT.
 *
 * This screen used to be a wall: it took the whole viewport the instant the
 * last agenda landed, and the only way past it was "New game", which reloads.
 * A player who wanted to see WHAT happened — the access that ended it, the
 * log, the board — had the result and nothing else. In the words of the one
 * it happened to: "I got kicked from the game before I could see what
 * happened. Just saw that I won."
 *
 * So it dismisses, like every other reader on this board (THE LAW §2:
 * nothing holds the table hostage), and the table stays exactly as the last
 * change left it — log scrollable, piles readable, every card still a card.
 * The result is one tap away again from the chip it leaves behind, because
 * a result you cannot get back to is the same wall with extra steps. */
let gameOverDismissed = null;   // the winner this seat has already read
function renderGameOver() {
  const o = $("gameover-overlay");
  const chip = $("result-chip");
  if (!S.winner) {
    o.style.display = "none";
    if (chip) chip.style.display = "none";
    gameOverDismissed = null;
    return;
  }
  const iWon = S.winner === mySide;
  const who = S.winner === "corp" ? "Corp" : "Runner";
  const line = `${who} wins${S.reason ? " — " + esc(S.reason) : ""}`;
  if (chip) {
    chip.style.display = gameOverDismissed === S.winner ? "" : "none";
    chip.textContent = iWon ? "🏁 You won" : "🏁 You lost";
    chip.onclick = () => { gameOverDismissed = null; render(); };
  }
  if (gameOverDismissed === S.winner) { o.style.display = "none"; return; }
  o.style.display = "flex";
  o.innerHTML = `<h1>${iWon ? "VICTORY" : "DEFEAT"}</h1>
    <div class="why">${line}</div>
    <div class="gobtns">
      <button class="big go alt" id="go-look">Back to the table</button>
      <button class="big go" id="go-new">New game</button>
    </div>`;
  o.querySelector("#go-look").onclick = () => { gameOverDismissed = S.winner; render(); };
  // The session token outlives the overlay for the same reason: it is what a
  // refresh reconnects with, and a player reading a finished board may well
  // refresh. It is dropped when they actually leave.
  o.querySelector("#go-new").onclick = () => {
    localStorage.removeItem("jinteki_local");
    location.reload();
  };
}

/* Fullscreen-ish chrome: lock landscape where the platform allows it
   (Android PWA), and teach iOS users the one real path to fullscreen —
   Add to Home Screen (standalone mode hides all Safari chrome). */
let chromeHinted = false;
function enterGameChrome() {
  if (screen.orientation && screen.orientation.lock) {
    screen.orientation.lock("landscape").catch(() => {});
  }
  if (chromeHinted) return;
  chromeHinted = true;
  const iOS = /iPhone|iPad/.test(navigator.userAgent);
  const standalone = window.navigator.standalone === true ||
    window.matchMedia("(display-mode: fullscreen), (display-mode: standalone)").matches;
  if (iOS && !standalone && !localStorage.getItem("jinteki_a2hs_hinted")) {
    localStorage.setItem("jinteki_a2hs_hinted", "1");
    setTimeout(() => toast("Fullscreen: Share → Add to Home Screen, then launch from there"), 1200);
  }
}

function toast(msg) {
  const t = $("toast");
  t.textContent = msg;
  t.style.display = "";
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.style.display = "none"; }, 2600);
}

/* ════════════════════════════════════════════════════════════════════════
   Accounts, decks, library, NRDB import (ACCOUNTS-AND-DECKS.md §9).
   Plain fetch() + the HttpOnly jrs_session cookie; the WS game channel
   above is untouched. Everything renders with textContent/escaped nodes —
   no user string ever meets innerHTML (§12.6).
   ═══════════════════════════════════════════════════════════════════════ */

let ME = null;              // /api/me summary; never persisted to localStorage
let selectedDeck = null;    // deck chosen for Play vs Bot ({id,name} | null)
let editDeck = null;        // deck being edited (client-side draft)
let importDraft = null;     // unsaved import draft awaiting Save
let libFilter = { side: "", q: "" };
let currentLibDeck = null;

async function api(path, opts) {
  const o = opts || {};
  const init = { method: o.method || "GET", headers: {} };
  if (o.body !== undefined) {
    init.headers["Content-Type"] = "application/json";
    init.body = JSON.stringify(o.body);
  }
  const res = await fetch(path, init);
  let data = null;
  try { data = await res.json(); } catch (e) { /* non-JSON */ }
  if (!res.ok) {
    const msg = (data && data.error) || `request failed (${res.status})`;
    throw new Error(msg);
  }
  return data;
}

/* Strict-mode refusal detail: the offending titles, one per line. */
function showStrictRefusal(msg) {
  show("screen-home");
  const o = $("zoom-overlay");
  // zoom-overlay lives inside screen-game; reparent to body once so it can
  // cover any screen. Idempotent.
  if (o.parentElement !== document.body) document.body.appendChild(o);
  o.style.display = "flex";
  o.textContent = "";
  const card = document.createElement("div");
  card.className = "zoom-card";
  const h = document.createElement("h3");
  h.textContent = "Deck not playable vs bot";
  card.appendChild(h);
  const intro = document.createElement("div");
  intro.className = "zline";
  intro.textContent = "These cards have no implemented behavior yet:";
  card.appendChild(intro);
  const list = msg.split(":").slice(1).join(":").trim();
  list.split(", ").forEach((t) => {
    const d = document.createElement("div");
    d.className = "ztext";
    d.textContent = "✗ " + t;
    card.appendChild(d);
  });
  o.appendChild(card);
  dismissOnTapAway(o, null);
}

/* ── build stamp + self-heal ─────────────────────────────────────────────
   The binary is the single source of the build id; the server stamps it into
   this script's own URL (app.js?v=<rev>). If the server has since moved on,
   the page we are running came from some cache — an iOS PWA shell, a bfcache
   entry, a proxy — so reload once through a URL that cannot be served stale.
   Guarded by sessionStorage so a genuinely broken deploy can't loop. */
(async function bootVersion() {
  const mine = new URL(document.currentScript?.src || location.href, location.href)
    .searchParams.get("v");
  try {
    const rev = (await (await fetch("/version", { cache: "no-store" })).text()).trim();
    const short = rev.slice(0, 12);
    const b = $("build"); if (b) b.textContent = short;
    const bl = $("build-log"); if (bl) bl.textContent = short;
    if (mine && rev && mine !== rev && sessionStorage.getItem("jrs_heal") !== rev) {
      sessionStorage.setItem("jrs_heal", rev);
      location.replace(`/?v=${encodeURIComponent(rev)}`);
    }
  } catch (e) { /* offline: placeholders stay empty */ }
})();

/* ── boot: identity first, then the auth-redirect toast ──────────────── */
const AUTH_TOASTS = {
  ok: "Signed in — this browser now holds your account",
  invalid: "That sign-in link is not valid — request a fresh one",
  expired: "That sign-in link expired — request a fresh one",
  conflict: "This account already has a different email",
};
(async function bootAccount() {
  try {
    ME = await api("/api/me");
  } catch (e) {
    ME = null; // offline / server without accounts: UI stays playable
  }
  renderAccountChip();
  const param = new URLSearchParams(location.search).get("auth");
  if (param) {
    history.replaceState(null, "", "/");
    if (AUTH_TOASTS[param]) toast(AUTH_TOASTS[param]);
    if (param === "ok") { try { ME = await api("/api/me"); } catch (e) {} renderAccountChip(); }
  }
})();

function renderAccountChip() {
  const chip = $("account-chip");
  if (!ME) { chip.textContent = "…"; return; }
  chip.textContent = ME.kind === "claimed" ? `✓ ${ME.display_name}` : ME.display_name;
  chip.classList.toggle("claimed", ME.kind === "claimed");
}

/* ── tiny DOM helpers (no innerHTML with user strings) ───────────────── */
function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}
function implBadge(playable) {
  const total = playable.behavior + playable.jnet_only + playable.unimplemented;
  const b = el("span", "badge-impl", `${playable.behavior}/${total}`);
  b.classList.add(playable.behavior === total ? "ok" : "warn");
  b.title = `${playable.behavior} of ${total} cards playable vs bot`;
  return b;
}
function legalBadge(legal) {
  return el("span", "badge-legal " + (legal ? "ok" : "bad"), legal ? "✓ legal" : "✗ illegal");
}

/* ── navigation wiring ───────────────────────────────────────────────── */
$("nav-decks").onclick = () => { show("screen-decks"); loadMyDecks(); };
$("nav-library").onclick = () => { show("screen-library"); loadLibrary(); };
$("account-chip").onclick = () => { show("screen-account"); renderAccountScreen(); };
$("decks-back").onclick = () => show("screen-home");
$("library-back").onclick = () => show("screen-home");
$("crgap-back").onclick = () => { show("screen-home"); loadCrReady(); };
$("account-back").onclick = () => show("screen-home");
$("edit-back").onclick = () => { show("screen-decks"); loadMyDecks(); };
$("import-back").onclick = () => show("screen-decks");
$("libdeck-back").onclick = () => show("screen-library");
$("decks-import").onclick = () => { resetImport(); show("screen-import"); };
$("decks-new").onclick = () => openEditor(null);

/* ── my decks ────────────────────────────────────────────────────────── */
async function loadMyDecks() {
  const box = $("decks-list");
  box.textContent = "loading…";
  let list;
  try { list = await api("/api/decks"); } catch (e) { box.textContent = e.message; return; }
  box.textContent = "";
  if (!list.length) {
    const empty = el("div", "deck-row");
    empty.appendChild(el("div", "t", "No decks yet — fork one from the Library or Import from NetrunnerDB."));
    box.appendChild(empty);
    return;
  }
  list.forEach((d) => box.appendChild(deckRow(d, () => openEditor(d.id))));
}

function deckRow(d, onOpen) {
  const row = el("div", "deck-row");
  const t = el("div", "t");
  const line1 = el("div", "", d.name);
  line1.appendChild(legalBadge(d.legal));
  line1.appendChild(implBadge(d.playable));
  const line2 = el("small", "", `${d.side === "corp" ? "⬢ Corp" : "⬡ Runner"} · ${d.identity.title}`);
  if (d.author_name) line2.textContent += ` · by ${d.author_name}`;
  if (d.published_at) line1.appendChild(el("span", "badge-pub", "published"));
  t.appendChild(line1);
  t.appendChild(line2);
  row.appendChild(t);
  const open = el("button", "chip go", "Open");
  open.onclick = onOpen;
  row.appendChild(open);
  return row;
}

/* ── deck editor ─────────────────────────────────────────────────────── */
async function openEditor(deckId) {
  if (deckId) {
    try { editDeck = await api(`/api/decks/${deckId}`); }
    catch (e) { toast(e.message); return; }
  } else {
    editDeck = { id: null, name: "", identity: { title: "", code: "" }, cards: [], notes: "",
                 validation: null, published_at: null };
  }
  show("screen-deck-edit");
  $("edit-name").value = editDeck.name;
  $("edit-notes").value = editDeck.notes || "";
  $("edit-search").value = "";
  $("edit-results").textContent = "";
  $("edit-title-label").textContent = editDeck.id ? "Edit deck" : "New deck";
  renderEditor();
}

function editorLines() {
  return (editDeck.cards || []).map((c) => ({ title: c.title, qty: c.qty }));
}

function renderEditor() {
  $("edit-identity").textContent = editDeck.identity.title || "no identity — search below and tap one";
  $("edit-publish").style.display = editDeck.id && !editDeck.published_at ? "" : "none";
  $("edit-unpublish").style.display = editDeck.id && editDeck.published_at ? "" : "none";
  $("edit-delete").style.display = editDeck.id ? "" : "none";
  const box = $("edit-cards");
  box.textContent = "";
  (editDeck.cards || []).forEach((c, i) => {
    const row = el("div", "deck-row");
    const t = el("div", "t");
    const l1 = el("div", "", `${c.qty}× ${c.title}`);
    const impl = c.impl_status || "unknown";
    const badge = el("span", "badge-impl " + (impl === "behavior" ? "ok" : "warn"),
      impl === "behavior" ? "✓" : impl === "jnet_only" ? "jnet" : "✗");
    badge.title = impl === "behavior" ? "fully playable vs bot"
      : impl === "jnet_only" ? "not yet implemented here (jinteki.net has it)"
      : "not implemented anywhere yet";
    l1.appendChild(badge);
    if (c.influence_spent) l1.appendChild(el("span", "badge-inf", "●".repeat(Math.min(c.influence_spent, 5))));
    t.appendChild(l1);
    row.appendChild(t);
    const minus = el("button", "chip", "−");
    minus.onclick = () => { c.qty -= 1; if (c.qty <= 0) editDeck.cards.splice(i, 1); revalidate(); };
    const plus = el("button", "chip", "+");
    plus.onclick = () => { c.qty += 1; revalidate(); };
    row.appendChild(minus);
    row.appendChild(plus);
    // long-press zoom reuses the game's card reader
    attachZoom(row, c);
    box.appendChild(row);
  });
  renderValidStrip();
}

/* Long-press to read, on anything that stands for a card but is not drawn as
   one: a pile row, a deck-editor row. THE LAW §5 — collapsing a card for
   space must never cost the ability to read what it says, so the WHOLE card
   goes to the reader. Passing only title+code showed a card with its rules
   text, counters and subroutines stripped, which is exactly the reading the
   press was asking for. */
function attachZoom(elm, c) {
  // A pile row and a deck-editor row both live in LISTS the player scrolls,
  // so this is the site the drag-opens-a-reader bug bit hardest.
  return pressToRead(elm, 420, () => zoomCard(c));
}

function renderValidStrip() {
  const strip = $("edit-valid");
  const v = editDeck.validation;
  if (!v) { strip.textContent = editDeck.identity.title ? "validating…" : ""; return; }
  strip.textContent = "";
  const c = v.counts;
  const sums = el("span", "",
    `${c.cards} cards · inf ${c.influence_used}/${c.influence_limit == null ? "∞" : c.influence_limit}` +
    (editDeck.side === "corp" || (editDeck.identity && v.counts.agenda_points > 0) ? ` · ${c.agenda_points} AP` : ""));
  strip.appendChild(legalBadge(v.legal));
  strip.appendChild(sums);
  if (v.playable) strip.appendChild(implBadge(v.playable));
  if (v.problems.length) {
    const n = el("button", "chip warn", `${v.problems.length} problem${v.problems.length > 1 ? "s" : ""}`);
    n.onclick = () => {
      const o = $("zoom-overlay");
      o.style.display = "flex";
      const card = el("div", "zoom-card");
      card.appendChild(el("h3", "", "Deck problems"));
      v.problems.forEach((p) => card.appendChild(el("div", "ztext", p.message)));
      (v.warnings || []).forEach((p) => card.appendChild(el("div", "zline", "⚠ " + p.message)));
      o.textContent = "";
      o.appendChild(card);
      o.onclick = () => { o.style.display = "none"; };
    };
    strip.appendChild(n);
  }
}

let validateTimer = null;
function revalidate() {
  renderEditor();
  clearTimeout(validateTimer);
  if (!editDeck.identity.title) return;
  validateTimer = setTimeout(async () => {
    try {
      const v = await api("/api/decks/validate", { method: "POST", body: {
        name: $("edit-name").value || "draft",
        identity: { title: editDeck.identity.title },
        cards: editorLines(),
      }});
      editDeck.validation = v;
      // refresh per-card impl/influence from the authoritative check
      const byTitle = {};
      (v.cards || []).forEach((cv) => { byTitle[cv.title] = cv; });
      (editDeck.cards || []).forEach((c) => {
        const cv = byTitle[c.title];
        if (cv) { c.impl_status = cv.impl_status; c.influence_spent = cv.influence_spent; c.code = cv.code; }
      });
      renderEditor();
    } catch (e) { /* keep the last strip on transient errors */ }
  }, 250);
}

let searchTimer = null;
$("edit-search").oninput = () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(doCardSearch, 200);
};
$("edit-pick-identity").onclick = () => {
  $("edit-search").value = "";
  $("edit-search").placeholder = "search identities";
  $("edit-search").focus();
  doCardSearch(true);
};

async function doCardSearch(identityMode) {
  const q = $("edit-search").value.trim();
  const box = $("edit-results");
  const wantIdentity = identityMode === true || $("edit-search").placeholder.includes("identities");
  if (!q && !wantIdentity) { box.textContent = ""; return; }
  const params = new URLSearchParams();
  params.set("q", q);
  if (wantIdentity) params.set("type", "Identity");
  else if (editDeck.side) {
    // side auto-locked to the identity (§9.2)
    params.set("side", editDeck.side === "corp" ? "Corp" : "Runner");
  }
  let list;
  try { list = await api(`/api/cards?${params}`); } catch (e) { box.textContent = e.message; return; }
  box.textContent = "";
  list.forEach((c) => {
    const row = el("div", "deck-row");
    const t = el("div", "t");
    const l1 = el("div", "", c.title);
    const badge = el("span", "badge-impl " + (c.impl === "behavior" ? "ok" : "warn"),
      c.impl === "behavior" ? "✓" : c.impl === "jnet_only" ? "jnet" : "✗");
    l1.appendChild(badge);
    t.appendChild(l1);
    t.appendChild(el("small", "", `${c.side} · ${c.type}${c.faction ? " · " + c.faction : ""}` +
      (c.influence_cost != null ? " · inf " + c.influence_cost : "")));
    row.appendChild(t);
    const add = el("button", "chip go", wantIdentity ? "Pick" : "Add");
    add.onclick = () => {
      if (wantIdentity || c.type === "Identity") {
        editDeck.identity = { title: c.title, code: c.code };
        editDeck.side = c.side === "Corp" ? "corp" : "runner";
        $("edit-search").placeholder = "add cards — search the pool";
        $("edit-search").value = "";
        box.textContent = "";
      } else {
        const have = editDeck.cards.find((x) => x.title === c.title);
        if (have) have.qty += 1;
        else editDeck.cards.push({ title: c.title, code: c.code, qty: 1 });
      }
      revalidate();
    };
    attachZoom(row, c);
    row.appendChild(add);
    box.appendChild(row);
  });
}

$("edit-save").onclick = async () => {
  if (!editDeck.identity.title) { toast("Pick an identity first"); return; }
  const body = {
    name: $("edit-name").value.trim() || "unnamed deck",
    identity: { title: editDeck.identity.title },
    cards: editorLines(),
    notes: $("edit-notes").value,
  };
  try {
    editDeck = editDeck.id
      ? await api(`/api/decks/${editDeck.id}`, { method: "PUT", body })
      : await api("/api/decks", { method: "POST", body });
    toast("Deck saved");
    renderEditor();
  } catch (e) { toast(e.message); }
};

$("edit-delete").onclick = async () => {
  if (!editDeck.id || !confirm("Delete this deck?")) return;
  try {
    await api(`/api/decks/${editDeck.id}`, { method: "DELETE" });
    show("screen-decks");
    loadMyDecks();
  } catch (e) { toast(e.message); }
};

$("edit-publish").onclick = async () => {
  if (ME && ME.kind !== "claimed") {
    toast("Claim your account with an email to publish");
    show("screen-account"); renderAccountScreen();
    return;
  }
  try {
    editDeck = await api(`/api/decks/${editDeck.id}/publish`, { method: "POST" });
    toast("Published to the library");
    renderEditor();
  } catch (e) { toast(e.message); }
};

$("edit-unpublish").onclick = async () => {
  try {
    editDeck = await api(`/api/decks/${editDeck.id}/unpublish`, { method: "POST" });
    toast("Removed from the library");
    renderEditor();
  } catch (e) { toast(e.message); }
};

/* ── NRDB import ─────────────────────────────────────────────────────── */
function resetImport() {
  importDraft = null;
  $("import-form").style.display = "";
  $("import-preview").style.display = "none";
  $("import-input").value = "";
}

$("import-go").onclick = async () => {
  const input = $("import-input").value.trim();
  if (!input) return;
  $("import-go").disabled = true;
  $("import-go").textContent = "Importing…";
  try {
    const res = await api("/api/decks/import", { method: "POST", body: { input } });
    importDraft = res.deck;
    renderImportPreview(res.deck, res.report);
  } catch (e) {
    toast(e.message);
  } finally {
    $("import-go").disabled = false;
    $("import-go").textContent = "Import";
  }
};

function renderImportPreview(deck, report) {
  $("import-form").style.display = "none";
  $("import-preview").style.display = "";
  const box = $("import-report");
  box.textContent = "";
  box.appendChild(el("h3", "", deck.name));
  box.appendChild(el("div", "hint", `${deck.side === "corp" ? "Corp" : "Runner"} · ${deck.identity.title}`));
  const v = report.validation;
  const roll = el("div", "import-roll");
  const total = v.playable.behavior + v.playable.jnet_only + v.playable.unimplemented;
  roll.appendChild(implBadge(v.playable));
  roll.appendChild(el("span", "", ` ${v.playable.behavior} of ${total} cards playable vs bot`));
  roll.appendChild(legalBadge(v.legal));
  box.appendChild(roll);
  const note = (label, items, cls) => {
    if (!items || !items.length) return;
    const d = el("div", cls || "zline", `${label}: ${items.join(", ")}`);
    box.appendChild(d);
  };
  note("Unknown codes (dropped)", report.unknown_codes, "import-bad");
  note("Rotated", report.rotated);
  if (report.via_previous_printing) {
    box.appendChild(el("div", "zline", `${report.via_previous_printing} resolved via previous printings`));
  }
  (v.problems || []).forEach((p) => box.appendChild(el("div", "import-bad", p.message)));
  const cards = $("import-cards");
  cards.textContent = "";
  (v.cards || []).forEach((c) => {
    const row = el("div", "deck-row");
    const t = el("div", "t");
    const l1 = el("div", "", `${c.qty}× ${c.title}`);
    const badge = el("span", "badge-impl " + (c.impl_status === "behavior" ? "ok" : "warn"),
      c.impl_status === "behavior" ? "✓" : c.impl_status === "jnet_only" ? "jnet" : "✗");
    l1.appendChild(badge);
    t.appendChild(l1);
    row.appendChild(t);
    attachZoom(row, c);
    cards.appendChild(row);
  });
}

$("import-discard").onclick = resetImport;
$("import-save").onclick = async () => {
  if (!importDraft) return;
  try {
    const saved = await api("/api/decks", { method: "POST", body: {
      name: importDraft.name,
      identity: { title: importDraft.identity.title },
      cards: importDraft.cards.map((c) => ({ title: c.title, qty: c.qty })),
      notes: importDraft.notes,
      source: importDraft.source,
    }});
    toast("Deck saved");
    resetImport();
    show("screen-decks");
    loadMyDecks();
  } catch (e) { toast(e.message); }
};

/* ── library ─────────────────────────────────────────────────────────── */
document.querySelectorAll("[data-lside]").forEach((b) => {
  b.onclick = () => {
    libFilter.side = b.dataset.lside;
    document.querySelectorAll("[data-lside]").forEach((x) => x.classList.toggle("on", x === b));
    loadLibrary();
  };
});
let libSearchTimer = null;
$("library-q").oninput = () => {
  clearTimeout(libSearchTimer);
  libSearchTimer = setTimeout(() => { libFilter.q = $("library-q").value.trim(); loadLibrary(); }, 250);
};

async function loadLibrary() {
  const box = $("library-list");
  box.textContent = "loading…";
  const params = new URLSearchParams();
  if (libFilter.side) params.set("side", libFilter.side);
  if (libFilter.q) params.set("q", libFilter.q);
  let res;
  try { res = await api(`/api/library?${params}`); } catch (e) { box.textContent = e.message; return; }
  box.textContent = "";
  if (!res.decks.length) {
    const empty = el("div", "deck-row");
    empty.appendChild(el("div", "t", "Nothing published yet."));
    box.appendChild(empty);
    return;
  }
  res.decks.forEach((d) => box.appendChild(deckRow(d, () => openLibDeck(d.id))));
}

async function openLibDeck(id) {
  let d;
  try { d = await api(`/api/library/${id}`); } catch (e) { toast(e.message); return; }
  currentLibDeck = d;
  show("screen-lib-deck");
  $("libdeck-title").textContent = d.name;
  const meta = $("libdeck-meta");
  meta.textContent = "";
  meta.appendChild(el("div", "", `${d.side === "corp" ? "Corp" : "Runner"} · ${d.identity.title}`));
  meta.appendChild(el("small", "hint", `by ${d.author_name}`));
  const v = d.validation;
  const roll = el("div", "import-roll");
  roll.appendChild(legalBadge(v.legal));
  roll.appendChild(implBadge(v.playable));
  meta.appendChild(roll);
  if (d.notes) meta.appendChild(el("div", "zline", d.notes));
  const cards = $("libdeck-cards");
  cards.textContent = "";
  (d.cards || []).forEach((c) => {
    const row = el("div", "deck-row");
    const t = el("div", "t");
    const l1 = el("div", "", `${c.qty}× ${c.title}`);
    const badge = el("span", "badge-impl " + (c.impl_status === "behavior" ? "ok" : "warn"),
      c.impl_status === "behavior" ? "✓" : c.impl_status === "jnet_only" ? "jnet" : "✗");
    l1.appendChild(badge);
    t.appendChild(l1);
    row.appendChild(t);
    attachZoom(row, c);
    cards.appendChild(row);
  });
}

$("libdeck-fork").onclick = async () => {
  if (!currentLibDeck) return;
  try {
    await api(`/api/library/${currentLibDeck.id}/fork`, { method: "POST" });
    toast("Forked to your decks");
    show("screen-decks");
    loadMyDecks();
  } catch (e) { toast(e.message); }
};

/* ── deck picker for Play vs Bot (§9.1: playable first, greyed rest) ── */
$("deck-select-chip").onclick = async () => {
  const sheet = $("deck-sheet");
  const list = $("deck-sheet-list");
  list.textContent = "loading…";
  sheet.style.display = "";
  let decks;
  try { decks = await api("/api/decks"); } catch (e) { list.textContent = e.message; return; }
  list.textContent = "";
  const starter = el("button", "chip deckpick", "Starter deck (built-in)");
  starter.onclick = () => { selectedDeck = null; renderDeckChip(); sheet.style.display = "none"; };
  list.appendChild(starter);
  const mine = decks.filter((d) => d.side === mySide);
  // Playable decks first (§9.1), then by how close the rest are.
  const playableFirst = mine.slice().sort((a, b) =>
    (a.playable.jnet_only + a.playable.unimplemented) -
    (b.playable.jnet_only + b.playable.unimplemented));
  playableFirst.forEach((d) => {
    const un = d.playable.jnet_only + d.playable.unimplemented;
    const playable = un === 0 && d.legal;
    const b = el("button", "chip deckpick" + (playable ? "" : " disabled"),
      `${d.name} — ${d.identity.title.split(":")[0]}` + (playable ? "" : ` (${un} not playable)`));
    if (playable) {
      b.onclick = () => { selectedDeck = { id: d.id, name: d.name }; renderDeckChip(); sheet.style.display = "none"; };
    } else {
      b.disabled = true;
      b.title = `${un} cards without implemented behavior — not selectable for bot games`;
    }
    list.appendChild(b);
  });
  if (!mine.length) list.appendChild(el("div", "hint", `No ${mySide} decks yet — fork one from the Library.`));
};
$("deck-sheet-close").onclick = () => { $("deck-sheet").style.display = "none"; };
function renderDeckChip() {
  $("deck-select-chip").textContent = "Deck: " + (selectedDeck ? selectedDeck.name : "Starter deck");
}
// Side toggle invalidates a picked deck of the other side (the server would
// refuse it anyway; don't let the mismatch reach the socket).
$("pick-runner").onclick = () => { pickSide("runner"); selectedDeck = null; renderDeckChip(); };
$("pick-corp").onclick = () => { pickSide("corp"); selectedDeck = null; renderDeckChip(); };

/* ── account screen ──────────────────────────────────────────────────── */
function renderAccountScreen() {
  if (!ME) return;
  $("account-name").value = ME.display_name || "";
  const status = $("account-status");
  status.textContent = "";
  if (ME.kind === "claimed") {
    status.appendChild(el("div", "acc-claimed", `Signed in as ${ME.email}`));
    $("account-claim").style.display = "none";
  } else {
    status.appendChild(el("div", "acc-anon", "Anonymous — your decks live in this browser's cookie"));
    $("account-claim").style.display = "";
  }
}

$("account-name").onblur = async () => {
  const name = $("account-name").value.trim();
  if (!ME || !name || name === ME.display_name) return;
  try {
    ME = await api("/api/profile", { method: "PUT", body: { display_name: name } });
    renderAccountChip();
    toast("Name saved");
  } catch (e) { toast(e.message); }
};

$("account-claim-btn").onclick = async () => {
  const email = $("account-email").value.trim();
  if (!email) return;
  $("account-claim-btn").disabled = true;
  try {
    await api("/api/auth/claim", { method: "POST", body: { email } });
    toast("Check your inbox — the link works for 30 minutes");
  } catch (e) { toast(e.message); }
  finally { $("account-claim-btn").disabled = false; }
};

$("account-logout").onclick = async () => {
  if (ME && ME.kind !== "claimed") {
    const n = "This browser's cookie is the only key to your decks. Claim with an email first?";
    if (!confirm(n + "\n\nLog out anyway and orphan them?")) return;
  }
  try { await api("/api/auth/logout", { method: "POST" }); } catch (e) {}
  ME = null;
  try { ME = await api("/api/me"); } catch (e) {}
  renderAccountChip();
  show("screen-home");
  toast("Logged out");
};

/* ════════════════════════════════════════════════════════════════════════
   The two-aspect home, the Find a Game shell, My Decks and the DECK
   BUILDER — coded against the deck-builder backend contract:

     GET  /api/catalog?format=eternal
          → {format, point_limit, identities:[CatalogCard], cards:[CatalogCard]}
       CatalogCard = {id, title, side, faction, type, influence_cost,
                      deck_limit, agenda_points, points, banned, draft_only,
                      min_deck_size, influence_limit}   (nullable → null)
     GET  /api/decks           → {decks:[{key, name, builtin, legal, …}]}
     GET  /api/decks/<key>     → {key, name, identity, cards:{id:count},
                                  legal, problems:[{code, message, card}]}
     POST /api/decks {name, identity, cards} / PUT /api/decks/<key>
                               → {key, legal, problems}
     DELETE /api/decks/<key>   (403 on builtins)

   Until those routes land, everything below falls back to a static stub:
   the fixtures in ui/dev-fixtures/ (list/get) plus a localStorage store for
   writes. The probe is GET /api/catalog — it exists only in the new
   contract, so the moment the real backend lands the stub steps aside.
   UX.md THE LAW applies throughout: cards render as CARDS.
   ═══════════════════════════════════════════════════════════════════════ */

const DBS = {
  live: null,              // null = not probed; true = real API; false = fixtures
  catalog: null,           // {format, point_limit, identities, cards} (filtered)
  byId: {},                // id → CatalogCard (identities included)
  deck: null,              // {key, name, identity, cards:{id:n}, builtin}
  problems: [],            // last SERVER verdict, verbatim
  dirty: false,
  filters: { q: "", fac: "", type: "" },
  pickSide: "runner",
};
const DEV_DECKS_KEY = "jrs_dev_decks";

/* ── backend probe + fixture fallback ─────────────────────────────────── */
async function dbProbe() {
  if (DBS.live !== null) return DBS.live;
  try {
    const c = await api("/api/catalog?format=eternal");
    if (c && c.format === "eternal" && Array.isArray(c.identities)) {
      DBS.live = true;
      dbSetCatalog(c);
      return true;
    }
  } catch (e) { /* not there yet */ }
  DBS.live = false;
  const c = await api("/dev-fixtures/catalog-eternal.json");
  dbSetCatalog(c);
  return false;
}

/* The API contract excludes banned and draft-only cards; assert that
   defensively anyway — a banned card in the pool is a deck refused later. */
function dbSetCatalog(c) {
  const keep = (x) => !x.banned && !x.draft_only;
  const droppedN = c.identities.length + c.cards.length
    - c.identities.filter(keep).length - c.cards.filter(keep).length;
  if (droppedN) console.warn(`catalog sent ${droppedN} banned/draft-only cards; dropped client-side`);
  DBS.catalog = {
    format: c.format,
    point_limit: c.point_limit != null ? c.point_limit : 7,
    identities: c.identities.filter(keep),
    cards: c.cards.filter(keep),
  };
  DBS.byId = {};
  DBS.catalog.identities.concat(DBS.catalog.cards).forEach((x) => { DBS.byId[x.id] = x; });
}

function devDecks() {
  try { return JSON.parse(localStorage.getItem(DEV_DECKS_KEY)) || {}; }
  catch (e) { return {}; }
}
function devDecksWrite(m) { localStorage.setItem(DEV_DECKS_KEY, JSON.stringify(m)); }

async function dbListDecks() {
  if (await dbProbe()) {
    const r = await api("/api/decks");
    return r.decks || [];
  }
  const fix = await api("/dev-fixtures/decks.json");
  const mine = Object.values(devDecks()).map((d) => ({
    key: d.key, name: d.name, builtin: false, legal: d.legal,
    identity: d.identity, side: (DBS.byId[d.identity] || {}).side,
  }));
  return (fix.decks || []).concat(mine);
}

async function dbGetDeck(key) {
  if (await dbProbe()) return api(`/api/decks/${encodeURIComponent(key)}`);
  const mine = devDecks();
  if (mine[key]) return mine[key];
  return api(`/dev-fixtures/decks/${encodeURIComponent(key)}.json`);
}

async function dbSaveDeck(deck) {
  const body = { name: deck.name, identity: deck.identity, cards: deck.cards };
  if (await dbProbe()) {
    return deck.key
      ? api(`/api/decks/${encodeURIComponent(deck.key)}`, { method: "PUT", body })
      : api("/api/decks", { method: "POST", body });
  }
  // Dev stub: same shape back. The "server" verdict here is the mirror below,
  // which is exactly what makes the problems-rendering path testable offline.
  const key = deck.key ||
    (deck.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "deck")
    + "-" + Math.random().toString(36).slice(2, 7);
  const problems = deckMirrorProblems(deck.identity, deck.cards);
  const stored = { key, name: deck.name, builtin: false, identity: deck.identity,
    cards: Object.assign({}, deck.cards), legal: !problems.length, problems };
  const mine = devDecks();
  mine[key] = stored;
  devDecksWrite(mine);
  return { key, legal: stored.legal, problems };
}

async function dbDeleteDeck(key, builtin) {
  if (await dbProbe()) return api(`/api/decks/${encodeURIComponent(key)}`, { method: "DELETE" });
  if (builtin) throw new Error("built-in decks cannot be deleted (403)");
  const mine = devDecks();
  delete mine[key];
  devDecksWrite(mine);
}

/* ── client-side legality mirrors ─────────────────────────────────────────
   For LIVE METERS ONLY — the server's verdict (the `problems` array a save
   returns) is the truth, and is rendered verbatim wherever it lands. These
   formulas exist so the meters can go green/red between keystrokes:

     influence used  = Σ printed influence_cost × copies, over cards whose
                       faction differs from the identity's faction
                       (limit: identity influence_limit; null = unlimited)
     deck size       ≥ identity min_deck_size
     agenda points   (Corp) CR 1.4.6: a deck of size S needs LO..HI where
                       n  = floor((max(S, min_deck_size, 40) − 40) / 5)
                       LO = 18 + 2n,  HI = 19 + 2n
                       (40–44 → 18/19, 45–49 → 20/21, 50–54 → 22/23, then
                        +2 per full 5 cards over 50 — same arithmetic)
     eternal points  ONCE PER CARD NAME, the identity's own listing included
                     — NOT per copy. Three Account Siphons cost the deck the
                     same 3 points one does (eternal.rs:351-353,447, citing
                     the reference validator). The mirror used to multiply by
                     the copy count, which read "points 20/7" in red over a
                     deck the server had just called legal at 7/7 — a client
                     that contradicts the server about legality is worse than
                     a client with no meter at all.  Σ ≤ point_limit (7)
     deck limit      copies of a card ≤ its printed deck_limit             */
function apWindow(size, minSize) {
  const n = Math.max(0, Math.floor((Math.max(size, minSize || 40, 40) - 40) / 5));
  return [18 + 2 * n, 19 + 2 * n];
}
function deckTotals(identityId, cards) {
  const idc = identityId != null ? DBS.byId[identityId] : null;
  // The identity is a NAME on the points list like any other, and it is
  // counted before a single card is looked at.
  const t = { size: 0, inf: 0, ap: 0, pts: idc ? (idc.points || 0) : 0,
    minSize: idc ? (idc.min_deck_size || 0) : 0,
    infLimit: idc ? idc.influence_limit : null,
    side: idc ? idc.side : null, apLo: 0, apHi: 0 };
  for (const [id, n] of Object.entries(cards)) {
    const c = DBS.byId[id];
    if (!c || !n) continue;
    t.size += n;
    if (idc && c.faction !== idc.faction) t.inf += (c.influence_cost || 0) * n;
    t.ap += (c.agenda_points || 0) * n;
    // Once per name: the map has one entry per name, so no × n here.
    t.pts += (c.points || 0);
  }
  const w = apWindow(t.size, t.minSize);
  t.apLo = w[0]; t.apHi = w[1];
  return t;
}
function deckMirrorProblems(identityId, cards) {
  const out = [];
  const idc = identityId != null ? DBS.byId[identityId] : null;
  if (!idc) {
    out.push({ code: "identity", message: "the deck has no identity", card: null });
    return out;
  }
  const t = deckTotals(identityId, cards);
  if (t.size < t.minSize) out.push({ code: "deck_size",
    message: `deck has ${t.size} cards; the identity requires at least ${t.minSize}`, card: null });
  if (t.infLimit != null && t.inf > t.infLimit) out.push({ code: "influence",
    message: `${t.inf} influence used; the identity allows ${t.infLimit}`, card: null });
  if (t.side === "corp" && !(t.ap >= t.apLo && t.ap <= t.apHi)) out.push({ code: "agenda_points",
    message: `${t.ap} agenda points; a ${t.size}-card deck requires ${t.apLo} or ${t.apHi}`, card: null });
  // `points_limit` is the code the server's vocabulary uses (api.rs Problem).
  if (t.pts > DBS.catalog.point_limit) out.push({ code: "points_limit",
    message: `${t.pts} eternal points; the limit is ${DBS.catalog.point_limit}`, card: null });
  for (const [id, n] of Object.entries(cards)) {
    const c = DBS.byId[id];
    if (!c || !n) continue;
    if (c.side !== idc.side) out.push({ code: "side",
      message: `${c.title} is a ${c.side} card in a ${idc.side} deck`, card: id });
    if (c.deck_limit != null && n > c.deck_limit) out.push({ code: "deck_limit",
      message: `${n} copies of ${c.title}; the printed limit is ${c.deck_limit}`, card: id });
  }
  return out;
}

/* ── Find a Game shell ────────────────────────────────────────────────────
   MOUNT HOOK for the lobby screen (built separately): at load time it calls

       window.jrsRegisterLobby(function mount(hostEl) { … });

   and the shell hands it #findgame-mount, hides the quick-start placeholder,
   and the lobby owns that subtree from then on. Idempotent; registration
   before or after the screen is first opened both work. */
let lobbyMountFn = null;
let lobbyMounted = false;
window.jrsRegisterLobby = function (mountFn) {
  lobbyMountFn = mountFn;
  maybeMountLobby();
};
function maybeMountLobby() {
  if (!lobbyMountFn || lobbyMounted) return;
  lobbyMounted = true;
  $("findgame-placeholder").style.display = "none";
  try { lobbyMountFn($("findgame-mount")); }
  catch (e) {
    console.error("lobby mount failed", e);
    lobbyMounted = false;
    $("findgame-placeholder").style.display = "";
  }
}
$("aspect-findgame").onclick = () => { show("screen-findgame"); maybeMountLobby(); };
$("findgame-back").onclick = () => show("screen-home");

/* ── My Decks ─────────────────────────────────────────────────────────── */
$("aspect-decks").onclick = () => openMyDecks();
$("mydecks-back").onclick = () => show("screen-home");
$("mydecks-new").onclick = () => newDeckFlow();

async function openMyDecks() {
  show("screen-mydecks");
  const box = $("mydecks-list");
  box.textContent = "loading…";
  let list;
  try { list = await dbListDecks(); }
  catch (e) { box.textContent = e.message; return; }
  box.textContent = "";
  // Builtins first — the API sends them first; keep it true regardless.
  list.sort((a, b) => (b.builtin === true) - (a.builtin === true));
  if (!list.length) {
    const empty = el("div", "deck-row");
    empty.appendChild(el("div", "t", "No decks yet — New Deck starts one."));
    box.appendChild(empty);
    return;
  }
  list.forEach((d) => box.appendChild(myDeckRow(d)));
}

function myDeckRow(d) {
  const row = el("div", "deck-row");
  const t = el("div", "t");
  const line1 = el("div", "", d.name);
  if (d.builtin) line1.appendChild(el("span", "badge-builtin", "built-in"));
  line1.appendChild(legalBadge(d.legal !== false));
  t.appendChild(line1);
  const idc = d.identity != null ? DBS.byId[d.identity] : null;
  const sideName = (idc && idc.side) || d.side;
  if (idc || sideName) {
    t.appendChild(el("small", "",
      `${sideName === "corp" ? "⬢ Corp" : sideName === "runner" ? "⬡ Runner" : ""}` +
      (idc ? ` · ${idc.title}` : "")));
  }
  row.appendChild(t);
  if (!d.builtin) {
    const del = el("button", "chip danger", "Delete");
    del.onclick = async (e) => {
      e.stopPropagation();
      if (!confirm(`Delete "${d.name}"? This cannot be undone.`)) return;
      try { await dbDeleteDeck(d.key, d.builtin); } catch (err) { toast(err.message); return; }
      openMyDecks();
    };
    row.appendChild(del);
  }
  const open = el("button", "chip go", "Open");
  open.onclick = () => openBuilder(d.key);
  row.appendChild(open);
  return row;
}

/* ── deck builder ─────────────────────────────────────────────────────── */
$("builder-back").onclick = () => {
  if (DBS.dirty && !confirm("Discard unsaved changes?")) return;
  DBS.dirty = false;
  openMyDecks();
};
$("builder-name").oninput = () => {
  if (!DBS.deck) return;
  DBS.deck.name = $("builder-name").value;
  DBS.dirty = true;
};

async function newDeckFlow() {
  try { await dbProbe(); } catch (e) { toast(e.message); return; }
  DBS.deck = { key: null, name: "", identity: null, cards: {}, builtin: false };
  DBS.problems = [];
  DBS.dirty = false;
  DBS.filters = { q: "", fac: "", type: "" };
  openIdPicker(true);
}

async function openBuilder(key) {
  let d;
  try { await dbProbe(); d = await dbGetDeck(key); }
  catch (e) { toast(e.message); return; }
  DBS.deck = { key: d.key, name: d.name || "", identity: d.identity || null,
    cards: Object.assign({}, d.cards || {}), builtin: d.builtin === true };
  DBS.problems = d.problems || [];
  DBS.dirty = false;
  DBS.filters = { q: "", fac: "", type: "" };
  enterBuilder();
}

function enterBuilder() {
  show("screen-builder");
  $("builder-name").value = DBS.deck.name;
  $("builder-search").value = DBS.filters.q;
  // Built-in decks are read-only originals: Save As forks them into yours.
  $("builder-save").disabled = DBS.deck.builtin;
  $("builder-save").title = DBS.deck.builtin ? "Built-in — use Save As to make it yours" : "";
  renderBuilder();
}

function renderBuilder() {
  renderMeters();
  renderIdSlot();
  renderDeckList();
  renderFilters();
  renderPool();
}

/* Meters: ALWAYS visible, computed live from the client mirror; the server
   problems from the last save are rendered verbatim right under them. */
function renderMeters() {
  const box = $("builder-meters");
  box.textContent = "";
  const d = DBS.deck;
  const idc = d.identity != null ? DBS.byId[d.identity] : null;
  const t = deckTotals(d.identity, d.cards);
  const meter = (label, ok) => el("span", "meter" + (ok === null ? " na" : ok ? "" : " bad"), label);
  if (!idc) {
    box.appendChild(meter("no identity", null));
  } else {
    box.appendChild(meter(`${t.size}/${t.minSize}+ cards`, t.size >= t.minSize));
    if (idc.side === "corp") {
      box.appendChild(meter(`AP ${t.ap} (need ${t.apLo}–${t.apHi})`, t.ap >= t.apLo && t.ap <= t.apHi));
    }
    box.appendChild(meter(
      `influence ${t.inf}/${t.infLimit == null ? "∞" : t.infLimit}`,
      t.infLimit == null || t.inf <= t.infLimit));
    box.appendChild(meter(`points ${t.pts}/${DBS.catalog.point_limit}`,
      t.pts <= DBS.catalog.point_limit));
  }
  if (DBS.deck.builtin) box.appendChild(el("span", "badge-builtin", "built-in"));
  // The server's word, verbatim. Problems that name a card also mark its row.
  const general = DBS.problems.filter((p) => !p.card);
  if (general.length) {
    const strip = el("div", "builder-problems");
    general.forEach((p) => strip.appendChild(el("div", "problem", p.message)));
    box.appendChild(strip);
  }
}

function factionClass(f) {
  const k = String(f || "neutral").toLowerCase();
  if (k.includes("crim")) return "fac-criminal";
  if (k.includes("shaper")) return "fac-shaper";
  if (k.includes("anarch")) return "fac-anarch";
  if (k.includes("nbn")) return "fac-nbn";
  if (k.includes("haas") || k === "hb") return "fac-hb";
  if (k.includes("jinteki")) return "fac-jinteki";
  if (k.includes("weyland")) return "fac-weyland";
  return "fac-neutral";
}

/* A catalog card drawn as a CARD (THE LAW §1): the game's .card box with the
   builder's own anatomy — faction stripe, influence pips, eternal points
   badge, copies-in-deck disc. The art comes from our own cache (the route
   resolves the catalog's NSG id, which no CDN would have understood, and
   which is why this grid used to be a wall of blanks); the text scaffold
   shows through underneath for a card we have no art for. */
function builderCardEl(cc, opts) {
  opts = opts || {};
  const d = document.createElement("div");
  const qty = opts.qty || 0;
  // `armKey` is what makes this card a candidate for the editor's focus —
  // stamped on the element so the painter can find it after a re-render
  // replaced every node in the grid. A card without one is a card no click
  // arms (the identity slot: tapping it opens the picker, which is a look
  // and not a commit).
  if (opts.armKey) d.dataset.armkey = opts.armKey;
  d.className = "card " + (cc.side === "corp" ? "corp-card" : "runner-card") +
    (cc.type === "Identity" ? " identity" : "") +
    (cc.points > 0 ? " hasbpts" : "");
  const pips = cc.influence_cost > 0 ? "●".repeat(cc.influence_cost) : "";
  d.innerHTML = `
    <span class="fstripe ${factionClass(cc.faction)}"></span>
    <div class="cname">${esc(cc.title)}</div>
    <div class="ctype">${esc(cc.type)}</div>
    ${cc.agenda_points != null ? `<div class="bap">${cc.agenda_points}</div>` : ""}
    ${cc.points > 0 ? `<div class="bpts">${cc.points} pt${cc.points > 1 ? "s" : ""}</div>` : ""}
    ${pips ? `<div class="binf">${pips}</div>` : ""}
    ${qty ? `<div class="bqty">×${qty}</div>` : ""}`;
  const img = new Image();
  img.onload = () => { d.style.backgroundImage = `url(${cardImgUrl(cc.id)})`; d.classList.add("art"); };
  img.src = cardImgUrl(cc.id);
  builderRead(d, cc);
  return d;
}

/* Long-press (and hover, on a pointer device) reads the card — THE LAW §5.
   The catalog carries construction fields, not rules text; the reader shows
   what it has and the art carries the words. */
function builderRead(elm, cc) {
  const press = pressToRead(elm, 420, () => builderZoom(cc));
  // The grid this lives in is the longest scroller in the app, so a drag that
  // ends on a card must not count as a click on it either: `click` fires after
  // the release wherever the pointer went down and up on the same element, and
  // "add a copy of this card to my deck" is not what the player asked for by
  // flicking the pool. Both questions, one flag, checked by every caller.
  elm.__wasLongPress = () => press.fired() || press.travelled();
}
function builderZoom(cc) {
  const o = $("zoom-overlay");
  if (o.parentElement !== document.body) document.body.appendChild(o); // it lives in screen-game
  o.style.display = "flex";
  const lines = [];
  lines.push(`${esc(cc.type)} · ${esc(cc.faction || "Neutral")}`);
  if (cc.influence_cost != null) lines.push(`Influence ${cc.influence_cost}`);
  if (cc.agenda_points != null) lines.push(`Agenda points ${cc.agenda_points}`);
  if (cc.points > 0) lines.push(`Eternal points ${cc.points}`);
  if (cc.deck_limit != null) lines.push(`Deck limit ${cc.deck_limit}`);
  if (cc.min_deck_size != null) lines.push(`Minimum deck size ${cc.min_deck_size}`);
  if (cc.influence_limit != null) lines.push(`Influence limit ${cc.influence_limit}`);
  o.innerHTML = `<div class="zoom-card">
      <img class="zart" src="${cardImgUrl(cc.id)}" alt="" onerror="this.remove()">
      <h3>${esc(cc.title)}</h3>
      <div class="zline">${lines.join("<br>")}</div>
    </div>
    <div class="tapaway">tap anywhere to close</div>`;
  dismissOnTapAway(o, null);
}

function renderIdSlot() {
  const box = $("builder-idslot");
  box.textContent = "";
  const idc = DBS.deck.identity != null ? DBS.byId[DBS.deck.identity] : null;
  if (idc) {
    const c = builderCardEl(idc);
    c.onclick = () => { if (!c.__wasLongPress()) openIdPicker(false); };
    box.appendChild(c);
    const meta = el("div", "idmeta");
    meta.appendChild(el("div", "", idc.title));
    meta.appendChild(el("small", "",
      `${idc.side === "corp" ? "⬢ Corp" : "⬡ Runner"} · ${idc.faction || ""}` +
      ` · min ${idc.min_deck_size == null ? "—" : idc.min_deck_size}` +
      ` · inf ${idc.influence_limit == null ? "∞" : idc.influence_limit}`));
    const ch = el("button", "chip", "Change identity");
    ch.onclick = () => openIdPicker(false);
    meta.appendChild(ch);
    box.appendChild(meta);
  } else {
    const pick = el("button", "chip go", "Pick an identity");
    pick.onclick = () => openIdPicker(false);
    box.appendChild(pick);
  }
}

const TYPE_ORDER = ["Agenda", "Asset", "Operation", "Upgrade", "Ice",
  "Event", "Hardware", "Program", "Resource"];
function typeRank(t) { const i = TYPE_ORDER.indexOf(t); return i < 0 ? 99 : i; }

function renderDeckList() {
  const box = $("builder-decklist");
  box.textContent = "";
  const d = DBS.deck;
  const idc = d.identity != null ? DBS.byId[d.identity] : null;
  const byCard = DBS.problems.filter((p) => p.card);
  const entries = Object.entries(d.cards).filter(([, n]) => n > 0)
    .map(([id, n]) => ({ cc: DBS.byId[id] || { id, title: id, type: "?", side: "" }, n }))
    .sort((a, b) => typeRank(a.cc.type) - typeRank(b.cc.type) ||
      String(a.cc.title).localeCompare(b.cc.title));
  let lastType = null;
  entries.forEach(({ cc, n }) => {
    if (cc.type !== lastType) {
      lastType = cc.type;
      const count = entries.filter((e) => e.cc.type === cc.type)
        .reduce((s, e) => s + e.n, 0);
      box.appendChild(el("div", "btype-head", `${cc.type} (${count})`));
    }
    const probs = byCard.filter((p) => p.card === cc.id);
    const row = el("div", "brow" + (probs.length ? " problem" : ""));
    const thumb = el("span", "bthumb");
    thumb.style.backgroundImage = `url(${cardImgUrl(cc.id)})`;
    row.appendChild(thumb);
    row.appendChild(el("span", "bq", `${n}×`));
    const bt = el("span", "bt", cc.title);
    row.appendChild(bt);
    // Influence IS per copy (CR 1.4.5a), so the pips multiply. Eternal
    // points are NOT: the badge is the card's own value, whether the deck
    // runs one copy or three, exactly as the total counts it.
    const offFaction = idc && cc.faction !== idc.faction && cc.influence_cost > 0;
    if (offFaction) row.appendChild(el("span", "bpips", "●".repeat(cc.influence_cost * n)));
    if (cc.points > 0) row.appendChild(el("span", "bpips", `${cc.points}pt`));
    const minus = el("button", "chip", "−");
    minus.onclick = () => bumpCard(cc.id, -1);
    const plus = el("button", "chip", "+");
    plus.onclick = () => bumpCard(cc.id, +1);
    row.appendChild(minus);
    row.appendChild(plus);
    builderRead(row, cc);
    if (hoverCapable) {
      row.addEventListener("mouseenter", () => showHoverPreview({ title: cc.title, type: cc.type, code: cc.id }, row));
      row.addEventListener("mouseleave", hideHoverPreview);
    }
    box.appendChild(row);
    // The server's per-card problems, verbatim, next to the card they name.
    probs.forEach((p) => box.appendChild(el("div", "brow-problem-msg", p.message)));
  });
  if (!entries.length) box.appendChild(el("div", "hint",
    "No cards yet — tap a card in the pool to focus it, tap it again to add a copy."));
}

function bumpCard(id, delta) {
  const d = DBS.deck;
  const cc = DBS.byId[id];
  const cur = d.cards[id] || 0;
  let next = cur + delta;
  if (next < 0) next = 0;
  if (delta > 0 && cc && cc.deck_limit != null && next > cc.deck_limit) {
    toast(`Deck limit: ${cc.deck_limit}× ${cc.title}`);
    return;
  }
  if (next === 0) delete d.cards[id];
  else d.cards[id] = next;
  DBS.dirty = true;
  renderMeters();
  renderDeckList();
  renderPool();
}

/* ── the pool: search + faction/type filters over the identity's side ──── */
let builderSearchTimer = null;
$("builder-search").oninput = () => {
  clearTimeout(builderSearchTimer);
  builderSearchTimer = setTimeout(() => {
    DBS.filters.q = $("builder-search").value.trim().toLowerCase();
    renderPool();
  }, 150);
};

function poolCards() {
  const idc = DBS.deck && DBS.deck.identity != null ? DBS.byId[DBS.deck.identity] : null;
  if (!idc) return [];
  const f = DBS.filters;
  return DBS.catalog.cards.filter((c) =>
    c.side === idc.side &&
    (!f.fac || (c.faction || "Neutral") === f.fac) &&
    (!f.type || c.type === f.type) &&
    (!f.q || c.title.toLowerCase().includes(f.q)));
}

function renderFilters() {
  const idc = DBS.deck && DBS.deck.identity != null ? DBS.byId[DBS.deck.identity] : null;
  const pool = idc ? DBS.catalog.cards.filter((c) => c.side === idc.side) : [];
  const facs = [...new Set(pool.map((c) => c.faction || "Neutral"))].sort();
  const types = [...new Set(pool.map((c) => c.type))].sort((a, b) => typeRank(a) - typeRank(b));
  const mk = (host, values, cur, set) => {
    host.textContent = "";
    values.forEach((v) => {
      const b = el("button", "chip" + (cur === v ? " on" : ""), v);
      b.onclick = () => { set(cur === v ? "" : v); renderFilters(); renderPool(); };
      host.appendChild(b);
    });
  };
  mk($("builder-facs"), facs, DBS.filters.fac, (v) => { DBS.filters.fac = v; });
  mk($("builder-types"), types, DBS.filters.type, (v) => { DBS.filters.type = v; });
}

function renderPool() {
  const grid = $("builder-grid");
  grid.textContent = "";
  const cards = poolCards()
    .sort((a, b) => typeRank(a.type) - typeRank(b.type) || a.title.localeCompare(b.title));
  cards.forEach((cc) => {
    const key = "pool:" + cc.id;
    const c = builderCardEl(cc, { qty: DBS.deck.cards[cc.id] || 0, armKey: key });
    // TWO CLICKS, the board's grammar (THE LAW §3, lesson 16). The first
    // click FOCUSES this card — it surfaces over its neighbours and takes
    // the white ring — and only a second click on the same card puts a copy
    // in the deck. A click on a different card moves the focus there and
    // adds nothing. A single-click add was the editor teaching the opposite
    // of what the board then enforces, with the added cost that a mis-hit in
    // a wall of 132px cards was a card silently in your list.
    //
    // The focus SURVIVES the add: a third click is the second copy, which is
    // how decks are actually built, and unlike the board's commits (CR
    // 9.2.7f — chosen resolves to the end) a copy is undone by the row's −.
    c.onclick = () => {
      if (c.__wasLongPress()) return;
      armTap(BUILDER_ARM, key, () => bumpCard(cc.id, +1));
    };
    if (hoverCapable) {
      c.addEventListener("mouseenter", () => showHoverPreview({ title: cc.title, type: cc.type, code: cc.id }, c));
      c.addEventListener("mouseleave", hideHoverPreview);
    }
    grid.appendChild(c);
  });
  if (!cards.length) grid.appendChild(el("div", "hint",
    DBS.deck && DBS.deck.identity != null ? "No cards match." : "Pick an identity first."));
  // The grid was just rebuilt from scratch, so the ring has to be put back on
  // whichever card still holds the focus.
  paintBuilderFocus();
}

/* ── identity picker: side toggle, identities as cards ────────────────── */
let idPickerIsNew = false;
function openIdPicker(isNew) {
  idPickerIsNew = isNew;
  // A focus belongs to the surface you are looking at. Opening the picker
  // covers the pool, so the pool's focused card is not the one the next
  // click means any more.
  clearBuilderFocus();
  const idc = DBS.deck && DBS.deck.identity != null ? DBS.byId[DBS.deck.identity] : null;
  if (idc) DBS.pickSide = idc.side;
  $("idpicker").style.display = "";
  if (isNew) show("screen-builder");
  renderIdPicker();
}
$("idpicker-close").onclick = () => {
  $("idpicker").style.display = "none";
  clearBuilderFocus();
  if (idPickerIsNew && DBS.deck && DBS.deck.identity == null) openMyDecks();
  else renderBuilder();
};
// Switching sides replaces every card in the picker, so whatever was focused
// is not on screen any more.
$("idpick-corp").onclick = () => { DBS.pickSide = "corp"; clearBuilderFocus(); renderIdPicker(); };
$("idpick-runner").onclick = () => { DBS.pickSide = "runner"; clearBuilderFocus(); renderIdPicker(); };

function renderIdPicker() {
  document.querySelectorAll("#idpicker [data-pickside]").forEach((b) =>
    b.classList.toggle("on", b.dataset.pickside === DBS.pickSide));
  const grid = $("idpicker-grid");
  grid.textContent = "";
  // draft_only identities never appear — the API already excludes them and
  // dbSetCatalog filtered defensively; this is the last line.
  DBS.catalog.identities
    .filter((c) => c.side === DBS.pickSide && !c.draft_only && !c.banned)
    .forEach((cc) => {
      const key = "idp:" + cc.id;
      const slot = el("div", "");
      const c = builderCardEl(cc, { armKey: key });
      // The picker takes the same two clicks as the pool, and for a stronger
      // reason: choosing an identity is the one commit in this screen that
      // can EMPTY THE DECK (switching sides), and it used to happen on the
      // first tap of a 140px card in a scrolling wall of them.
      c.onclick = () => {
        if (c.__wasLongPress()) return;
        armTap(BUILDER_ARM, key, () => pickIdentity(cc));
      };
      slot.appendChild(c);
      slot.appendChild(el("div", "idlabel", cc.title));
      grid.appendChild(slot);
    });
  paintBuilderFocus();
}

function pickIdentity(cc) {
  const d = DBS.deck;
  const prev = d.identity != null ? DBS.byId[d.identity] : null;
  if (prev && prev.side !== cc.side && Object.keys(d.cards).length) {
    if (!confirm("Switching sides empties the deck. Continue?")) return;
    d.cards = {};
  }
  d.identity = cc.id;
  if (!d.name) d.name = `${cc.title.split(":")[0]} deck`;
  DBS.dirty = true;
  $("idpicker").style.display = "none";
  enterBuilder();
}

/* ── save / save as ───────────────────────────────────────────────────────
   Illegal decks save fine (the API allows it) — they come back badged, and
   the problems land verbatim next to their cards and meters. */
async function builderSave(asNew) {
  const d = DBS.deck;
  if (!d) return;
  if (d.identity == null) { toast("Pick an identity first"); return; }
  d.name = $("builder-name").value.trim() || "unnamed deck";
  let payload = d;
  if (asNew) {
    const name = prompt("Save as:", d.name + (d.builtin ? "" : " copy"));
    if (name == null) return;
    payload = { key: null, name: name.trim() || "unnamed deck",
      identity: d.identity, cards: d.cards, builtin: false };
  }
  let res;
  try { res = await dbSaveDeck(payload); }
  catch (e) { toast(e.message); return; }
  DBS.deck = { key: res.key, name: payload.name, identity: payload.identity,
    cards: Object.assign({}, payload.cards), builtin: false };
  DBS.problems = res.problems || [];
  DBS.dirty = false;
  toast(res.legal ? "Saved — legal" : "Saved — the deck is not legal yet");
  enterBuilder();
}
$("builder-save").onclick = () => builderSave(false);
$("builder-saveas").onclick = () => builderSave(true);
