const invoke = (...a) => window.__TAURI__.core.invoke(...a);

const HEX_W = 62, HEX_H = 72;
const OWNER_CLASS = { 0: 'owner-you', 1: 'owner-karg', 2: 'owner-vex', 3: 'owner-mord' };
const OWNER_DOT = { 0: 'var(--you)', 1: 'var(--karg)', 2: 'var(--vex)', 3: 'var(--mord)' };
const OWNER_NAME = { 0: 'You', 1: 'Karg', 2: 'Vex', 3: 'Mord' };

const ICONS = {
  waves: '<svg viewBox="0 0 24 24"><path d="M2 9c2 0 2 3 4 3s2-3 4-3 2 3 4 3 2-3 4-3 2 3 4 3" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><path d="M2 15c2 0 2 3 4 3s2-3 4-3 2 3 4 3 2-3 4-3 2 3 4 3" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" opacity=".55"/></svg>',
  pine: '<svg viewBox="0 0 24 24"><path d="M12 2 7 10h3l-4 6h4v4h4v-4h4l-4-6h3z" fill="currentColor"/></svg>',
  grave: '<svg viewBox="0 0 24 24"><path d="M12 3v7M9 7h6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><path d="M5 20c0-4.5 3-8 7-8s7 3.5 7 8" fill="none" stroke="currentColor" stroke-width="1.6"/></svg>',
  castle: '<svg viewBox="0 0 24 24"><path d="M4 21V10h3V7h2v3h2V5h2v5h2V7h2v3h3v11z" fill="currentColor"/></svg>',
  person: '<svg viewBox="0 0 24 24"><circle cx="12" cy="7" r="3" fill="currentColor"/><path d="M6 21c0-4 3-7 6-7s6 3 6 7" fill="currentColor"/></svg>',
};

const NOTES = [
  "Stack units on one hex to combine ranks.",
  "Cut a foe in two — the poor half starves.",
  "Bright men can still move; dim ones are spent.",
  "Gold houses can buy troops; grey ones are broke.",
  "Trees pay nothing — chop before they spread.",
  "Spearmen beat peasant lines; knights crack castles.",
  "Link your lands — treasuries merge into one.",
  "A baron eats 54 gold a turn. Feed it land first.",
];

let S = null;
let cursor = { x: 0, y: 0 };
let busy = false;
let opts = { foes: 2, diff: 'normal', size: 0 };
let omarchy = null;
let sfxOn = localStorage.getItem('slay-sfx') !== 'off';
let audioCtx = null;

// ---- tiny synth: all SFX generated, zero assets ----
function tone(freq, dur, type = 'square', vol = 0.045, when = 0) {
  if (!sfxOn) return;
  try {
    audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
    const t = audioCtx.currentTime + when;
    const o = audioCtx.createOscillator();
    const g = audioCtx.createGain();
    o.type = type;
    o.frequency.setValueAtTime(freq, t);
    g.gain.setValueAtTime(vol, t);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    o.connect(g).connect(audioCtx.destination);
    o.start(t);
    o.stop(t + dur + 0.02);
  } catch { /* audio unavailable: stay silent */ }
}
function playSfx(name) {
  if (!name || !sfxOn) return;
  const seq = {
    select: [[660, 0.06, 'square', 0.04]],
    move: [[330, 0.07, 'triangle', 0.06]],
    chop: [[180, 0.09, 'square', 0.05]],
    combine: [[440, 0.07, 'square', 0.05], [660, 0.09, 'square', 0.05, 0.07]],
    attack: [[120, 0.16, 'sawtooth', 0.07], [90, 0.2, 'square', 0.05, 0.02]],
    buy: [[880, 0.06, 'square', 0.04], [1320, 0.09, 'square', 0.04, 0.06]],
    castle: [[150, 0.2, 'triangle', 0.08]],
    error: [[110, 0.15, 'square', 0.05]],
    turn: [[500, 0.08, 'sine', 0.04], [400, 0.1, 'sine', 0.04, 0.08]],
    undo: [[300, 0.07, 'sine', 0.04], [380, 0.08, 'sine', 0.04, 0.07]],
    win: [[523, 0.1, 'square', 0.05], [659, 0.1, 'square', 0.05, 0.1], [784, 0.1, 'square', 0.05, 0.2], [1046, 0.22, 'square', 0.05, 0.3]],
    lose: [[400, 0.14, 'sawtooth', 0.05], [300, 0.14, 'sawtooth', 0.05, 0.13], [200, 0.24, 'sawtooth', 0.05, 0.26]],
  }[name];
  if (seq) seq.forEach(([f, d, t, v, w]) => tone(f, d, t, v, w || 0));
}
function toggleSfx() {
  sfxOn = !sfxOn;
  localStorage.setItem('slay-sfx', sfxOn ? 'on' : 'off');
  return sfxOn;
}

const $ = (id) => document.getElementById(id);

// Play one command's worth of combat effects after render rebuilt the board.
// Human actions animate fully; AI leftovers get a red ring so raids read.
function playFx(s) {
  const fx = s.fx || [];
  if (!fx.length || !window.hexEls) return;
  const humans = s.humans || [0];
  const at = ([x, y]) => window.hexEls[x + ',' + y];
  const lungeTo = (from, to) => {
    const a = at(from), b = at(to);
    if (!a || !b) return;
    const ar = a.getBoundingClientRect(), br = b.getBoundingClientRect();
    const dx = (br.left - ar.left), dy = (br.top - ar.top);
    const len = Math.hypot(dx, dy) || 1;
    a.style.setProperty('--lx', (dx / len * 16).toFixed(1) + 'px');
    a.style.setProperty('--ly', (dy / len * 16).toFixed(1) + 'px');
    a.classList.add('lunge');
  };
  for (const e of fx) {
    const mine = e.by === undefined || humans.includes(e.by);
    if (e.t === 'attack') {
      if (mine) {
        lungeTo(e.from, e.to);
        const t = at(e.to);
        if (t) t.classList.add('captured');
      } else {
        aiMark(e.to);
      }
    } else if (e.t === 'capture') {
      if (!mine) aiMark(e.at);
    } else if (e.t === 'combine' || e.t === 'chop' || e.t === 'buy') {
      const t = at(e.at);
      if (t) t.classList.add('pop');
    } else if (e.t === 'starve') {
      const t = at(e.at);
      if (t) t.classList.add('starve');
    }
  }
}

// Enemy-raid marker on the top overlay (never buried, unlike per-hex rings).
function aiMark([x, y]) {
  const layer = document.getElementById('ringlayer');
  if (!layer) return;
  const p = hexPos(x, y);
  const el = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  el.setAttribute('d', hexPoly(p.left, p.top));
  el.setAttribute('class', 'rl-ai');
  layer.appendChild(el);
}

// Center the view on your capital (or first hex) and ping it, so you
// never have to hunt for your start when a turn begins.
function pingCapital() {
  if (!S) return;
  const mine = S.hexes.filter((h) => h.owner === S.current);
  if (!mine.length) return;
  const cap = mine.find((h) => h.capital) || mine[0];
  cursor = { x: cap.x, y: cap.y };
  render(S);
  const el = window.hexEls && window.hexEls[cap.x + ',' + cap.y];
  if (!el) return;
  try {
    el.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'center' });
  } catch { /* small boards: nothing to scroll */ }
  const p = hexPos(cap.x, cap.y);
  const layer = document.getElementById('ringlayer');
  if (!layer) return;
  const ping = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  ping.setAttribute('d', hexPoly(p.left, p.top));
  ping.setAttribute('class', 'rl-ping');
  ping.id = 'ping-ring';
  layer.appendChild(ping);
  setTimeout(() => {
    const r = document.getElementById('ping-ring');
    // only remove the ping if nothing else claimed the hex meanwhile
    if (r && !S.sel) r.remove();
  }, 1700);
}

async function cmd(name, args = {}) {
  if (busy) return null;
  busy = true;
  try {
    const s = await invoke(name, args);
    if (s.sfx) playSfx(s.sfx);
    render(s);
    playFx(s);
    if ((name === 'new_game' || name === 'end_turn') && !s.winner) pingCapital();
    return s;
  } finally {
    busy = false;
  }
}

function hexPos(x, y) {
  return { left: x * HEX_W + (y % 2 === 1 ? HEX_W / 2 : 0), top: y * (HEX_H * 0.75) };
}

const RING_COLOR = { 0: '#ffffff', 1: '#e25f66', 2: '#e0a23f', 3: '#b48ce8' };
// ---- ring overlay: ONE svg above all hexes, so outlines are never buried
// under later-painted neighbours (the old per-hex rings lost their bottom
// edges that way). Perimeter = outer edges only, no interior double walls.
const HEX_PTS = [[31, 0], [62, 18], [62, 54], [31, 72], [0, 54], [0, 18]];
const EDGE_OF = {
  even: { '1,0': 1, '-1,0': 4, '0,-1': 0, '-1,-1': 5, '0,1': 2, '-1,1': 3 },
  odd: { '1,0': 1, '-1,0': 4, '1,-1': 0, '0,-1': 5, '1,1': 2, '0,1': 3 },
};
function hexPt(x0, y0, i, inset = 0.9) {
  const cx = x0 + 31, cy = y0 + 36;
  const [vx, vy] = HEX_PTS[i];
  const ax = x0 + vx, ay = y0 + vy;
  return [cx + (ax - cx) * inset, cy + (ay - cy) * inset];
}
function hexPoly(x0, y0, inset = 0.9) {
  let d = '';
  for (let e = 0; e < 6; e++) {
    const [ax, ay] = hexPt(x0, y0, e, inset);
    d += (e ? 'L' : 'M') + ` ${ax.toFixed(1)} ${ay.toFixed(1)} `;
  }
  return d + 'Z';
}

function render(s) {
  S = s;
  $('chip-round').textContent = s.round;
  $('chip-seed').textContent = s.seed;
  $('chip-arch').textContent = s.arch;
  $('chip-diff').textContent = s.difficulty[0].toUpperCase() + s.difficulty.slice(1);
  $('mapname').textContent = 'The ' + s.arch;
  $('turnhint').textContent = s.current === 0 && ((s.humans || [0]).length <= 1)
    ? 'your move'
    : humanLabel(s.current) + "'s move";
  $('gametitle').textContent = 'Slay ' + s.arch[0].toUpperCase() + s.arch.slice(1);

  // board
  const field = $('hexfield');
  field.innerHTML = '';
  const cols = Math.max(...s.hexes.map((h) => h.x)) + 1;
  const rows = Math.max(...s.hexes.map((h) => h.y)) + 1;
  field.style.width = cols * HEX_W + HEX_W / 2 + 'px';
  field.style.height = (rows - 1) * (HEX_H * 0.75) + HEX_H + 'px';
  const tset = new Set(s.targets.map(([x, y]) => x + ',' + y));
  const outline = new Set((s.outline || []).map(([x, y]) => x + ',' + y));
  const guard = new Set((s.guard || []).map(([x, y]) => x + ',' + y));
  const hexEls = {};
  for (const h of s.hexes) {
    const d = document.createElement('div');
    d.className = 'hex';
    const p = hexPos(h.x, h.y);
    d.style.left = p.left + 'px';
    d.style.top = p.top + 'px';
    let inner = '';
    if (h.water) {
      d.classList.add('t-water');
      inner = ICONS.waves;
    } else if (h.owner === -1) {
      if (h.tree) {
        d.classList.add('t-forest');
        inner = ICONS.pine;
      } else {
        d.classList.add('t-open');
      }
    } else {
      d.classList.add(OWNER_CLASS[h.owner] || 'owner-you');
      if (h.unit) {
        inner = `<span class="bob"><span class="uniticon">${ICONS.person}</span><div class="pips">${'●'.repeat(h.unit)}</div></span>`;
        if (h.acted) d.classList.add('spent');
      } else if (h.castle) {
        inner = `<span class="bob"><span class="uniticon">${ICONS.castle}</span></span>`;
      } else if (h.capital) {
        inner = ICONS.castle;
      } else if (h.tree) {
        inner = ICONS.pine;
        if (h.tree === 'palm') inner = `<span style="color:#4fb3a9">${ICONS.pine}</span>`;
      } else if (h.grave) {
        d.classList.add('t-grave');
        inner = ICONS.grave;
      }
    }
    d.innerHTML = '<div class="hexbg"></div>' + inner;
    if (s.sel && s.sel[0] === h.x && s.sel[1] === h.y) d.classList.add('selected');
    if (tset.has(h.x + ',' + h.y)) {
      d.classList.add('target');
      if (h.owner !== 0 || h.unit || h.castle || h.capital || h.grave) {
        d.classList.add('target-foe');
      } else {
        d.innerHTML += '<div class="move-dot"></div>';
      }
    }
    // nothing picked up: every man you *can* pick hops, Slay-style
    if (!s.sel && h.owner === s.current && h.unit && !h.acted) d.classList.add('pickable');
    if (cursor.x === h.x && cursor.y === h.y) d.classList.add('cursor');
    d.addEventListener('click', () => {
      cursor = { x: h.x, y: h.y };
      cmd('click_hex', { x: h.x, y: h.y });
    });
    field.appendChild(d);
    hexEls[h.x + ',' + h.y] = d;
  }
  window.hexEls = hexEls;

  // ring overlay: ONE svg above all hex bodies, so outlines are never
  // buried under later-painted neighbours. Perimeter = outer edges only.
  {
    const byKey = {};
    for (const h of s.hexes) byKey[h.x + ',' + h.y] = h;
    const at = (x, y) => hexPos(x, y);
    const layer = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    layer.id = 'ringlayer';
    layer.setAttribute('width', parseFloat(field.style.width));
    layer.setAttribute('height', parseFloat(field.style.height));
    const shape = (d, cls, stroke) => {
      if (!d) return;
      const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      p.setAttribute('d', d);
      p.setAttribute('class', cls);
      if (stroke) p.setAttribute('stroke', stroke);
      layer.appendChild(p);
    };
    if (outline.size) {
      const first = byKey[[...outline][0]];
      const pcol = RING_COLOR[(first && first.owner) ?? -1] || '#ffffff';
      // the selected hex already wears gold: keep guard blue off it
      const selKey = s.sel ? s.sel[0] + ',' + s.sel[1] : null;
      let perim = '', guardP = '';
      for (const key of outline) {
        const [x, y] = key.split(',').map(Number);
        const p = at(x, y);
        const offs = (y & 1) ? EDGE_OF.odd : EDGE_OF.even;
        for (const dk in offs) {
          const [dx, dy] = dk.split(',').map(Number);
          if (!outline.has((x + dx) + ',' + (y + dy))) {
            // exact shared border: both neighbours compute byte-identical
            // endpoints, and round caps fuse the joints — no dots needed
            const e = offs[dk];
            const [ax, ay] = hexPt(p.left, p.top, e, 1.0);
            const [bx, by] = hexPt(p.left, p.top, (e + 1) % 6, 1.0);
            perim += `M ${ax.toFixed(1)} ${ay.toFixed(1)} L ${bx.toFixed(1)} ${by.toFixed(1)} `;
          }
        }
        // guard lives clearly INSIDE the white border, never on top of it
        if (guard.has(key) && key !== selKey) guardP += hexPoly(p.left, p.top, 0.74);
      }
      shape(perim, 'rl-perim', pcol);
      if (guardP) shape(guardP, 'rl-guard');
    }
    if (s.sel) {
      const p = at(s.sel[0], s.sel[1]);
      shape(hexPoly(p.left, p.top), 'rl-sel');
    }
    {
      const p = at(cursor.x, cursor.y);
      shape(hexPoly(p.left, p.top), 'rl-cursor');
    }
    let foe = '';
    for (const key of tset) {
      const [x, y] = key.split(',').map(Number);
      const h = byKey[key];
      if (!h) continue;
      const p = at(x, y);
      if (h.owner !== 0 || h.unit || h.castle || h.capital || h.grave) {
        foe += hexPoly(p.left, p.top);
      }
    }
    if (foe) shape(foe, 'rl-foe');
    let caps = '', capdim = '';
    for (const h of s.hexes) {
      if (!h.capital) continue;
      const p = at(h.x, h.y);
      if (h.afford) caps += hexPoly(p.left, p.top);
      else capdim += hexPoly(p.left, p.top);
    }
    if (caps) shape(caps, 'rl-cap');
    if (capdim) shape(capdim, 'rl-capdim');
    field.appendChild(layer);
  }

  // standings (humans tagged P1.. in hotseat games, current seat pulses)
  const order = [...s.players].sort((a, b) => b.hexes - a.hexes);
  const multi = (s.humans || [0]).length > 1;
  $('standings').innerHTML = order.map((p) => {
    const tag = multi && (s.humans || []).includes(p.owner)
      ? ` P${s.humans.indexOf(p.owner) + 1}` : '';
    return `<tr class="${p.owner === s.current ? 'me turn' : ''} ${p.alive ? '' : 'dead'}">` +
    `<td><span class="dot" style="background:${OWNER_DOT[p.owner]}"></span>` +
    `<span class="name">${p.name}${tag}</span></td>` +
    `<td class="num">${p.hexes} hex</td>` +
    `<td class="num">${p.owner === s.current ? s.totals.savings + 'g' : ''}</td></tr>`;
  }).join('');

  // treasury: focused own territory, else totals
  const t = s.focus_terr || s.totals;
  $('treasury-sub').textContent = guard.size
    ? `⌂ guards ${guard.size} hex${guard.size === 1 ? '' : 'es'}`
    : s.focus_terr ? `${s.focus_terr.hexes} hex · ${s.focus_terr.savings}g` : 'all lands';
  const net = t.income - t.wages;
  setStat('st-income', '+' + t.income, t.income > 0);
  setStat('st-wages', '' + t.wages, false);
  setStat('st-net', (net >= 0 ? '+' : '') + net, net > 0, net < 0);

  // recruit panel
  const fh = s.focus ? s.hexes.find((h) => h.x === s.focus[0] && h.y === s.focus[1]) : null;
  const ft = s.focus_terr;
  const free = fh && ft && !fh.unit && !fh.castle && !fh.grave && !fh.water;
  const canP = free && ft.savings >= 10;
  const canC = free && !fh.tree && ft && ft.savings >= 15;
  let note = 'Click one of your open hexes.';
  if (fh && fh.owner !== 0) note = fh.owner === -1 ? 'Wilds must be captured first.' : 'Enemy ground — take it first.';
  else if (fh && ft && !free) note = 'Occupied — pick an open hex.';
  else if (fh && ft && ft.savings < 10) note = `Needs ${10 - ft.savings} more gold.`;
  else if (free) note = 'Yours and open — build here.';
  $('recruit-note').textContent = note;
  $('row-peasant').classList.toggle('off', !canP);
  $('row-castle').classList.toggle('off', !canC);

  // legend
  $('maplegend').innerHTML =
    `<span><span class="swatch" style="background:var(--gold)"></span>Selected unit</span>` +
    order.filter((p) => p.alive).map((p) =>
      `<span><span class="swatch" style="background:${OWNER_DOT[p.owner]}"></span>${p.name}</span>`
    ).join('') +
    `<span><span class="swatch" style="background:var(--rich-ink)"></span>Rich capital</span>`;

  // notes rotate with the round
  const ni = ((s.round - 1) * 2) % NOTES.length;
  $('fieldnotes').innerHTML =
    `<div class="row">${NOTES[ni]}</div><div class="row">${NOTES[(ni + 1) % NOTES.length]}</div>`;

  // log bar + waybar-style statusline
  const txt = s.msg || (s.log.length ? s.log[s.log.length - 1] : 'Conquer the island.');
  $('logtext').textContent = txt;
  const stLine = $('st-line');
  if (stLine) stLine.innerHTML = `◈ <b>${s.totals.hexes}</b> hex · <b>${s.totals.savings}g</b> · rd ${s.round}`;

  // end overlay
  if (s.winner !== null && s.winner !== undefined) {
    const humanWon = (s.humans || [0]).includes(s.winner);
    $('end-title').textContent = humanWon
      ? `${humanLabel(s.winner)} takes the island!`
      : `${OWNER_NAME[s.winner] || 'Foe'} wins — you were slain`;
    const lands = order.map((p) => `${p.name} ${p.hexes}`).join(' · ');
    let best = '';
    if (humanWon) {
      const prev = JSON.parse(localStorage.getItem('slay-best') || 'null');
      if (!prev || s.totals.hexes > prev.hexes) {
        localStorage.setItem('slay-best', JSON.stringify({ hexes: s.totals.hexes, round: s.round }));
        best = ' — new best!';
      }
    }
    $('end-sub').textContent = `${lands} · round ${s.round}${best}`;
    if ($('end-overlay').classList.contains('hidden')) playSfx(humanWon ? 'win' : 'lose');
    $('end-overlay').classList.remove('hidden');
  } else {
    $('end-overlay').classList.add('hidden');
  }

  updateCoach(s);
}

function setStat(id, txt, pos, neg = false) {
  const el = $(id);
  el.textContent = txt;
  el.className = 'n' + (pos ? ' pos' : neg ? ' neg' : '');
}

// ---------- events ----------
document.addEventListener('keydown', async (e) => {
  if (!$('help-overlay').classList.contains('hidden')) {
    $('help-overlay').classList.add('hidden');
    return;
  }
  if (!$('start-overlay').classList.contains('hidden')) return;
  if (!$('menu-overlay').classList.contains('hidden')) return;
  if (!$('mp-overlay').classList.contains('hidden')) return;
  if (!$('creator-overlay').classList.contains('hidden')) return;
  if (e.key === 'ArrowUp') cursor.y = Math.max(0, cursor.y - 1);
  else if (e.key === 'ArrowDown') cursor.y = Math.min(99, cursor.y + 1);
  else if (e.key === 'ArrowLeft') cursor.x = Math.max(0, cursor.x - 1);
  else if (e.key === 'ArrowRight') cursor.x = Math.min(99, cursor.x + 1);
  else if (e.key === ' ') { e.preventDefault(); await cmd('end_turn'); return; }
  else if (e.key === 'Enter') { e.preventDefault(); await cmd('click_hex', cursor); return; }
  else if (e.key === 'x' || e.key === 'X') { await cmd('click_hex', cursor); await cmd('buy', { kind: 'man' }); return; }
  else if (e.key === 'c' || e.key === 'C') { await cmd('click_hex', cursor); await cmd('buy', { kind: 'castle' }); return; }
  else if (e.key === 'Escape') { await cmd('cancel_sel'); return; }
  else if (e.key === 'e' || e.key === 'E') { await cmd('end_turn'); return; }
  else if (e.key === 'm' || e.key === 'M') {
    const on = toggleSfx();
    $('logtext').textContent = on ? 'Sound on.' : 'Sound off.';
    return;
  }
  else if (e.key === 'h' || e.key === 'H' || e.key === '?') { $('help-overlay').classList.remove('hidden'); return; }
  else if (e.key === 'n' || e.key === 'N') { await cmd('rematch', { same: false }); return; }
  else return;
  render(S);
});

function segWire(id, key, obj) {
  const target = obj || opts;
  // NOTE: string-valued segs (diff, tool) must keep the raw string —
  // +'land' is NaN, which silently broke the creator tools once already
  const numeric = new Set(['foes', 'size', 'humans', 'edsize']);
  $(id).querySelectorAll('button').forEach((b) => {
    b.addEventListener('click', () => {
      $(id).querySelectorAll('button').forEach((x) => x.classList.remove('on'));
      b.classList.add('on');
      target[key] = numeric.has(key) ? +b.dataset.v : b.dataset.v;
      if (typeof segChanged === 'function') segChanged(id);
    });
  });
}

let launchCfg = null;
let tutIdx = 0, tutOff = false;
const tutSeen = {};
let mpCfg = { humans: 2, foes: 0, size: 0, diff: 'normal' };

function showOnly(id) {
  ['menu-overlay', 'start-overlay', 'mp-overlay', 'creator-overlay', 'custom-overlay',
   'end-overlay', 'help-overlay'].forEach((x) => $(x).classList.add('hidden'));
  if (id) $(id).classList.remove('hidden');
}

function showMenu() {
  launchCfg = null;
  try {
    const best = JSON.parse(localStorage.getItem('slay-best') || 'null');
    $('menu-best').textContent = best
      ? `best conquest: ${best.hexes} hexes · round ${best.round}`
      : 'no conquests yet. be the first.';
  } catch { /* fresh start */ }
  showOnly('menu-overlay');
}

async function startGame(cfg) {
  launchCfg = cfg;
  tutOff = false;
  for (const k in tutSeen) delete tutSeen[k];
  showOnly(null);
  const s = await cmd('new_game', {
    seed: cfg.seed ?? null,
    enemies: cfg.enemies,
    difficulty: cfg.difficulty,
    size: cfg.size,
    humans: cfg.humans,
    tutorial: !!cfg.tutorial,
    custom: cfg.custom || null,
  });
  if (s) {
    const mine = s.hexes.find((h) => h.owner === s.current);
    if (mine) cursor = { x: mine.x, y: mine.y };
    render(s);
  }
}

function humanLabel(o) {
  const hs = (S && S.humans) || [0];
  if (hs.length <= 1) return o === 0 ? 'you' : OWNER_NAME[o];
  return 'Player ' + (hs.indexOf(o) + 1);
}

// ---- tutorial coach (monotonic: lessons never un-complete) ----
const TUTS = [
  { t: 'Buy a peasant: click your glowing ⌂ house, then X (or tap Recruit → Peasant).',
    done: (s) => s.hexes.some((h) => h.owner === s.current && h.unit) },
  { t: 'Click your peasant to pick him up — he turns gold.',
    done: (s) => tutSeen.sel || !!s.sel },
  { t: 'Order him onto a glowing wild hex to claim it.',
    done: (s) => s.hexes.filter((h) => h.owner === s.current).length >= 2 },
  { t: 'Trees earn nothing. Claim that pine, then order a man onto it to chop.',
    done: (s) => {
      if (!s.tut_pine) return true;
      const h = s.hexes.find((q) => q.x === s.tut_pine[0] && q.y === s.tut_pine[1]);
      return !h || h.tree !== 'pine';
    } },
  { t: 'End the turn with Space — every territory earns, then pays wages (♟2 ♝6 ♞18 ♚54). Starve and everyone dies.',
    done: (s) => s.round >= 2 },
  { t: 'Buy a second peasant and stack him onto the first: hello, Spearman.',
    done: (s) => s.hexes.some((h) => h.owner === s.current && h.unit >= 2) },
];
function updateCoach(s) {
  const bar = $('coach-bar');
  const active = launchCfg && launchCfg.mode === 'tutorial' && !tutOff &&
    (s.winner === null || s.winner === undefined);
  if (!active) { bar.classList.add('hidden'); return; }
  if (s.sel) tutSeen.sel = true;
  while (tutIdx < TUTS.length && TUTS[tutIdx].done(s)) tutIdx++;
  bar.classList.remove('hidden');
  $('btn-coach-play').classList.add('hidden');
  if (tutIdx >= TUTS.length) {
    $('coach-dots').textContent = '●'.repeat(TUTS.length);
    $('coach-text').textContent = 'Graduated! Expand, combine, and cut foes in half.';
    const b = $('btn-coach-play');
    b.textContent = 'Play for real →';
    b.classList.remove('hidden');
  } else {
    $('coach-dots').textContent = '●'.repeat(tutIdx) + '○'.repeat(TUTS.length - tutIdx);
    $('coach-text').textContent = `Lesson ${tutIdx + 1}/${TUTS.length} — ${TUTS[tutIdx].t}`;
  }
}

// ---- level creator ----
const ED = { cols: 12, rows: 9, land: new Set(), starts: new Map(), tool: 'land' };
const edKey = (x, y) => x + ',' + y;
const ED_SIZES = [[12, 9], [15, 11], [18, 13]];
function edReset(c, r) {
  ED.cols = c; ED.rows = r;
  ED.land = new Set(); ED.starts = new Map();
  renderEditor();
}
function renderEditor() {
  const f = $('editfield');
  f.innerHTML = '';
  const W = 40, H = 47;
  f.style.width = ED.cols * W + W / 2 + 'px';
  f.style.height = (ED.rows - 1) * (H * 0.75) + H + 'px';
    for (let y = 0; y < ED.rows; y++) {
    for (let x = 0; x < ED.cols; x++) {
      const d = document.createElement('div');
      d.className = 'hex';
      d.dataset.x = x;
      d.dataset.y = y;
      d.style.left = (x * W + (y % 2 === 1 ? W / 2 : 0)) + 'px';
      d.style.top = (y * (H * 0.75)) + 'px';
      const k = edKey(x, y);
      let eInner = '';
      if (!ED.land.has(k)) {
        d.classList.add('t-water');
        eInner = ICONS.waves;
      } else if (ED.starts.has(k)) {
        const slot = ED.starts.get(k);
        d.classList.add(['owner-you', 'owner-karg', 'owner-vex', 'owner-mord'][slot]);
        eInner = `<span style="font-weight:700;color:#fff;position:relative;z-index:1">P${slot + 1}</span>`;
      } else {
        d.classList.add('t-open');
      }
      d.innerHTML = '<div class="hexbg"></div>' + eInner;
      d.addEventListener('pointerdown', (e) => {
        e.preventDefault();
        ED.painting = true;
        edPaint(x, y);
      });
      d.addEventListener('pointerenter', (e) => {
        if (ED.painting && (e.buttons & 1)) edPaint(x, y);
      });
      f.appendChild(d);
    }
  }
  window.addEventListener('pointerup', () => { ED.painting = false; }, { once: false });
  const slots = new Set(ED.starts.values());
  const okLand = ED.land.size >= 12, okStarts = slots.size >= 2;
  $('edit-status').textContent =
    `${ED.land.size} land · ${slots.size} starts — ` +
    (!okLand ? 'need 12+ land. ' : '') +
    (!okStarts ? 'need 2+ starts (Start tool).' : 'ready ✓');
}
function edPaint(x, y) {
  const k = edKey(x, y);
  if (ED.tool === 'land') {
    ED.land.add(k);
  } else if (ED.tool === 'water') {
    ED.land.delete(k);
    ED.starts.delete(k);
  } else {
    if (!ED.land.has(k)) {
      ED.land.add(k);
      ED.starts.set(k, 0);
    } else {
      const cur = ED.starts.get(k);
      if (cur === undefined) ED.starts.set(k, 0);
      else if (cur >= 3) ED.starts.delete(k);
      else ED.starts.set(k, cur + 1);
    }
  }
  renderEditor();
}
function getMaps() {
  try { return JSON.parse(localStorage.getItem('slay-maps') || '[]'); }
  catch { return []; }
}
function renderMapList() {
  const maps = getMaps();
  $('map-list').innerHTML = maps.length ? '' : '<p class="tiny">No painted islands yet.</p>';
  maps.forEach((m, i) => {
    const row = document.createElement('div');
    row.className = 'maprow';
    row.innerHTML = `<span class="nm">${m.name} · ${m.cols}×${m.rows}</span>`;
    const play = document.createElement('button');
    play.className = 'play';
    play.textContent = 'Play';
    play.addEventListener('click', () => playCustom(m));
    const del = document.createElement('button');
    del.textContent = 'Delete';
    del.addEventListener('click', () => {
      const all = getMaps();
      all.splice(i, 1);
      localStorage.setItem('slay-maps', JSON.stringify(all));
      renderMapList();
    });
    row.appendChild(play);
    row.appendChild(del);
    $('map-list').appendChild(row);
  });
}
function creatorView(which) {
  const paint = which !== 'lib';
  $('creator-paint-view').classList.toggle('hidden', !paint);
  $('creator-lib-view').classList.toggle('hidden', paint);
  if (paint) renderEditor();
  else renderMapList();
}

// custom island picked from the library for an upcoming game
let pickerFor = 'single';
const pendingCustom = { single: null, mp: null };
function customPayload(m) {
  const slots = [...new Set(m.starts.map((s) => s.slot))].sort();
  return {
    cols: m.cols, rows: m.rows,
    land: m.land,
    starts: slots.map((sl) => m.starts.find((s) => s.slot === sl).at),
  };
}
function playCustom(m) {
  const slots = [...new Set(m.starts.map((s) => s.slot))].sort();
  startGame({
    mode: 'custom', seed: null, enemies: slots.length - 1, difficulty: 'normal',
    size: 0, humans: [0], tutorial: false,
    custom: customPayload(m),
  });
}
function openPicker(which) {
  pickerFor = which;
  const maps = getMaps();
  $('custom-list').innerHTML = maps.length ? '' : '<p class="tiny">No painted islands yet — make one in the level creator.</p>';
  maps.forEach((m) => {
    const slots = [...new Set(m.starts.map((s) => s.slot))].sort();
    const row = document.createElement('div');
    row.className = 'maprow';
    row.innerHTML = `<span class="nm">${m.name} · ${m.cols}×${m.rows} · ${slots.length}P</span>`;
    const use = document.createElement('button');
    use.className = 'play';
    use.textContent = 'Use';
    use.addEventListener('click', () => {
      pendingCustom[which] = m;
      refreshCustomLabels();
      showOnly(which === 'mp' ? 'mp-overlay' : 'start-overlay');
    });
    row.appendChild(use);
    $('custom-list').appendChild(row);
  });
  showOnly('custom-overlay');
}
function refreshCustomLabels() {
  const s = pendingCustom.single;
  $('single-custom').textContent = s ? `🖌 ${s.name} × (click to clear)` : '';
  $('single-custom').onclick = s ? () => { pendingCustom.single = null; refreshCustomLabels(); } : null;
  $('single-custom').style.cursor = s ? 'pointer' : 'default';
  const m = pendingCustom.mp;
  $('mp-custom').textContent = m ? `🖌 ${m.name} × (click to clear)` : '';
  $('mp-custom').onclick = m ? () => { pendingCustom.mp = null; refreshCustomLabels(); } : null;
  $('mp-custom').style.cursor = m ? 'pointer' : 'default';
}

let OM = {};

function hx(h) {
  h = h.replace('#', '');
  if (h.length === 3) h = [...h].map((c) => c + c).join('');
  return [0, 2, 4].map((i) => parseInt(h.substr(i, 2), 16));
}
function mix(a, b, t) {
  const A = hx(a), B = hx(b);
  return '#' + A.map((v, i) => Math.round(v + (B[i] - v) * t).toString(16).padStart(2, '0')).join('');
}
function lumOf(h) {
  const [r, g, b] = hx(h);
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}
// Guarantee a readable gap between a foreground and the background by
// pulling the fg toward white (dark themes) or black (light themes).
function ensurePop(fg, bg, minGap = 0.28) {
  const dark = lumOf(bg) < 0.5;
  let c = fg;
  for (let i = 0; i < 12 && Math.abs(lumOf(c) - lumOf(bg)) < minGap; i++) {
    c = mix(c, dark ? '#ffffff' : '#000000', 0.18);
  }
  return c;
}
// Build a full UI variable set from one real Omarchy terminal palette.
// Icons always take the BRIGHT variants so pieces pop off their washes,
// and washes carry 30% hue so no territory ever melts into the background.
function varsFor(p) {
  const bg = p.background || '#0b0d13';
  const fg = p.foreground || '#c9d3e8';
  const red = ensurePop(p.bright_red || p.red || '#e25f66', bg);
  const green = ensurePop(p.bright_green || p.green || '#5fae86', bg);
  const yellow = ensurePop(p.bright_yellow || p.yellow || '#e0a23f', bg);
  const blue = ensurePop(p.bright_blue || p.blue || '#5c81c4', bg);
  const magenta = ensurePop(p.bright_magenta || p.magenta || '#b48ce8', bg);
  const cyan = ensurePop(p.bright_cyan || p.cyan || p.accent || '#3fc0cc', bg);
  return {
    '--bg': bg,
    '--panel': mix(bg, fg, 0.05),
    '--panel-line': mix(bg, fg, 0.20),
    '--panel-line-soft': mix(bg, fg, 0.10),
    '--ink': fg,
    '--ink-dim': mix(fg, bg, 0.38),
    '--ink-faint': mix(fg, bg, 0.62),
    '--water-bg': mix(blue, bg, 0.70),
    '--water-ink': blue,
    '--forest-bg': mix(green, bg, 0.70),
    '--forest-ink': green,
    '--grave-bg': mix(bg, fg, 0.07),
    '--grave-ink': mix(fg, bg, 0.45),
    '--open-bg': mix(yellow, bg, 0.80),
    '--you': cyan, '--you-bg': mix(cyan, bg, 0.70),
    '--karg': red, '--karg-bg': mix(red, bg, 0.72),
    '--vex': yellow, '--vex-bg': mix(yellow, bg, 0.72),
    '--mord': magenta, '--mord-bg': mix(magenta, bg, 0.70),
    '--gold': p.accent || yellow,
  };
}

const OM_VARS = Object.keys(varsFor({}));

function applyTheme(name) {
  const r = document.documentElement.style;
  if (OM[name]) {
    const vars = varsFor(OM[name]);
    for (const [k, v] of Object.entries(vars)) r.setProperty(k, v);
    document.documentElement.dataset.theme = 'omarchy-real';
  } else {
    for (const k of OM_VARS) r.removeProperty(k);
    document.documentElement.dataset.theme = name;
  }
  localStorage.setItem('slay-theme', name);
  const nm = $('theme-name');
  if (nm) nm.textContent = name;
  const st = $('st-theme');
  if (st) st.textContent = name;
  const sel = $('theme-select');
  if (sel) sel.value = name;
}

function segChanged(id) {
  if (id === 'seg-editsize') {
    const v = +document.querySelector('#seg-editsize .on').dataset.v;
    const [c, r] = ED_SIZES[v];
    if (c !== ED.cols || r !== ED.rows) edReset(c, r);
    return;
  }
  const hint = $('mp-hint');
  if (hint && ['seg-humans', 'seg-aifoes'].includes(id)) {
    const f = Math.min(mpCfg.foes, 4 - mpCfg.humans);
    hint.textContent = `${mpCfg.humans} humans · ${f} AI` +
      (mpCfg.humans + mpCfg.foes > 4 ? ' (capped at 4 seats)' : '');
  }
}

async function boot() {
  segWire('seg-foes', 'foes');
  segWire('seg-diff', 'diff');
  segWire('seg-size', 'size');
  segWire('seg-humans', 'humans', mpCfg);
  segWire('seg-aifoes', 'foes', mpCfg);
  segWire('seg-mpsize', 'size', mpCfg);
  segWire('seg-mpdiff', 'diff', mpCfg);
  $('seed-dice').addEventListener('click', () => {
    $('seed-input').value = Math.floor(Math.random() * 90000 + 10000);
  });
  $('btn-start').addEventListener('click', async () => {
    const raw = $('seed-input').value.trim();
    const seed = raw === '' ? null : parseInt(raw, 10);
    const custom = pendingCustom.single ? customPayload(pendingCustom.single) : null;
    startGame({
      mode: custom ? 'custom' : 'single',
      seed: custom ? null : (Number.isFinite(seed) ? seed : null),
      enemies: opts.foes, difficulty: opts.diff, size: opts.size,
      humans: [0], tutorial: false, custom,
    });
  });
  $('btn-start-back').addEventListener('click', showMenu);
  $('btn-menu-single').addEventListener('click', () => { refreshCustomLabels(); showOnly('start-overlay'); });
  $('btn-menu-multi').addEventListener('click', () => { segChanged(); refreshCustomLabels(); showOnly('mp-overlay'); });
  $('btn-menu-tutorial').addEventListener('click', () => startGame({
    mode: 'tutorial', seed: 424242, enemies: 1, difficulty: 'easy',
    size: 0, humans: [0], tutorial: true, custom: null,
  }));
  $('btn-menu-creator').addEventListener('click', () => {
    creatorView('paint');
    showOnly('creator-overlay');
  });
  $('btn-mp-back').addEventListener('click', showMenu);
  $('btn-mp-start').addEventListener('click', () => {
    const h = mpCfg.humans;
    const f = Math.min(mpCfg.foes, 4 - h);
    const custom = pendingCustom.mp ? customPayload(pendingCustom.mp) : null;
    const raw = ($('mp-seed-input').value || '').trim();
    const seed = raw === '' ? null : parseInt(raw, 10);
    if (custom) {
      const slots = [...new Set(pendingCustom.mp.starts.map((s) => s.slot))].sort();
      if (h > slots.length) {
        $('mp-hint').textContent = `That isle seats ${slots.length} — lower Humans first.`;
        return;
      }
    }
    startGame({
      mode: custom ? 'custom' : (h > 1 ? 'hotseat' : 'single'),
      seed: custom ? null : (Number.isFinite(seed) ? seed : null),
      enemies: h + f - 1, difficulty: mpCfg.diff, size: mpCfg.size,
      humans: [...Array(h).keys()], tutorial: false, custom,
    });
  });
  $('btn-coach-skip').addEventListener('click', () => { tutOff = true; $('coach-bar').classList.add('hidden'); });
  $('btn-coach-play').addEventListener('click', showMenu);
  // creator toolbar
  segWire('seg-tool', 'tool', ED);
  segWire('seg-editsize', 'edsize', { edsize: 0 });
  $('btn-edit-clear').addEventListener('click', () => {
    const [c, r] = ED_SIZES[+document.querySelector('#seg-editsize .on').dataset.v];
    edReset(c, r);
  });
  $('btn-edit-save').addEventListener('click', () => {
    const name = ($('map-name').value.trim() || `Isle ${getMaps().length + 1}`).slice(0, 24);
    const slots = [...new Set(ED.starts.values())].sort();
    if (ED.land.size < 12) { $('edit-status').textContent = 'Need 12+ land hexes.'; return; }
    if (slots.length < 2) { $('edit-status').textContent = 'Need 2+ starts (Start tool).'; return; }
    const maps = getMaps();
    maps.push({
      name, cols: ED.cols, rows: ED.rows,
      land: [...ED.land].map((k) => k.split(',').map(Number)),
      starts: slots.map((sl) => ({
        slot: sl,
        at: [...ED.starts.entries()].find(([k, v]) => v === sl)[0].split(',').map(Number),
      })),
    });
    localStorage.setItem('slay-maps', JSON.stringify(maps));
    $('map-name').value = '';
    renderMapList();
    $('edit-status').textContent = `Saved “${name}”.`;
  });
  $('btn-edit-play').addEventListener('click', () => {
    const slots = [...new Set(ED.starts.values())].sort();
    if (ED.land.size < 12 || slots.length < 2) {
      $('edit-status').textContent = 'Need 12+ land and 2+ starts first.';
      return;
    }
    playCustom({
      name: 'draft', cols: ED.cols, rows: ED.rows,
      land: [...ED.land].map((k) => k.split(',').map(Number)),
      starts: slots.map((sl) => ({
        slot: sl,
        at: [...ED.starts.entries()].find(([k, v]) => v === sl)[0].split(',').map(Number),
      })),
    });
  });
  $('btn-edit-back').addEventListener('click', showMenu);
  $('btn-view-paint').addEventListener('click', () => creatorView('paint'));
  $('btn-view-lib').addEventListener('click', () => creatorView('lib'));
  // custom-map picker shared by single + multiplayer setups
  $('seed-custom').addEventListener('click', () => openPicker('single'));
  $('mp-seed-custom').addEventListener('click', () => openPicker('mp'));
  $('mp-seed-dice').addEventListener('click', () => {
    $('mp-seed-input').value = Math.floor(Math.random() * 90000 + 10000);
  });
  $('btn-custom-random').addEventListener('click', () => {
    pendingCustom[pickerFor] = null;
    refreshCustomLabels();
    showOnly(pickerFor === 'mp' ? 'mp-overlay' : 'start-overlay');
  });
  $('btn-custom-back').addEventListener('click', () => {
    showOnly(pickerFor === 'mp' ? 'mp-overlay' : 'start-overlay');
  });
  $('btn-same').addEventListener('click', () => cmd('rematch', { same: true }));
  $('btn-new').addEventListener('click', showMenu);
  $('btn-close-help').addEventListener('click', () => $('help-overlay').classList.add('hidden'));
  $('row-peasant').addEventListener('click', () => cmd('buy', { kind: 'man' }));
  $('row-castle').addEventListener('click', () => cmd('buy', { kind: 'castle' }));
  $('btn-end-turn').addEventListener('click', () => cmd('end_turn'));
  $('btn-undo').addEventListener('click', () => cmd('undo'));
  $('btn-end-turn').addEventListener('click', () => cmd('end_turn'));

  // themes: Atoll house default + every real Omarchy palette in a dropdown
  let presets = ['atoll'];
  try {
    const res = await fetch('omarchy-themes.json');
    OM = await res.json();
    presets = presets.concat(Object.keys(OM).sort());
  } catch { OM = {}; }
  try {
    omarchy = await invoke('omarchy_theme');
  } catch { omarchy = null; }
  const label = (v) => v.split('-').map((w) => w[0].toUpperCase() + w.slice(1)).join(' ');
  $('theme-select').innerHTML = presets.map((v) => `<option value="${v}">${label(v)}</option>`).join('');
  $('theme-select').addEventListener('change', (e) => applyTheme(e.target.value));
  const stored = localStorage.getItem('slay-theme');
  if (stored && presets.includes(stored)) applyTheme(stored);
  else if (omarchy && omarchy.name && presets.includes(omarchy.name)) applyTheme(omarchy.name);
  else applyTheme('atoll');
  showMenu();
}

// click-drag panning for big maps (mouse only; touch uses native scroll).
// A drag suppresses the click that would otherwise order a hex.
(function () {
  const stage = document.querySelector('.mapstage');
  const field = document.getElementById('hexfield');
  let down = null;
  let suppressed = false;
  stage.addEventListener('pointerdown', (e) => {
    if (e.pointerType !== 'mouse' || e.button !== 0) return;
    down = { x: e.clientX, y: e.clientY, sl: stage.scrollLeft, st: stage.scrollTop, moved: false };
  });
  window.addEventListener('pointermove', (e) => {
    if (!down) return;
    const dx = e.clientX - down.x, dy = e.clientY - down.y;
    if (!down.moved && Math.hypot(dx, dy) > 6) {
      down.moved = true;
      stage.classList.add('dragging');
    }
    if (down.moved) {
      stage.scrollLeft = down.sl - dx;
      stage.scrollTop = down.st - dy;
    }
  });
  const up = () => {
    if (down && down.moved) suppressed = true;
    down = null;
    stage.classList.remove('dragging');
  };
  window.addEventListener('pointerup', up);
  window.addEventListener('pointercancel', up);
  field.addEventListener('click', (e) => {
    if (suppressed) {
      suppressed = false;
      e.stopPropagation();
      e.preventDefault();
    }
  }, true);
})();

boot();
