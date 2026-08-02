/* jinteki-rs mobile client.
   One renderer, two backends: local engine (with legality glow from the
   enumerator) and the reference-server bridge (generic controls; the server
   is the authority). Full redraw per state; CSS does the juice. */

"use strict";

const $ = (id) => document.getElementById(id);

let ws = null;
let mode = null;            // "local" | "bridge"
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
    .replaceAll("[Credits]", "⬡").replaceAll("[Credit]", "⬡").replaceAll("[c]", "⬡")
    .replaceAll("[Click]", "●").replaceAll("[click]", "●")
    .replaceAll("[Subroutine]", "↳").replaceAll("[sub]", "↳")
    .replaceAll("[their]", "their");
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
      localStorage.setItem("jinteki_local", JSON.stringify({ token: m.token, side: m.side }));
      if (m.side) mySide = m.side;
      break;
    case "state":
      S = m.state;
      ACTIONS = m.actions || [];
      if (m.mode === "bridge" && m.side && m.side !== "spect") mySide = m.side;
      show("screen-game");
      render();
      break;
    case "connected":
      $("lobby-status").textContent = "connected — pick or create a game";
      send({ type: "lobbies" });
      break;
    case "lobbies": renderLobbies(m.list || []); break;
    case "lobby": renderLobbyState(m.lobby); break;
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
      if (m.error === "session expired") {
        localStorage.removeItem("jinteki_local");
        show("screen-home");
        toast("Previous game expired — start a new one");
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
  document.querySelectorAll(".seg").forEach((b) => b.classList.toggle("on", b.dataset.side === s));
}
pickSide("runner");

$("btn-local").onclick = () => {
  mode = "local";
  connect("/ws/local", () => {
    const seed = parseInt($("seed").value, 10);
    send({ type: "start", side: mySide, seed: Number.isFinite(seed) ? seed : undefined });
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
    mode = "local";
    if (saved.side) mySide = saved.side;
    connect("/ws/local", () => send({ type: "resume", token: saved.token }));
  }
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
function actionsFor(cid) { return ACTIONS.filter((a) => a.cid === cid); }

function dirty(key, val) {
  const s = JSON.stringify(val);
  if (sectionCache[key] === s) return false;
  sectionCache[key] = s;
  return true;
}

function render() {
  if (!S) return;
  // Seat orientation: YOUR territory renders on YOUR half of the board,
  // adjacent to your bar and hand; the opponent's on theirs.
  $("board").classList.toggle("flipped", mySide === "corp");
  renderBars();
  if (dirty("servers", [(S.corp || {}).servers, S.run, ACTIONS, myPrompt()])) renderServers();
  if (dirty("rig", [(S.runner || {}).rig, ACTIONS, myPrompt()])) renderRig();
  if (dirty("hand", [me().hand, raised, ACTIONS, myPrompt()])) renderHand();
  renderPrompt();
  renderChips();
  renderTurnBtn();
  renderRunControls();
  renderLog();
  renderPhasePill();
  renderFocus();
  renderGameOver();
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
  const thinking = !S.winner && S["active-player"] === oSide && !myPrompt() ? `<span class="thinking">thinking…</span>` : "";
  top.innerHTML = barHtml(o, oSide, true) + thinking;
  bot.innerHTML = barHtml(m, mySide, false);
  const credEl = bot.querySelector(".cred");
  if (credEl) statBump("mycred", m.credit, credEl);
  const oc = top.querySelector(".cred");
  if (oc) statBump("oppcred", o.credit, oc);
}

function barHtml(st, side, isOpp) {
  const id = st.identity ? st.identity.title : side;
  const s = sideStats(st, side);
  const clicks = "●".repeat(Math.max(0, st.click || 0)) || "–";
  return `
    <span class="stat who">${(id || side).split(":")[0]}</span>
    <span class="stat cred" title="credits">⬡ ${st.credit ?? 0}</span>
    <span class="stat" title="clicks remaining">${clicks}</span>
    <span class="stat" title="cards in hand">Hand ${st["hand-count"] ?? (st.hand || []).length}</span>
    <span class="stat" title="cards in deck">Deck ${st["deck-count"] ?? 0}</span>
    <span class="stat" title="agenda points">AP ${st["agenda-point"] ?? 0}${s.extra}</span>`;
}

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
    idcol.className = "server";
    const nm = document.createElement("div");
    nm.className = "sname";
    nm.textContent = "Identity";
    idcol.appendChild(nm);
    idcol.appendChild(cardEl(corp.identity, { side: "corp", identity: true }));
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
      box.onclick = () => { if (key === "archives") zoomPile(corp.discard || [], "Archives"); };
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
      sliver.className = "ice-sliver" + (rezzed ? " rezzed" : "") + (isCurrent ? " current" : "");
      const subsN = (c.subroutines || []).length;
      sliver.innerHTML = `<span class="iname">${rezzed ? c.title : "?"}</span>` +
        (rezzed ? `<span class="imeta">${c.strength ?? ""}${subsN ? " · " + "↳".repeat(subsN) : ""}</span>` : "");
      let t = null, fired = false;
      sliver.addEventListener("pointerdown", () => { fired = false; t = setTimeout(() => { fired = true; zoomCard(c); }, 380); });
      sliver.addEventListener("pointerup", () => { clearTimeout(t); if (!fired) zoomCard(c); });
      sliver.addEventListener("pointerleave", () => clearTimeout(t));
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
    if (k === "program" && runner.identity) row.appendChild(cardEl(runner.identity, { side: "runner", identity: true }));
    rigEl.appendChild(row);
  });
}

function renderHand() {
  const handEl = $("hand");
  handEl.innerHTML = "";
  const cards = me().hand || [];
  const mid = (cards.length - 1) / 2;
  cards.forEach((c, i) => {
    const el = cardEl(c, { side: mySide, hand: true });
    if (raised !== c.cid) {
      const rot = (i - mid) * 6;
      const lift = Math.abs(i - mid) * 5;
      el.style.transform = `rotate(${rot}deg) translateY(${lift}px)`;
    } else {
      el.classList.add("raised");
    }
    handEl.appendChild(el);
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
    <div class="badges">
      ${c["advance-counter"] ? `<div class="badge adv">${c["advance-counter"]}</div>` : ""}
      ${c.counter && c.counter.credit ? `<div class="badge cred">${c.counter.credit}</div>` : ""}
    </div>`;
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
  const acts = actionsFor(c.cid);
  if (isSelectMode()) {
    const selActs = ACTIONS.filter((a) => a.command === "select");
    const eligible = mode === "bridge" || selActs.some((a) => a.cid === c.cid);
    if (eligible) el.classList.add("selectable");
  } else if (acts.length) {
    el.classList.add("legal");
  } else if (mode === "bridge" && !facedown && opts.hand) {
    el.classList.add("legal");
  }

  // tap + long-press (mobile read gesture) + hover preview (desktop)
  let pressTimer = null, longFired = false;
  el.addEventListener("pointerdown", (e) => {
    longFired = false;
    pressTimer = setTimeout(() => { longFired = true; zoomCard(c); }, 420);
  });
  el.addEventListener("pointerup", (e) => {
    clearTimeout(pressTimer);
    if (!longFired) onCardTap(c, opts, el);
  });
  el.addEventListener("pointerleave", () => clearTimeout(pressTimer));
  if (hoverCapable) {
    el.addEventListener("mouseenter", () => showHoverPreview(c));
    el.addEventListener("mouseleave", hideHoverPreview);
  }
  return el;
}

function cardImgUrl(code) {
  return `https://card-images.netrunnerdb.com/v2/large/${code}.jpg`;
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
  if (c.counter && c.counter.credit) lines.push("Credits hosted: " + c.counter.credit);
  if (c.implementation) lines.push("⚠ " + c.implementation);
  const art = c.code
    ? `<img class="zart" src="${cardImgUrl(c.code)}" alt="" onerror="this.remove()">`
    : "";
  return `${art}<h3>${c.title || "Facedown card"}</h3>
    <div class="zline">${lines.join("<br>")}</div>
    <div class="ztext">${sym(c.text || "")}</div>
    ${(c.subroutines || []).map((s) => `<div class="ztext ${s.broken ? "zline" : ""}">↳ ${sym(s.label)}${s.broken ? " (broken)" : ""}</div>`).join("")}`;
}

function showHoverPreview(c) {
  let hp = document.getElementById("hover-preview");
  if (!hp) {
    hp = document.createElement("div");
    hp.id = "hover-preview";
    hp.className = "hover-preview";
    document.body.appendChild(hp);
  }
  hp.innerHTML = `<div class="zoom-card small">${cardInfoHtml(c)}</div>`;
  hp.style.display = "block";
}
function hideHoverPreview() {
  const hp = document.getElementById("hover-preview");
  if (hp) hp.style.display = "none";
}

/* ── interactions ────────────────────────────────────────────────────── */
function onCardTap(c, opts, el) {
  closeSheet();
  if (S.winner) return;

  if (isSelectMode()) {
    if (mode === "local") act("select", { card: { cid: c.cid } });
    else act("select", { card: c });
    return;
  }

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
  if (mode === "local") {
    actionsFor(c.cid).forEach((a) => {
      if (a.command === "play") items.push(["Play", () => act("play", { card: { cid: c.cid } })]);
      if (a.command === "runner-install") items.push(["Install", () => act("runner-install", { card: { cid: c.cid } })]);
      if (a.command === "corp-install") items.push([
        a.server === "New remote" ? "Install → new remote" : `Install → ${SERVER_NAME(a.server)}`,
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
  if (mode === "local") {
    actionsFor(c.cid).forEach((a) => {
      const label =
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
function renderPrompt() {
  const sheet = $("prompt-sheet");
  const p = myPrompt();
  if (!p) { sheet.style.display = "none"; return; }
  sheet.style.display = "flex";
  sheet.classList.toggle("waiting", p["prompt-type"] === "waiting");
  const choices = p.choices || [];
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
  if (mode === "local") {
    if (has("credit")) mk("Gain 1 ⬡", () => act("credit"));
    if (has("draw")) mk("Draw a card", () => act("draw"));
    if (has("remove-tag")) mk("Remove tag (2⬡)", () => act("remove-tag"));
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
  if (mode === "local") {
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
  $("say-row").style.display = mode === "bridge" ? "" : "none";
  if (log.length > prev.logn && $("log-drawer").classList.contains("open")) box.scrollTop = box.scrollHeight;
  prev.logn = log.length;
}

/* ── zoom / gameover / toast ─────────────────────────────────────────── */
function zoomCard(c) {
  hideHoverPreview();
  const o = $("zoom-overlay");
  o.style.display = "flex";
  o.innerHTML = `<div class="zoom-card">${cardInfoHtml(c)}</div>`;
  o.onclick = () => { o.style.display = "none"; };
}

function zoomPile(cards, title) {
  const o = $("zoom-overlay");
  o.style.display = "flex";
  o.innerHTML = `<div class="zoom-card"><h3>${title}</h3>
    ${cards.map((c) => `<div class="ztext">${c.title || "🂠 facedown"}</div>`).join("") || "<div class='zline'>empty</div>"}
  </div>`;
  o.onclick = () => { o.style.display = "none"; };
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

function toast(msg) {
  const t = $("toast");
  t.textContent = msg;
  t.style.display = "";
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.style.display = "none"; }, 2600);
}
