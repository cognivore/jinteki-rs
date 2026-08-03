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
      enterGameChrome();
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
  document.querySelectorAll(".seg").forEach((b) => b.classList.toggle("on", b.dataset.side === s));
}
pickSide("runner");

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
  const idt = st.identity || {};
  const name = (idt.title || side).split(":")[0];
  const s = sideStats(st, side);
  const clicks = "●".repeat(Math.max(0, st.click || 0)) || "–";
  const art = idt.code ? ` style="background-image:url(${cardImgUrl(idt.code)})"` : "";
  // MTGA-style corner cluster: tappable identity art + compact stat chips.
  return `
    <span class="idchip" data-side="${side}"><span class="idthumb"${art}></span><span class="who">${name}</span></span>
    <span class="stat cred" title="credits">⬡ ${st.credit ?? 0}</span>
    <span class="stat" title="clicks remaining">${clicks}</span>
    <span class="stat" title="cards in hand">Hand ${st["hand-count"] ?? (st.hand || []).length}</span>
    <span class="stat" title="cards in deck">Deck ${st["deck-count"] ?? 0}</span>
    <span class="stat tappable" data-stat="ap" data-side="${side}"
      title="agenda points — tap to see the agendas">AP ${st["agenda-point"] ?? 0}${s.extra}</span>`;
}

// Tap an identity thumb to read the identity card.
document.addEventListener("click", (e) => {
  const chip = e.target.closest(".idchip");
  if (!chip || !S) return;
  const st = S[chip.dataset.side];
  if (st && st.identity) zoomCard(st.identity);
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
      sliver.addEventListener("pointercancel", () => clearTimeout(t));
      sliver.addEventListener("contextmenu", (e) => e.preventDefault());
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
      row.appendChild(idEl);
    }
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
  el.addEventListener("pointercancel", () => clearTimeout(pressTimer));
  // Suppress the iOS long-press callout / selection so the read gesture is ours.
  el.addEventListener("contextmenu", (e) => e.preventDefault());
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
  if (!p) { sheet.style.display = "none"; hideAccessReader(); return; }
  // A decision ABOUT a card puts the card itself in front of you.
  if (p.card && p.card.title) { sheet.style.display = "none"; renderAccessReader(p); return; }
  hideAccessReader();
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
    </div></div>`;
  o.querySelectorAll("[data-uuid]").forEach((b) => {
    b.onclick = () => { peekingBoard = false; act("choice", { choice: { uuid: b.dataset.uuid } }); };
  });
  document.getElementById("access-peek").onclick = () => { peekingBoard = true; renderAccessReader(p); };
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
  const rows = cards.map((c, i) => `
    <div class="pilerow" data-i="${i}">
      ${c.code ? `<span class="pilethumb" style="background-image:url(${cardImgUrl(c.code)})"></span>` : ""}
      <span class="pilename">${c.title || "🂠 facedown"}</span>
      ${c.agendapoints != null ? `<span class="pilepts">${c.agendapoints} pts</span>` : ""}
    </div>`).join("");
  o.innerHTML = `<div class="zoom-card pile"><h3>${title}</h3>
    ${rows || "<div class='zline'>none yet</div>"}</div>`;
  // Tap a row to read that card; tap anywhere else to close.
  o.onclick = (e) => {
    const row = e.target.closest(".pilerow");
    const c = row ? cards[+row.dataset.i] : null;
    if (c && c.title) zoomCard(c);
    else o.style.display = "none";
  };
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
  o.onclick = () => { o.style.display = "none"; };
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

function attachZoom(elm, c) {
  let t = null;
  elm.addEventListener("pointerdown", () => { t = setTimeout(() => zoomCard({ title: c.title, code: c.code }), 420); });
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
