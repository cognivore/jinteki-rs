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
let prev = { credits: {}, clicks: {}, logn: 0 };
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

/* A player-supplied string on its way into innerHTML — display names are the
   only such strings the board renders (§12.6: everything else uses nodes). */
function esc(s) {
  return String(s == null ? "" : s)
    .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

/* ── screens ─────────────────────────────────────────────────────────── */
function show(id) {
  document.querySelectorAll(".screen").forEach((s) => s.classList.remove("active"));
  $(id).classList.add("active");
}

/* ── networking ──────────────────────────────────────────────────────── */
function connect(path, onopen) {
  seenCids = new Set();
  sectionCache = {};
  const proto = location.protocol === "https:" ? "wss" : "ws";
  ws = new WebSocket(`${proto}://${location.host}${path}`);
  ws.onopen = onopen;
  ws.onmessage = (ev) => handle(JSON.parse(ev.data));
  ws.onclose = () => { showDisconnected(); };
}
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
      $("crlobby-cancel").style.display = "none";
      $("crlobby-mine").textContent = "";
      crPairingClear();
      break;
    case "state":
      S = m.state;
      ACTIONS = m.actions || [];
      if (m.mode === "bridge" && m.side && m.side !== "spect") mySide = m.side;
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

$("btn-cr").onclick = () => {
  if (!CR_READY || !CR_READY.ready) { showCrGap(); return; }
  mode = "cr";
  mySide = crSide;
  connect("/ws/local", () => {
    const seed = parseInt($("cr-seed").value, 10);
    send({
      type: "start",
      engine: "cr",
      side: crSide,
      seed: Number.isFinite(seed) ? seed : undefined,
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
      action_secs: num("crlobby-rope-action", 60),
      decision_secs: num("crlobby-rope-decision", 10),
      timeout_fuse_secs: num("crlobby-rope-fuse", 30),
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
$("crlobby-create-runner").onclick = () => crCreate("runner");
$("crlobby-create-corp").onclick = () => crCreate("corp");
$("crlobby-anyone").onclick = () => {
  $("crlobby-status").textContent = "finding an opponent…";
  send({
    type: "lobby-anyone",
    decks: { runner: crDeck("runner") || null, corp: crDeck("corp") || null },
    // Used only if nobody is waiting and a seat gets opened. NOTE: autopair
    // itself seats you at ROPED tables only (the server's rule).
    timing: crTiming(),
  });
};
$("crlobby-cancel").onclick = () => {
  crWaitToken = null;
  crWaitId = null;
  localStorage.removeItem("jinteki_local");
  send({ type: "lobby-cancel" });
  $("crlobby-mine").textContent = "";
  $("crlobby-cancel").style.display = "none";
  $("crlobby-status").textContent = "open games";
};

function crCreate(side) {
  const seed = parseInt($("cr-seed").value, 10);
  send({
    type: "lobby-create",
    side,
    title: $("crlobby-title").value,
    seed: Number.isFinite(seed) ? seed : undefined,
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
  $("crlobby-cancel").style.display = "";
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
  mode = "local";
  connect("/ws/local", () => {
    const seed = parseInt($("seed").value, 10);
    send({
      type: "start",
      side: mySide,
      seed: Number.isFinite(seed) ? seed : undefined,
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
    b.textContent = `${d.name || "deck"} — ${idt}`;
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
  if (promptChoicesFor(cid).length) return "usable";
  if (actionsFor(cid).length) return "legal";
  return "";
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
  section("servers", () => {
    if (dirty("servers", [(S.corp || {}).servers, S.run, ACTIONS, myPrompt(), S.priority, armed])) renderServers();
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
  const thinking = gone
    ? `<span class="thinking offline">${esc(who)} disconnected — game held</span>`
    : (!S.winner && S["active-player"] === oSide && !myPrompt() ? `<span class="thinking">thinking…</span>` : "");
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
  const chip = ["idchip", hasPriority(side) ? "priority" : "", glow]
    .filter(Boolean).join(" ");
  // MTGA-style corner cluster: tappable identity art + compact stat chips.
  return `
    <span class="${chip}" data-side="${side}"><span class="idthumb"${art}></span><span class="who">${name}</span></span>
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
  const stat = e.target.closest('.stat[data-stat="discard"]');
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
  if (st && st.identity) onCardTap(st.identity, { identity: true }, chip);
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
  const scroll = wrap.scrollLeft;
  wrap.innerHTML = "";
  const corp = S.corp || {};
  const servers = corp.servers || {};
  const runServer = S.run && S.run.server ? String(S.run.server[0]).replace(":", "") : null;
  const runPos = S.run ? S.run.position : null;
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
    const name = document.createElement("div");
    name.className = "sname"; name.textContent = SERVER_NAME(key);
    col.appendChild(name);

    // central box or content
    if (key === "hq" || key === "rd" || key === "archives") {
      const box = document.createElement("div");
      box.className = "central";
      const n = key === "hq" ? (corp["hand-count"] ?? 0) : key === "rd" ? (corp["deck-count"] ?? 0) : (corp.discard || []).length;
      box.innerHTML = `<b>${n}</b><span>${key === "hq" ? "cards" : key === "rd" ? "cards" : "cards"}</span>`;
      box.onclick = () => { if (key === "archives") zoomPile(corp.discard || [], `Archives (${(corp.discard || []).length})`); };
      col.appendChild(box);
      (srv.content || []).forEach((c) => col.appendChild(cardEl(c, { side: "corp" })));
    } else {
      const content = srv.content || [];
      if (content.length === 0) {
        const box = document.createElement("div");
        box.className = "central"; box.innerHTML = `<span>empty</span>`;
        col.appendChild(box);
      }
      content.forEach((c) => col.appendChild(cardEl(c, { side: "corp" })));
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
      let t = null, fired = false;
      // A tap answers where there is something to answer; otherwise it reads.
      const answerable = isSelectCandidate(c.cid) || promptChoicesFor(c.cid).length > 0;
      sliver.addEventListener("pointerdown", () => { fired = false; t = setTimeout(() => { fired = true; if (sliver.isConnected) zoomCard(c); }, 380); });
      sliver.addEventListener("pointerup", () => {
        clearTimeout(t);
        if (fired) return;
        if (answerable) onCardTap(c, { ice: true }, sliver); else zoomCard(c);
      });
      sliver.addEventListener("pointerleave", () => clearTimeout(t));
      sliver.addEventListener("pointercancel", () => clearTimeout(t));
      sliver.addEventListener("contextmenu", (e) => e.preventDefault());
      // THE LAW §5: a chip is still a card — hover reads it on a pointer
      // device, exactly as hovering the card it stands for would.
      if (hoverCapable) {
        sliver.addEventListener("mouseenter", () => showHoverPreview(c, sliver));
        sliver.addEventListener("mouseleave", hideHoverPreview);
      }
      stack.appendChild(sliver);
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
  wrap.scrollLeft = scroll;
  wireServerScroll(wrap);
  updateServerChevrons(wrap);
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
    if (wrap.scrollWidth <= wrap.clientWidth + 1) return;
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
function updateServerChevrons(wrap) {
  const host = wrap.parentElement;
  if (!host) return;
  let L = host.querySelector(".srvchev.left"), R = host.querySelector(".srvchev.right");
  if (!L) {
    const mk = (cls, glyph, dir) => {
      const b = el("button", "srvchev " + cls, glyph);
      b.onclick = () => {
        // "auto", not "smooth": embedded/zoomed webviews were seen dropping
        // smooth programmatic scrolls outright (the row simply did not
        // move), and an instant jump that always happens beats an animation
        // that sometimes does not. The snap rule still settles the landing.
        wrap.scrollBy({ left: dir * Math.max(120, wrap.clientWidth - 80), behavior: "auto" });
        // Refresh after the proximity snap settles — see the wheel
        // handler's note on dropped scroll events.
        setTimeout(() => updateServerChevrons(wrap), 250);
      };
      host.appendChild(b);
      return b;
    };
    L = mk("left", "‹", -1);
    R = mk("right", "›", 1);
  }
  const max = wrap.scrollWidth - wrap.clientWidth;
  L.style.display = wrap.scrollLeft > 4 ? "" : "none";
  R.style.display = wrap.scrollLeft < max - 4 ? "" : "none";
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
    .map((s) => `<div class="fsub ${s.broken ? "fbroken" : ""}">↳ ${sym(s.label)}</div>`)
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

function renderRig() {
  const rigEl = $("rig");
  rigEl.innerHTML = "";
  const runner = S.runner || {};
  const rig = runner.rig || {};
  [["program", "PRG"], ["hardware", "HW"], ["resource", "RES"]].forEach(([k, label]) => {
    const row = document.createElement("div");
    row.className = "rig-row";
    row.innerHTML = `<span class="rowlabel">${label}</span>`;
    (rig[k] || []).forEach((c) => row.appendChild(cardEl(c, { side: "runner" })));
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
  const facedown = c.facedown && !c.title;
  const isNew = !seenCids.has(c.cid);
  if (isNew) seenCids.add(c.cid);
  const showCost = opts.hand && c.cost != null;
  const showStr = c.strength != null && !facedown;
  el.className = "card" + (isNew ? " deal" : "") + (opts.ice ? " ice" : "") + (facedown ? " facedown" : "") +
    (opts.side === "corp" ? " corp-card" : " runner-card") +
    (opts.identity ? " identity" : "") +
    (c.rezzed === false && opts.side === "corp" && !opts.hand && !facedown ? " unrezzed" : "") +
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
    <div class="cname">${facedown ? "" : (c.title || "")}</div>
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
    if (PRESS.cid !== c.cid) return;
    clearTimeout(PRESS.timer); PRESS.timer = null; PRESS.cid = null;
  };
  el.addEventListener("pointerdown", (e) => {
    clearTimeout(PRESS.timer);
    PRESS.cid = c.cid; PRESS.long = false;
    PRESS.x = e.clientX; PRESS.y = e.clientY;
    // `isConnected`: a re-render that replaces this card mid-press strands
    // the timer (the replacement never hears the pointerup), and a stranded
    // timer opening a reader nobody asked for is the double-spawn race.
    PRESS.timer = setTimeout(() => {
      PRESS.long = true;
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
    if (armed !== c.cid) { setArmed(c.cid); return; }
    onCardTap(c, opts, el);
  });
  el.addEventListener("pointerleave", el.__cancelPress);
  el.addEventListener("pointercancel", el.__cancelPress);
  // Suppress the iOS long-press callout / selection so the read gesture is ours.
  el.addEventListener("contextmenu", (e) => e.preventDefault());
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

function cardImgUrl(code) {
  return `https://card-images.netrunnerdb.com/v2/large/${code}.jpg`;
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
  if (c.implementation) lines.push("⚠ " + c.implementation);
  // The art is the front face's; a back face is text-rendered, like the
  // card pool itself (UX.md: no art is a rendering, not a gap).
  const art = c.code && !back
    ? `<img class="zart" src="${cardImgUrl(c.code)}" alt="" onerror="this.remove()">`
    : "";
  return `${art}<h3>${faceTitle(c, face)}</h3>
    <div class="zline">${lines.join("<br>")}</div>
    <div class="ztext">${sym((back ? back.text : c.text) || "")}</div>
    ${back ? "" : (c.subroutines || []).map((s) => `<div class="ztext ${s.broken ? "zline" : ""}">↳ ${sym(s.label)}${s.broken ? " (broken)" : ""}</div>`).join("")}`;
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

  // A card the current window offers something on: tapping it takes that
  // option. One option resolves immediately; several open a sheet naming
  // them, so a card with two abilities is still unambiguous.
  const offered = promptChoicesFor(c.cid);
  if (offered.length === 1) {
    act("choice", { choice: { uuid: offered[0].uuid } });
    return;
  }
  if (offered.length > 1) {
    const r = el && el.getBoundingClientRect ? el.getBoundingClientRect() : { left: 40, bottom: 120 };
    openSheet(offered.map((ch) => [
      sym(String(ch.value)),
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
      if (a.command === "play") items.push([sym(a.label || "Play"), () => act("play", { card: { cid: c.cid } })]);
      if (a.command === "runner-install") items.push([sym(a.label || "Install"), () => act("runner-install", { card: { cid: c.cid } })]);
      if (a.command === "corp-install") items.push([
        a.label ? sym(a.label)
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
      const label = a.label ? sym(a.label) :
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
      items.push([sym(ab.label || `Ability ${i}`), () => act("ability", { card: c, ability: i })]);
    });
    (c["runner-abilities"] || []).forEach((ab, i) => {
      items.push([sym(ab.label || `Ability ${i}`), () => act("runner-ability", { card: c, ability: i })]);
    });
  }
  if (!items.length) { zoomCard(c); return; }
  const r = el.getBoundingClientRect();
  openSheet(items, Math.min(r.left, window.innerWidth - 200), Math.min(r.bottom + 6, window.innerHeight - 60 * items.length - 20));
}

function abilityLabel(c, idx) {
  const ab = (c.abilities || [])[idx];
  return ab ? sym(ab.label) : `Ability ${idx}`;
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
  "button, .fan-preview, .hover-preview, #zoom-overlay, #access-overlay, #reveal-overlay";
document.addEventListener("pointerdown", (e) => {
  const t = e.target;
  if (t.closest && t.closest(".card")) return;   // its own handler decides
  if (!(t.closest && t.closest(".action-sheet"))) closeSheet();
  if (t.closest && t.closest(HOLDS_FOCUS)) return;
  disarm();
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
    b.textContent = sym(typeof v === "object" && v ? (v.title || "card") : String(v));
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
  const b = el("button", "chip go", ch ? sym(String(ch.value)) : "OK");
  b.onclick = ch ? () => act("choice", { choice: { uuid: ch.uuid } })
                 : () => act("select-done", {});
  btns.appendChild(b);
}

/* CR 4.6.7: the play area, on its own rail.

   A card BEING PLAYED sits in the play area while it resolves (8.6.7g), and
   an active current stays there until another current replaces it (3.7.1b).
   Both are active and both are open information — and neither was drawn
   anywhere, so a run event mid-resolution had no card on screen to outline,
   and a current did its work invisibly.

   The rail is pinned right and never reflows the board (THE LAW §2). Cards
   are real `cardEl`s, so hover and long-press preview work and the green
   `.usable` outline lands on them like any other card. */
function renderPlayRail() {
  const rail = $("play-rail");
  if (!rail) return;
  const cards = [...((S.corp || {})["play-area"] || []).map((c) => ["corp", c, null]),
                 ...((S.runner || {})["play-area"] || []).map((c) => ["runner", c, null])];
  // THE LAW §3: where the board itself can answer, ask it there — which
  // requires the board to be DRAWING the card the answer lives on. An
  // ability can act from a zone the board draws only as a count: "[click]:
  // Play this operation from Archives" (Petty Cash, CR 9.3.3c) puts a legal
  // action on a card whose only pixels were "Archives 1", so the affordance
  // had nowhere to land and the play existed only for a player who thought
  // to open the pile reader. Any card the engine is offering an action on
  // that is drawn nowhere joins this rail, tagged with the zone it is
  // acting from, wearing the same glow ladder and answering the same tap
  // as every other card. (Prompt choices need no copy here: a choice's card
  // is already drawn in the sheet, THE LAW §1.)
  const drawn = drawnCids();
  const offered = [];
  ACTIONS.forEach((a) => {
    if (a.cid != null && !drawn.has(a.cid) && !offered.includes(a.cid)) offered.push(a.cid);
  });
  offered.forEach((cid) => {
    const found = findUndrawnCard(cid);
    if (found) cards.push(found);
  });
  if (!cards.length) { rail.style.display = "none"; rail.innerHTML = ""; return; }
  rail.style.display = "flex";
  rail.innerHTML = "";
  cards.forEach(([side, c, tag]) => {
    const wrap = el("div", "playslot");
    wrap.appendChild(cardEl(c, { side }));
    const sub = (c.subtypes || []).map(String);
    if (sub.some((x) => x.toLowerCase() === "current")) {
      wrap.appendChild(el("div", "playtag", "current"));
    } else if (tag) {
      wrap.appendChild(el("div", "playtag", tag));
    }
    rail.appendChild(wrap);
  });
}

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
      wrap.appendChild(cardEl(c, { side: mySide }));
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
  // Where the board is ALREADY drawing every one of these cards, drawing them
  // again in a sheet on top of the board is the same question twice — and the
  // second copy covers the very cards it is asking about. A reaction window on
  // Daily Casts, with Daily Casts sitting in the rig two inches away already
  // outlined green, needs no picture of Daily Casts: it needs the sentence,
  // the label, and the shimmer that is already there (THE LAW §3). The server
  // decides, from the same `on_screen` the select path uses.
  const onboard = p["choices-onboard"] === true;
  const withCards = onboard ? [] : choices.filter((ch) => ch.card);
  const { row, hint, btns } = promptSheetFrame(sheet, p);
  renderFan(row, withCards, {
    key: "prompt",
    rail: sheet.querySelector(".fanrail"),
    repaint: () => renderCardPrompt(sheet, p, choices),
    cardOpts: { side: mySide },
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
      label: sym(String(ch.value)),
      // A card the viewer is not entitled to see has nothing on its face, so
      // its caption is the only thing telling two of them apart — it stays.
      extra: ch.card.title ? "" : "blind",
    }),
  });
  // AFTER the fan, not before it: `renderFan` empties its host, so a hint
  // written first was deleted before it was ever seen — which is the whole
  // of what an onboard prompt had to say about where to look.
  if (onboard) {
    hint.appendChild(el("div", "picker-hint onboard",
      "Tap the card outlined in green — or use a label below."));
  } else if (withCards.length) {
    // The same sentence a select prompt gets, because it is the same act:
    // one model for choosing a card out of a pool, everywhere (§7).
    hint.appendChild(el("div", "picker-hint", FAN_PICK_HINT));
  }
  // Everything that did not become a card: the options naming no card at all
  // ("Pass", "No action"), and — when the board is already showing them — the
  // LABELS of the options that do, which say what the ability actually does
  // and which the card face cannot. Same uuid, so both paths are one answer.
  choices.filter((ch) => onboard || !ch.card).forEach((ch) => {
    const b = document.createElement("button");
    b.className = "chip" + (ch.card ? " oncard" : "");
    b.textContent = sym(String(ch.value));
    const commit = () => act("choice", { choice: { uuid: ch.uuid } });
    if (ch.card) wireOptionCard(b, ch, commit);
    else b.onclick = commit;
    btns.appendChild(b);
  });
}

/* An option that lives on a card, offered as a LABEL because the board is
   already drawing the card (THE LAW §3). "Use the second ability" names
   nothing on its own, and the green outline it belongs to may be at the far
   end of the board — so pointing at the option shows the card it belongs to
   in the same right-hand preview the fan uses, and holding it reads the card
   in full, the same 420ms press that reads any card anywhere.

   A chip is a 48px button, not a 16px strip of a fan, so it stays ONE tap:
   the two-tap exists because a fan's resting cards are too small to tap
   safely (THE LAW §8), and it would be pure friction here. The press timer
   cancels the tap, so a read can never also commit. */
function wireOptionCard(b, ch, commit) {
  const preview = () => {
    fanPreviewSet("prompt", ch.card, sym(String(ch.value)));
    paintFanPreview();
  };
  let pressTimer = null, longFired = false;
  b.addEventListener("mouseenter", preview);
  b.addEventListener("pointerdown", () => {
    preview();
    longFired = false;
    pressTimer = setTimeout(() => { longFired = true; zoomCard(ch.card); }, 420);
  });
  ["pointerup", "pointerleave", "pointercancel"].forEach((ev) =>
    b.addEventListener(ev, () => { clearTimeout(pressTimer); pressTimer = null; }));
  // The read cancels the tap, never the other way round: the long-press that
  // committed a choice has been shipped once already.
  b.onclick = () => { if (!longFired) commit(); };
  b.addEventListener("contextmenu", (e) => e.preventDefault());
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
  // Where every candidate is already drawn on the board, the board is where
  // the question is asked (§3) — repeating the cards in a sheet on top of
  // them would be the same question twice, and on a phone it would cover the
  // very cards it is asking about. The sheet keeps the sentence and the
  // buttons that are not cards.
  const cards = p["select-onboard"] ? [] : (p["select-cards"] || []);
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
    cardOpts: { side: mySide },
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
  // bug. (A board-answerable question is not empty: the cards are lit behind
  // this sheet.)
  if (!cards.length && !(p["select-cards"] || []).length) {
    hint.appendChild(el("div", "picker-hint", "No card qualifies — there is nothing to choose."));
  } else if (!cards.length) {
    // The candidates are on the BOARD, where a card is a whole card and not a
    // strip of one: there the tap that reaches it is the tap that takes it,
    // and the two taps are the fan's answer to a fan's problem (THE LAW §8).
    hint.appendChild(el("div", "picker-hint",
      ready ? "The cards in white are the ones that go."
      : "Tap a card outlined in gold on the board."));
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
    const b = el("button", "chip", sym(String(ch.value)));
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
  if (yes) btns.push([binary ? "Yes" : sym(String(yes.value)), "yes", yes.uuid]);
  if (no) btns.push([binary ? "No" : "No action", "no", no.uuid]);
  if (!btns.length) choices.forEach((ch) => btns.push([sym(String(ch.value)) || "OK", "no", ch.uuid]));

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
  if (armed != null) {
    const b = document.createElement("button");
    b.className = "chip cancel-armed";
    b.textContent = "Cancel";
    b.onclick = () => { closeSheet(); disarm(); };
    bar.appendChild(b);
  }
  if (native()) {
    if (has("credit")) mk("Gain 1 ⬡", () => act("credit"));
    if (has("draw")) mk("Draw a card", () => act("draw"));
    if (has("remove-tag")) mk("Remove tag (2⬡)", () => act("remove-tag"));
    if (has("purge")) mk("Purge viruses (●●●)", () => act("purge"));
    if (has("trash-resource")) mk("Trash a resource (2⬡)", () => act("trash-resource"));
    const runs = ACTIONS.filter((a) => a.command === "run");
    if (runs.length) mk("Run ▾", () => {
      openSheet(runs.map((a) => [SERVER_NAME(a.server), () => act("run", { server: a.server })]),
        10, window.innerHeight - 300);
    });
  } else {
    const myTurn = S["active-player"] === mySide;
    if (myTurn && !myPrompt()) {
      mk("+1 ⬡", () => act("credit"));
      mk("Draw", () => act("draw"));
      if (mySide === "runner") mk("Run ▾", () => {
        const servers = Object.keys((S.corp || {}).servers || {});
        openSheet(servers.map((k) => [SERVER_NAME(k), () => act("run", { server: k === "hq" || k === "rd" || k === "archives" ? k.toUpperCase().replace("RD", "R&D") : "Server " + k.replace("remote", "") })]),
          10, window.innerHeight - 300);
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
$("log-tab").onclick = () => $("log-drawer").classList.add("open");
$("log-close").onclick = () => $("log-drawer").classList.remove("open");
$("concede-btn").onclick = () => {
  if (confirm("Concede the game?")) act("concede");
  $("log-drawer").classList.remove("open");
};
$("say-send").onclick = () => { send({ type: "say", msg: $("say-input").value }); $("say-input").value = ""; };

function renderLog() {
  const box = $("log-lines");
  const log = S.log || [];
  box.innerHTML = "";
  log.slice(-200).forEach((l) => {
    const d = document.createElement("div");
    const user = typeof l.user === "object" && l.user ? l.user.username : l.user;
    d.textContent = (user && user !== "__system__" ? user + ": " : "") + sym(l.text || "");
    box.appendChild(d);
  });
  box.scrollTop = box.scrollHeight;
  // Chat exists where there is somebody to say it to.
  const human = mode === "bridge" || (mode === "cr" && S["opponent-bot"] === false);
  $("say-row").style.display = human ? "" : "none";
  if (log.length > prev.logn && $("log-drawer").classList.contains("open")) box.scrollTop = box.scrollHeight;
  prev.logn = log.length;
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
   something inside the overlay (a pile row to read), false to dismiss. */
function dismissOnTapAway(o, hit, onClose) {
  const close = () => {
    o.style.display = "none";
    o.__dismiss = null;
    if (onClose) onClose();
  };
  o.__dismiss = close;
  o.onclick = null;
  o.onpointerdown = (e) => {
    if (hit && hit(e)) return;
    e.preventDefault();
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
  // fastest way to take back a card you did not mean to reach for.
  closeSheet();
  disarm();
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

function zoomPile(cards, title) {
  const o = $("zoom-overlay");
  zoomShowing = null;
  o.style.display = "flex";
  const rows = cards.map((c, i) => `
    <div class="pilerow" data-i="${i}">
      ${c.code ? `<span class="pilethumb" style="background-image:url(${cardImgUrl(c.code)})"></span>` : ""}
      <span class="pilename">${c.title || "🂠 facedown"}</span>
      ${c.agendapoints != null ? `<span class="pilepts">${c.agendapoints} pts</span>` : ""}
    </div>`).join("");
  o.innerHTML = `<div class="zoom-card pile"><h3>${title}</h3>
    ${rows || "<div class='zline'>none yet</div>"}</div>
    <div class="tapaway">tap anywhere to close</div>`;
  // UX.md THE LAW §5: every display mode previews. A pile row is a card in
  // compact clothing, so it gets hover preview on a pointer device and
  // long-press on touch, exactly like the card it stands for.
  o.querySelectorAll(".pilerow").forEach((row) => {
    const c = cards[+row.dataset.i];
    if (!c || !c.title) return;
    attachZoom(row, c);
    if (hoverCapable) {
      row.addEventListener("pointerenter", () => showHoverPreview(c, row));
      row.addEventListener("pointerleave", hideHoverPreview);
    }
  });
  // Tap a row to read that card; tap anywhere else to close.
  dismissOnTapAway(o, (e) => {
    const row = e.target.closest(".pilerow");
    const c = row ? cards[+row.dataset.i] : null;
    if (c && c.title) { zoomCard(c); return true; }
    return false;
  });
}

function renderGameOver() {
  const o = $("gameover-overlay");
  if (!S.winner) { o.style.display = "none"; return; }
  const iWon = S.winner === mySide;
  localStorage.removeItem("jinteki_local");
  o.style.display = "flex";
  o.innerHTML = `<h1>${iWon ? "VICTORY" : "DEFEAT"}</h1>
    <div class="why">${S.winner} wins — ${S.reason || ""}</div>
    <button class="big go" onclick="location.reload()">New game</button>`;
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
  let t = null;
  // The guard is `isConnected`: a press whose element is replaced mid-hold
  // (a tap on a pile row opens the reader ON the down and rebuilds the
  // overlay under the finger) loses its pointerup forever, so the timer
  // outlives its element and fired a SECOND reader over the first — the
  // "spawns, races itself, and pops again" a player reported. An element
  // that has left the DOM has no press to honour.
  elm.addEventListener("pointerdown", () => {
    clearTimeout(t);
    t = setTimeout(() => { if (elm.isConnected) zoomCard(c); }, 420);
  });
  ["pointerup", "pointerleave", "pointercancel"].forEach((ev) => elm.addEventListener(ev, () => clearTimeout(t)));
  elm.addEventListener("contextmenu", (e) => e.preventDefault());
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
     eternal points  = Σ points × copies ≤ point_limit (7)
     deck limit      copies of a card ≤ its printed deck_limit             */
function apWindow(size, minSize) {
  const n = Math.max(0, Math.floor((Math.max(size, minSize || 40, 40) - 40) / 5));
  return [18 + 2 * n, 19 + 2 * n];
}
function deckTotals(identityId, cards) {
  const idc = identityId != null ? DBS.byId[identityId] : null;
  const t = { size: 0, inf: 0, ap: 0, pts: 0,
    minSize: idc ? (idc.min_deck_size || 0) : 0,
    infLimit: idc ? idc.influence_limit : null,
    side: idc ? idc.side : null, apLo: 0, apHi: 0 };
  for (const [id, n] of Object.entries(cards)) {
    const c = DBS.byId[id];
    if (!c || !n) continue;
    t.size += n;
    if (idc && c.faction !== idc.faction) t.inf += (c.influence_cost || 0) * n;
    t.ap += (c.agenda_points || 0) * n;
    t.pts += (c.points || 0) * n;
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
  if (t.pts > DBS.catalog.point_limit) out.push({ code: "points",
    message: `${t.pts} eternal points used; the limit is ${DBS.catalog.point_limit}`, card: null });
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
   badge, copies-in-deck disc. Art if the id resolves on the CDN; the text
   scaffold is the real rendering, exactly like the board. */
function builderCardEl(cc, opts) {
  opts = opts || {};
  const d = document.createElement("div");
  const qty = opts.qty || 0;
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
  let t = null;
  let long = false;
  elm.addEventListener("pointerdown", () => {
    long = false;
    clearTimeout(t);
    t = setTimeout(() => { long = true; if (elm.isConnected) builderZoom(cc); }, 420);
  });
  ["pointerup", "pointerleave", "pointercancel"].forEach((ev) =>
    elm.addEventListener(ev, () => clearTimeout(t)));
  elm.addEventListener("contextmenu", (e) => e.preventDefault());
  elm.__wasLongPress = () => long;
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
    const offFaction = idc && cc.faction !== idc.faction && cc.influence_cost > 0;
    if (offFaction) row.appendChild(el("span", "bpips", "●".repeat(cc.influence_cost * n)));
    if (cc.points > 0) row.appendChild(el("span", "bpips", `${cc.points * n}pt`));
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
  if (!entries.length) box.appendChild(el("div", "hint", "No cards yet — tap cards in the pool to add them."));
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
    const c = builderCardEl(cc, { qty: DBS.deck.cards[cc.id] || 0 });
    c.onclick = () => { if (!c.__wasLongPress()) bumpCard(cc.id, +1); };
    if (hoverCapable) {
      c.addEventListener("mouseenter", () => showHoverPreview({ title: cc.title, type: cc.type, code: cc.id }, c));
      c.addEventListener("mouseleave", hideHoverPreview);
    }
    grid.appendChild(c);
  });
  if (!cards.length) grid.appendChild(el("div", "hint",
    DBS.deck && DBS.deck.identity != null ? "No cards match." : "Pick an identity first."));
}

/* ── identity picker: side toggle, identities as cards ────────────────── */
let idPickerIsNew = false;
function openIdPicker(isNew) {
  idPickerIsNew = isNew;
  const idc = DBS.deck && DBS.deck.identity != null ? DBS.byId[DBS.deck.identity] : null;
  if (idc) DBS.pickSide = idc.side;
  $("idpicker").style.display = "";
  if (isNew) show("screen-builder");
  renderIdPicker();
}
$("idpicker-close").onclick = () => {
  $("idpicker").style.display = "none";
  if (idPickerIsNew && DBS.deck && DBS.deck.identity == null) openMyDecks();
  else renderBuilder();
};
$("idpick-corp").onclick = () => { DBS.pickSide = "corp"; renderIdPicker(); };
$("idpick-runner").onclick = () => { DBS.pickSide = "runner"; renderIdPicker(); };

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
      const slot = el("div", "");
      const c = builderCardEl(cc);
      c.onclick = () => { if (!c.__wasLongPress()) pickIdentity(cc); };
      slot.appendChild(c);
      slot.appendChild(el("div", "idlabel", cc.title));
      grid.appendChild(slot);
    });
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
