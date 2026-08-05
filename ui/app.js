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
function act(command, args) { send({ type: "action", command, args: args || {} }); }

function handle(m) {
  switch (m.type) {
    case "session":
      localStorage.setItem("jinteki_local",
        JSON.stringify({ token: m.token, side: m.side, engine: m.engine || "local" }));
      if (m.engine === "cr") mode = "cr";
      if (m.side) mySide = m.side;
      // A waiting lobby seat that just became a game is a game now.
      crWaitToken = null;
      crWaitId = null;
      $("crlobby-cancel").style.display = "none";
      $("crlobby-mine").textContent = "";
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

/* ── the eternal lobby: the same VM with a person in the other seat ──────
   Creating picks a side and, with it, that side's eternal deck; joining
   takes the seat and the deck still going begging. The completeness gate is
   the SAME gate — the server refuses a create exactly as it refuses a bot
   start, with the same payload, so the honest screen below is one screen. */
let crWaitToken = null;
let crWaitId = null;

$("btn-cr-lobby").onclick = () => {
  if (!CR_READY || !CR_READY.ready) { showCrGap(); return; }
  mode = "cr";
  show("screen-cr-lobby");
  $("crlobby-status").textContent = "connecting…";
  connect("/ws/local", () => send({ type: "lobby-list" }));
};

$("crlobby-back").onclick = () => { if (ws) ws.close(); show("screen-home"); };
$("crlobby-refresh").onclick = () => send({ type: "lobby-list" });
$("crlobby-create-runner").onclick = () => crCreate("runner");
$("crlobby-create-corp").onclick = () => crCreate("corp");
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
  });
}

/* Your own seat, taken, waiting for someone to take the other. The token is
   stored exactly like a game's, so closing the tab loses nothing. */
function crWaiting(m) {
  mode = "cr";
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
  t.appendChild(el("small", "",
    `you are the ${g.side || "?"} — ${m.deck || ""} · waiting for the ${g["open-side"] || "?"}`));
  row.appendChild(t);
  row.appendChild(el("span", "chip", "waiting"));
  box.appendChild(row);
  send({ type: "lobby-list" });
}

function renderCrLobbies(list) {
  const box = $("crlobby-list");
  box.textContent = "";
  // Your own seat is shown above, not offered back to you as a join.
  list = list.filter((g) => g.gameid !== crWaitId);
  if (!crWaitToken) $("crlobby-status").textContent =
    list.length ? `${list.length} open game${list.length === 1 ? "" : "s"}` : "open games";
  list.forEach((g) => {
    const row = el("div", "lobby-row");
    const t = el("div", "t");
    t.appendChild(el("b", "", g.title || "eternal game"));
    const age = Math.max(0, g["age-seconds"] | 0);
    const ago = age < 60 ? `${age}s ago` : age < 3600 ? `${Math.round(age / 60)}m ago`
      : `${Math.round(age / 3600)}h ago`;
    t.appendChild(el("small", "",
      `${g.creator || "?"} as ${g.side || "?"} · free seat: ${g["open-side"] || "?"} (${g["open-deck"] || ""}) · ${ago}`));
    row.appendChild(t);
    const join = el("button", "chip go", "Join");
    join.onclick = () => {
      $("crlobby-status").textContent = "joining…";
      send({ type: "lobby-join", gameid: g.gameid });
    };
    row.appendChild(join);
    box.appendChild(row);
  });
  if (!list.length) {
    box.appendChild(el("div", "lobby-row", "No open games — create one."));
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
    if (dirty("servers", [(S.corp || {}).servers, S.run, ACTIONS, myPrompt(), S.priority])) renderServers();
  });
  section("rig", () => {
    if (dirty("rig", [(S.runner || {}).rig, ACTIONS, myPrompt(), S.priority])) renderRig();
  });
  section("hand", () => {
    if (dirty("hand", [me().hand, raised, ACTIONS, myPrompt()])) renderHand();
  });
  section("play area", renderPlayRail);
  section("the prompt", renderPrompt);
  section("access", renderAccessReveal);
  section("actions", renderChips);
  section("end turn", renderTurnBtn);
  section("run controls", renderRunControls);
  section("log", renderLog);
  section("phase", renderPhasePill);
  section("focus", renderFocus);
  section("game over", renderGameOver);
}

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
  const credEl = bot.querySelector(".cred");
  if (credEl) statBump("mycred", m.credit, credEl);
  const oc = top.querySelector(".cred");
  if (oc) statBump("oppcred", o.credit, oc);
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
      sliver.innerHTML = `<span class="iname">${rezzed ? c.title : "?"}</span>` +
        (rezzed ? `<span class="imeta">${c.strength ?? ""}${subsN ? " · " + "↳".repeat(subsN) : ""}</span>` : "");
      let t = null, fired = false;
      // A tap answers where there is something to answer; otherwise it reads.
      const answerable = isSelectCandidate(c.cid) || promptChoicesFor(c.cid).length > 0;
      sliver.addEventListener("pointerdown", () => { fired = false; t = setTimeout(() => { fired = true; zoomCard(c); }, 380); });
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
 * A bounded window over a list of cards, with peeks, a rail, hover-scroll on
 * a pointer and a momentum swipe on touch. ONE implementation, used by two
 * callers, because the hand and the prompt's card row are the same object
 * seen twice — and the prompt row was the proof: it was still a plain flex
 * strip that scrolled sideways, which is exactly the failure the hand was
 * rebuilt to fix. Rebirth (CR 1.5.4a) offers up to 22 identities; a
 * choose-1-of-22 as a horizontal scroll bar is not a choice, it is a search.
 *
 * A centre-anchored fan grows outward as its list grows, so past a few cards
 * it slides under the action bar and the run controls on every iPhone in
 * landscape. The fix is MTG Arena's: the fan is BOUNDED, not elastic. At most
 * FAN_WINDOW cards are laid out at once, the focused one sits in the middle
 * standing proud of its neighbours, and the cards either side peek in so it
 * is visible that there IS more.
 *
 * THE ARITHMETIC (ui/style.css keeps the constants; this is why they are what
 * they are). The hand is centre-anchored and `.action-bar`/`.run-controls`
 * are bounded to `calc(50vw - 122px)` at `left: 10px`, so the nearest either
 * bar can come to the centre line is `50vw - 112px`: the fan's half-width
 * must stay under 112px AT EVERY VIEWPORT WIDTH — which is why widening the
 * phone never bought the hand a single pixel.
 *
 *     card 44 x 61, step 18 (margin 0 -13px), peek 12, window 9
 *     row = 44 + 8*18 + 2*12 = 212px      half = 106px  <= 112  ✓
 *
 * 212px is EXACTLY what the old five-card fan occupied (60 + 4*28 + 2*20).
 * Nothing got wider; nine cards now live where five did. The clearance is
 * 6px per side on every device, because the budget is width-independent:
 *
 *     iPhone SE      667x375   50vw=333.5  bar edge 221.5  fan 229.5..437.5
 *     iPhone 15      852x393   50vw=426    bar edge 314    fan 322..530
 *     iPhone 15 PM   932x430   50vw=466    bar edge 354    fan 362..570
 *     SE portrait    375x667   50vw=187.5  bar edge  75.5  fan  83.5..291.5
 *
 * THE COST, stated: an unfocused card shows an 18px strip of a 44px card, and
 * an 18px strip is not the 48px tap target the rest of this UI holds to. So
 * tapping a specific unfocused card is NOT the interaction. Moving the focus
 * is — the rail's 44px chevrons, its 40px scrub track, the peeks, hover-scroll
 * on a pointer, swipe on touch — and the focused card, at 57x79 and lifted
 * clear of its neighbours, is the target. Tapping it raises it to 88x122, the
 * same size the raised card has always been. This is Hearthstone's own
 * big-hand behaviour and it is a deliberate deviation, recorded in UX.md.
 */
const FAN_WINDOW = 9;

/* Per-fan state, keyed by caller ("hand", "prompt"). `repaint` is the
 * caller's own redraw: a fan move is a LOCAL change and must not push a
 * state or redraw the board (THE LAW §2). */
const fans = {};
function fanOf(key) {
  if (!fans[key]) fans[key] = { focus: 0, total: 0, repaint: null };
  return fans[key];
}
function fanGoto(key, i) {
  const f = fanOf(key);
  const next = Math.max(0, Math.min(i, Math.max(0, f.total - 1)));
  if (next === f.focus) return;
  f.focus = next;
  fanStopHover();
  // The caller's own housekeeping: the hand drops its raised card, or the
  // focus would be dragged straight back to it by `pin` on the next draw.
  if (f.onMove) f.onMove();
  if (f.repaint) f.repaint();
}
function fanMove(key, d) { fanGoto(key, fanOf(key).focus + d); }

/* A swipe must never also play a card. `cardEl`'s tap checks this, so one
 * guard covers the hand, the prompt row and anything else drawn with a real
 * card — the same class of hazard as a long-press that committed a choice. */
let fanDragging = false;
let fanTapUntil = 0;
// A WINDOW, not a one-shot flag: the pointerup after a swipe may land on a
// card, on a peek, or on nothing at all, and whichever it is must not be
// treated as a tap. A flag consumed by the first of those would let the
// others through.
function fanSuppressesTap() { return performance.now() < fanTapUntil; }

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
  if (!hoverCapable || fanDragging) return;
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

/* ── the swipe: scrub the WINDOW, never slide the hand ───────────────────
 * The first attempt dragged the whole row sideways and sprang it back, which
 * is not a hand — it is a strip of paper being pushed around a table. MTG
 * Arena's hand is a CAROUSEL: the frame stays exactly where it is and the
 * cards flow THROUGH it, one entering as another leaves, with the card you
 * are on always in the middle.
 *
 * So the drag drives the FOCUS, continuously. A thumb travelling one card's
 * step advances the focus by one and the row re-lays out one card over; all
 * that is ever translated is the sub-step remainder, at most half a step, so
 * the motion is smooth and yet the hand never leaves its place. The
 * discontinuity at each half-step is exactly cancelled by the relayout — the
 * two are the same movement counted twice, which is why it reads continuous.
 *
 * Past either end the remaining pull is damped to a third: an edge is felt.
 * A flick carries momentum in proportion to its speed. `prefers-reduced-
 * motion` drops the glide, never the snap — the snap is the correctness. */
const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
function fanGestures(host, key) {
  // Attach ONCE per host. `renderFan` runs on every repaint, and a repaint
  // happens on every step of a drag: re-wiring here would add a listener per
  // card of travel and fire the whole stack of them on the next touch.
  if (host.__fanwired) return;
  host.__fanwired = true;
  const rowOf = () => host.querySelector(".fanrow");

  host.addEventListener("pointermove", (e) => fanHoverEdge(host, key, e));
  host.addEventListener("pointerleave", fanStopHover);

  host.addEventListener("pointerdown", (e) => {
    if (e.button != null && e.button !== 0) return;
    const f = fanOf(key);
    if (f.total <= 1) return;
    fanStopHover();
    const step = f.step || 18;
    const x0 = e.clientX, startFocus = f.focus;
    let last = e.clientX, lastT = performance.now(), vel = 0, moved = false;

    const paint = (dx, animate) => {
      const row = rowOf();
      if (!row) return;
      // Where the thumb says we are, in cards; the integer part is the focus
      // and the remainder is all that ever moves.
      const want = startFocus - dx / step;
      const target = Math.max(0, Math.min(Math.round(want), f.total - 1));
      if (target !== fanOf(key).focus) fanGoto(key, target);
      let slid = dx + (target - startFocus) * step;
      // Only the part of the pull that ran out of hand is damped.
      if (Math.abs(slid) > step / 2) slid = Math.sign(slid) * (step / 2 + (Math.abs(slid) - step / 2) * 0.33);
      const r2 = rowOf();
      if (!r2) return;
      r2.style.transition = animate && !reduceMotion ? "transform .18s cubic-bezier(.22,.7,.3,1)" : "none";
      r2.style.transform = `translateX(${slid}px)`;
    };

    const move = (ev) => {
      const dx = ev.clientX - x0;
      if (!moved && Math.abs(dx) < 8) return;
      if (!moved) { moved = true; fanDragging = true; }
      const now = performance.now();
      const dt = Math.max(1, now - lastT);
      vel = 0.7 * vel + 0.3 * ((ev.clientX - last) / dt);   // px/ms, smoothed
      last = ev.clientX; lastT = now;
      paint(dx, false);
    };
    const up = (ev) => {
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", up);
      document.removeEventListener("pointercancel", up);
      if (!moved) return;
      fanDragging = false;
      fanTapUntil = performance.now() + 350;
      // Momentum: 110ms of the release velocity, so a flick keeps going.
      const dx = (ev.clientX - x0) + (reduceMotion ? 0 : vel * 110);
      const want = startFocus - dx / step;
      fanGoto(key, Math.max(0, Math.min(Math.round(want), fanOf(key).total - 1)));
      const row = rowOf();
      if (row) {
        row.style.transition = reduceMotion ? "none" : "transform .2s cubic-bezier(.22,.7,.3,1)";
        row.style.transform = "translateX(0)";
      }
    };
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", up);
    document.addEventListener("pointercancel", up);
  });
}

/* Draw `cards` into `host` as a bounded fan.
 *   key     — which fan's focus this is
 *   build   — (card, index, focused, offsetFromMiddle) => the slot element
 *   rail    — where the rail goes (an element), or null for none
 *   pin     — a cid the focus must sit on (a raised hand card)
 * The geometry comes from CSS custom properties, so it lives in one place. */
function renderFan(host, cards, opts) {
  const key = opts.key;
  const f = fanOf(key);
  f.total = cards.length;
  f.repaint = opts.repaint || null;
  f.onMove = opts.onMove || null;
  host.classList.add("fan");
  host.innerHTML = "";
  const css = getComputedStyle(host);
  f.step = parseFloat(css.getPropertyValue("--fstep")) || 18;
  f.size = Math.min(opts.size || FAN_WINDOW, Math.max(1, cards.length));
  if (!cards.length) { renderFanRail(opts.rail, key, 0, f.size); return; }

  f.focus = Math.max(0, Math.min(f.focus, cards.length - 1));
  if (opts.pin != null) {
    const pi = cards.findIndex((c) => c.cid === opts.pin);
    if (pi >= 0) f.focus = pi;
  }
  const size = f.size;
  const half = Math.floor(size / 2);
  const start = Math.max(0, Math.min(f.focus - half, cards.length - size));
  const end = Math.min(cards.length, start + size);
  const shown = cards.slice(start, end);
  const mid = (shown.length - 1) / 2;

  // The row is the only thing a drag ever moves, and never by more than half
  // a step; the host stays put, so the board's layout does not shift (§2).
  const row = el("div", "fanrow");
  shown.forEach((c, i) => {
    const idx = start + i;
    const slot = opts.build(c, idx, idx === f.focus, i - mid);
    if (!slot) return;
    // MTGA: the card under the pointer IS the card you mean, and it rises out
    // of the fan to be read where it lies. Suspended while a card is raised
    // (the raise owns the focus until it is put down) and while a drag is in
    // flight (the drag drives the focus itself).
    if (hoverCapable && opts.pin == null) {
      slot.addEventListener("mouseenter", () => { if (!fanDragging) fanGoto(key, idx); });
    }
    row.appendChild(slot);
  });
  if (start > 0) row.prepend(fanPeek(cards[start - 1], "left", opts, key));
  if (end < cards.length) row.append(fanPeek(cards[end], "right", opts, key));
  host.appendChild(row);

  renderFanRail(opts.rail, key, cards.length, size);
  fanGestures(host, key);
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

/* The rail. Deliberately large: it is the one way to reach the rest of a big
 * list, and a hairline scrollbar on a phone is decoration, not a control. */
function renderFanRail(rail, key, total, size) {
  if (!rail) return;
  const f = fanOf(key);
  if (total <= (size || FAN_WINDOW)) { rail.style.display = "none"; return; }
  rail.style.display = "flex";
  rail.innerHTML = "";

  const left = el("button", "railbtn", "‹");
  left.disabled = f.focus <= 0;
  left.onclick = () => fanMove(key, -1);

  const right = el("button", "railbtn", "›");
  right.disabled = f.focus >= total - 1;
  right.onclick = () => fanMove(key, 1);

  // One pip per card, the focused one lit: position in the list is the
  // information, and a count of cards is small enough to show honestly.
  const track = el("div", "railtrack");
  for (let i = 0; i < total; i++) {
    const pip = el("div", "railpip" + (i === f.focus ? " on" : ""));
    pip.onclick = () => fanGoto(key, i);
    track.appendChild(pip);
  }
  // Drag anywhere along the track to scrub, which is faster than pips at 40
  // cards and is what a thumb expects to be able to do.
  const scrub = (e) => {
    const r = track.getBoundingClientRect();
    const t = Math.min(1, Math.max(0, (e.clientX - r.left) / Math.max(1, r.width)));
    fanGoto(key, Math.round(t * (total - 1)));
  };
  track.addEventListener("pointerdown", (e) => { track.setPointerCapture(e.pointerId); scrub(e); });
  track.addEventListener("pointermove", (e) => {
    if (track.hasPointerCapture && track.hasPointerCapture(e.pointerId)) scrub(e);
  });

  const count = el("div", "railcount", `${f.focus + 1}/${total}`);
  rail.append(left, track, count, right);
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

function renderHand() {
  const handEl = $("hand");
  renderFan(handEl, me().hand || [], {
    key: "hand",
    rail: handRailEl(),
    pin: raised,
    repaint: renderHand,
    // Moving the window puts a different card under your thumb, so the one
    // you had raised is no longer the one you are looking at.
    onMove: () => { raised = null; closeSheet(); },
    cardOpts: { side: mySide, hand: true },
    build: (c, idx, focused, off) => {
      const node = cardEl(c, { side: mySide, hand: true, fanKey: "hand", fanIndex: idx });
      if (raised === c.cid) {
        node.classList.add("raised");
      } else {
        // A whisper of rotation, and the focused card lifted AND scaled clear
        // of its neighbours: at an 18px step the resting cards are strips, so
        // the focused one is the card you are actually reading. The scale is
        // inline because the tilt is, and an inline transform is one string.
        // A SHALLOW arc, MTGA's rather than a card-table fan: at nine cards
        // a steep one dips the outer cards straight off the bottom of the
        // screen, and every degree of tilt also widens the row's ink against
        // a band that has none to spare.
        const lift = Math.abs(off) * 1 + (focused ? -8 : 0);
        node.style.transform =
          `rotate(${off * 1.2}deg) translateY(${lift}px)` + (focused ? " scale(1.22)" : "");
        if (focused) node.classList.add("focused");
      }
      return node;
    },
  });
}

function cardEl(c, opts) {
  opts = opts || {};
  const el = document.createElement("div");
  const facedown = c.facedown && !c.title;
  const isNew = !seenCids.has(c.cid);
  if (isNew) seenCids.add(c.cid);
  el.className = "card" + (isNew ? " deal" : "") + (opts.ice ? " ice" : "") + (facedown ? " facedown" : "") +
    (opts.side === "corp" ? " corp-card" : " runner-card") +
    (opts.identity ? " identity" : "") +
    (c.rezzed === false && opts.side === "corp" && !opts.hand && !facedown ? " unrezzed" : "") +
    (opts.current ? " current-ice" : "");
  el.dataset.cid = c.cid;

  const showCost = opts.hand && c.cost != null;
  el.innerHTML = `
    ${showCost ? `<div class="cost">${c.cost}</div>` : ""}
    <div class="cname">${facedown ? "" : (c.title || "")}</div>
    ${opts.ice && c.subroutines ? `<div class="subs">${c.subroutines.map((s) => `<span class="${s.broken ? "broken" : ""}">↳</span>`).join("")}</div>` : ""}
    <div class="ctype">${facedown ? "" : (c.type || "")}</div>
    ${c.strength != null && !facedown ? `<div class="cstr">${c.strength}</div>` : ""}
    <div class="badges">${counterBadges(c)}</div>`;
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
  let pressTimer = null, longFired = false, pressX = 0, pressY = 0;
  el.addEventListener("pointerdown", (e) => {
    longFired = false;
    pressX = e.clientX; pressY = e.clientY;
    pressTimer = setTimeout(() => { longFired = true; zoomCard(c); }, 420);
  });
  // A pointer that has TRAVELLED is a swipe, not a press: cancel the read.
  // Pointer capture during a fan drag lives on the document, so the card may
  // never see `pointerleave` — this is the check that does not depend on it.
  el.addEventListener("pointermove", (e) => {
    if (!pressTimer) return;
    if (Math.abs(e.clientX - pressX) > 8 || Math.abs(e.clientY - pressY) > 8) {
      clearTimeout(pressTimer); pressTimer = null;
    }
  });
  el.addEventListener("pointerup", (e) => {
    clearTimeout(pressTimer); pressTimer = null;
    // …and a swipe that ENDED on this card is still a swipe. Same hazard as
    // the long-press that used to commit a choice: a gesture must resolve to
    // exactly one meaning.
    if (fanSuppressesTap()) return;
    if (longFired) return;
    // TWO TAPS in a fan, MTGA's rule: the first brings the card to focus,
    // the second acts on it. At a 16px step the resting cards are strips, and
    // a strip is far below the 48px a tap target has to be — so a single tap
    // there would be a misplay waiting to happen, on a decision (play a card,
    // discard to hand size) that cannot be taken back. Once it is focused it
    // is 78px wide and lifted clear, and THAT is the thing you tap.
    if (opts.fanKey != null && fanOf(opts.fanKey).focus !== opts.fanIndex) {
      fanGoto(opts.fanKey, opts.fanIndex);
      return;
    }
    onCardTap(c, opts, el);
  });
  el.addEventListener("pointerleave", () => { clearTimeout(pressTimer); pressTimer = null; });
  el.addEventListener("pointercancel", () => { clearTimeout(pressTimer); pressTimer = null; });
  // Suppress the iOS long-press callout / selection so the read gesture is ours.
  el.addEventListener("contextmenu", (e) => e.preventDefault());
  if (hoverCapable) {
    el.addEventListener("mouseenter", () => showHoverPreview(c, el));
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
function counterBadges(c) {
  const out = [];
  if (c["advance-counter"]) {
    out.push(`<div class="badge adv" title="advancement counters">${+c["advance-counter"]}</div>`);
  }
  const k = c.counter || {};
  for (const [key, cls, glyph, hint] of COUNTER_BADGES) {
    if (k[key]) out.push(`<div class="badge ${cls}" title="${hint}">${glyph}${+k[key]}</div>`);
  }
  return out.join("");
}

/* ── card info (shared by hover preview and long-press zoom) ─────────── */
function cardInfoHtml(c) {
  const lines = [];
  if (c.type) lines.push(c.type + (c.subtypes && c.subtypes.length ? " — " + c.subtypes.join(" · ") : ""));
  if (c.cost != null) lines.push("Cost " + c.cost + (c.strength != null ? " · Strength " + c.strength : ""));
  else if (c.strength != null) lines.push("Strength " + c.strength);
  if (c.advancementcost != null) lines.push(`Adv req ${c.advancementcost} · ${c.agendapoints} pts`);
  if (c["trash-cost"] != null) lines.push("Trash cost " + c["trash-cost"]);
  if (c["advance-counter"]) lines.push("Advancements: " + c["advance-counter"]);
  for (const [key, , , hint] of COUNTER_BADGES) {
    if (c.counter && c.counter[key]) lines.push(`${hint}: ${c.counter[key]}`);
  }
  if (c.implementation) lines.push("⚠ " + c.implementation);
  const art = c.code
    ? `<img class="zart" src="${cardImgUrl(c.code)}" alt="" onerror="this.remove()">`
    : "";
  return `${art}<h3>${c.title || "Facedown card"}</h3>
    <div class="zline">${lines.join("<br>")}</div>
    <div class="ztext">${sym(c.text || "")}</div>
    ${(c.subroutines || []).map((s) => `<div class="ztext ${s.broken ? "zline" : ""}">↳ ${sym(s.label)}${s.broken ? " (broken)" : ""}</div>`).join("")}`;
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
    raised = c.cid;
    renderHand();
    openHandSheet(c);
    return;
  }
  openBoardSheet(c, el);
}

function openHandSheet(c) {
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
  if (!items.length) { toast("No legal action for this card"); raised = null; renderHand(); return; }
  openSheet(items, window.innerWidth / 2 - 90, window.innerHeight - 330);
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
document.addEventListener("pointerdown", (e) => {
  if (!e.target.closest(".action-sheet") && !e.target.closest(".card")) { closeSheet(); if (raised) { raised = null; renderHand(); } }
});

/* ── prompts ─────────────────────────────────────────────────────────── */
let promptFanKey = null;
function renderPrompt() {
  const sheet = $("prompt-sheet");
  const p = myPrompt();
  if (!p) { sheet.style.display = "none"; hideAccessReader(); return; }
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
  const cards = [...((S.corp || {})["play-area"] || []).map((c) => ["corp", c]),
                 ...((S.runner || {})["play-area"] || []).map((c) => ["runner", c])];
  if (!cards.length) { rail.style.display = "none"; rail.innerHTML = ""; return; }
  rail.style.display = "flex";
  rail.innerHTML = "";
  cards.forEach(([side, c]) => {
    const wrap = el("div", "playslot");
    wrap.appendChild(cardEl(c, { side }));
    const sub = (c.subtypes || []).map(String);
    if (sub.some((x) => x.toLowerCase() === "current")) {
      wrap.appendChild(el("div", "playtag", "current"));
    }
    rail.appendChild(wrap);
  });
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

/* Reorder by dragging. The card follows the pointer; the slot it would take
   is decided by which card centre the pointer has passed. */
function makeDraggable(wrap, cid, repaint) {
  let dragging = false, startX = 0;
  wrap.addEventListener("pointerdown", (e) => {
    startX = e.clientX; dragging = false;
    wrap.setPointerCapture(e.pointerId);
  });
  wrap.addEventListener("pointermove", (e) => {
    if (!wrap.hasPointerCapture || !wrap.hasPointerCapture(e.pointerId)) return;
    // A press that has not moved is a READ, not a drag — long-press preview
    // must still work, so the drag only starts past a real threshold.
    if (!dragging && Math.abs(e.clientX - startX) < 8) return;
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
    wrap.addEventListener(ev, () => { dragging = false; wrap.classList.remove("dragging"); }));
}

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
  const { row, btns } = promptSheetFrame(sheet, p);
  if (onboard) {
    row.appendChild(el("div", "picker-hint onboard", "Tap the card outlined in green — or use a label below."));
  }
  renderFan(row, withCards, {
    key: "prompt",
    rail: sheet.querySelector(".fanrail"),
    repaint: () => renderCardPrompt(sheet, p, choices),
    cardOpts: { side: mySide },
    build: (ch, idx, focused, off) => {
      const wrap = el("div", "cardpick");
      const node = cardEl(ch.card, { side: mySide });
      // A far shallower arc than the hand's: a fan reads as "yours, held",
      // and these are a set being OFFERED. The tilt rides the WRAPPER,
      // leaving the card's own transform free for the hover lift and the
      // picked state (an inline transform beats any stylesheet).
      wrap.style.transform = `rotate(${off * 1.6}deg)` + (focused ? " translateY(-8px)" : "");
      if (focused) wrap.classList.add("focused");
      // THE LAW §3: green = an ability you can use now, gold = a legal target.
      // A select prompt is asking for TARGETS, so its cards are gold, and they
      // are the same gold the board paints on the same cards.
      node.classList.add(isSelectMode() ? "selectable" : "usable");
      wrap.appendChild(node);
      // A card the viewer is not entitled to see has nothing on its face, so
      // its caption is the only thing telling two of them apart — it stays.
      if (!ch.card.title) wrap.classList.add("blind");
      wrap.appendChild(el("div", "cardpick-label", sym(String(ch.value))));
      // The tap is the CARD's own (cardEl wires it): one handler, so a
      // long-press to read can never also commit the choice, and the board and
      // the prompt answer the same way.
      return wrap;
    },
  });
  // Everything that did not become a card: the options naming no card at all
  // ("Pass", "No action"), and — when the board is already showing them — the
  // LABELS of the options that do, which say what the ability actually does
  // and which the card face cannot. Same uuid, so both paths are one answer.
  choices.filter((ch) => onboard || !ch.card).forEach((ch) => {
    const b = document.createElement("button");
    b.className = "chip" + (ch.card ? " oncard" : "");
    b.textContent = sym(String(ch.value));
    b.onclick = () => act("choice", { choice: { uuid: ch.uuid } });
    btns.appendChild(b);
  });
}

/* The sheet's own skeleton: sentence, the fan's host, the fan's rail, the
   chips. One frame for both card-shaped prompts, so the rail can never end
   up in one of them and not the other. */
function promptSheetFrame(sheet, p) {
  sheet.innerHTML = `<div class="pmsg">${sym(p.msg || "")}</div>
    <div class="cardprompt"></div>
    <div class="fanrail" style="display:none"></div>
    <div class="pbtns"></div>`;
  return { row: sheet.querySelector(".cardprompt"), btns: sheet.querySelector(".pbtns") };
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
  // MTG Arena's discard, in two phases: MARK the cards, look at what you
  // marked, and only then throw them away. 5.5.4c cannot be taken back, and
  // it is the one decision a player makes with no clicks left and their mind
  // already on next turn — a single tap that binned a card would be the most
  // expensive misclick in the game. Every other select is an ANNOUNCEMENT
  // (1.15.2) with nothing yet to undo, so those still commit on the last pick.
  const staging = p["select-kind"] === "discard";
  const ready = p["select-confirm"] === true;
  const { row, btns } = promptSheetFrame(sheet, p);
  if (staging) {
    // The server's sentence already carries "(n of m chosen)"; phase two
    // replaces it outright, because the question itself has changed.
    if (ready) {
      sheet.querySelector(".pmsg").textContent =
        `Discard ${picked.size} card${picked.size === 1 ? "" : "s"}? Tap one to put it back.`;
    }
    sheet.classList.toggle("confirming", ready);
  }
  renderFan(row, cards, {
    key: "prompt",
    rail: sheet.querySelector(".fanrail"),
    repaint: () => renderSelectPrompt(sheet, p, choices),
    cardOpts: { side: mySide },
    build: (c, idx, focused, off) => {
      const wrap = el("div", "cardpick");
      const node = cardEl(c, { side: mySide, fanKey: "prompt", fanIndex: idx });
      const on = picked.has(c.cid);
      // One inline transform carries the tilt, the focus lift and the
      // "already chosen" lift: an inline transform beats the stylesheet, so
      // it cannot be two fighting each other.
      wrap.style.transform = `rotate(${off * 1.6}deg) translateY(${(on ? -8 : 0) + (focused ? -8 : 0)}px)`;
      if (focused) wrap.classList.add("focused");
      // The glow is `cardEl`'s, from the one ladder in `glowClass` — gold
      // for a candidate, WHITE for one you have staged — so the sheet's copy
      // and the board's copy of the same card can never disagree.
      if (on) wrap.classList.add("on");
      wrap.appendChild(node);
      if (!c.title) {
        wrap.classList.add("blind");
        wrap.appendChild(el("div", "cardpick-label", `Unseen card ${idx + 1}`));
      }
      return wrap;
    },
  });
  // §6: an empty answer is stated, never implied — a prompt asking for a card
  // when no card qualifies has to SAY so, or it is indistinguishable from a
  // bug. (A board-answerable question is not empty: the cards are lit behind
  // this sheet.)
  if (!cards.length && !(p["select-cards"] || []).length) {
    row.appendChild(el("div", "picker-hint", "No card qualifies — there is nothing to choose."));
  } else if (!cards.length) {
    row.appendChild(el("div", "picker-hint",
      !staging ? "Tap a card outlined in gold."
      : ready ? "The cards in white are the ones that go."
      : "Tap a card outlined in gold to mark it."));
  }
  if (staging) {
    // Phase two, and only once the set is the size the rule asks for: a
    // confirm button that can commit a half-answer is a trap, not a gate.
    const go = el("button", "chip go confirm", `Discard ${picked.size} card${picked.size === 1 ? "" : "s"}`);
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
  // …and nothing else may sit on top of a decision either. A reveal waits
  // its turn; the seq floor means it is still there when the decision is done.
  if (!rest.length || (p && p["prompt-type"] !== "waiting")) { hide(); return; }

  const top = Math.max(...rest.map((a) => a.seq));
  const many = rest.length > 1;
  const ov = revealOverlayEl();
  ov.style.display = "flex";
  ov.innerHTML = "";
  const card = el("div", "zoom-card" + (many ? " pile" : ""));
  const where = rest[rest.length - 1].from;
  // An eyebrow, not a second title: `cardInfoHtml` prints the card's own
  // name, and printing it twice reads as a bug rather than as emphasis.
  card.appendChild(el("div", "acc-eyebrow", many
    ? `You accessed ${rest.length} cards`
    : where ? `You accessed — from ${where}` : "You accessed"));
  if (many) {
    const row = el("div", "cardprompt");
    rest.forEach((a) => {
      const wrap = el("div", "cardpick");
      wrap.appendChild(cardEl(a.card, { side: "runner" }));
      if (a.from) wrap.appendChild(el("div", "cardpick-label", a.from));
      row.appendChild(wrap);
    });
    card.appendChild(row);
    card.appendChild(el("div", "picker-hint", "Press and hold a card to read it."));
  } else {
    // §12.6: card text is the card layer's, never a user string.
    const body = document.createElement("div");
    body.innerHTML = cardInfoHtml(rest[0].card);
    while (body.firstChild) card.appendChild(body.firstChild);
  }
  const ok = el("button", "chip go", "Got it");
  const done = () => { accessSeen = Math.max(accessSeen, top); ov.style.display = "none"; render(); };
  ok.onclick = done;
  card.appendChild(ok);
  ov.appendChild(card);
  ov.appendChild(el("div", "tapaway", "tap away to close"));
  // Inside the reader the cards are real cards — press and hold still reads
  // one — so only a tap OUTSIDE it dismisses. Nothing here traps anybody:
  // the button, the dim area and Escape all close it, and what was accessed
  // stays in the log either way.
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
});

function zoomCard(c) {
  hideHoverPreview();
  const o = $("zoom-overlay");
  o.style.display = "flex";
  o.innerHTML = `<div class="zoom-card">${cardInfoHtml(c)}</div>
    <div class="tapaway">tap anywhere to close</div>`;
  dismissOnTapAway(o, null);
}

function zoomPile(cards, title) {
  const o = $("zoom-overlay");
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
  elm.addEventListener("pointerdown", () => { t = setTimeout(() => zoomCard(c), 420); });
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
