// WinIsland: macOS-notch style Dynamic Island.
// Compact notch → live activities (music / timer / privacy) → HUD & drop-down events → expanded dashboard.
// The host clips the window region to the bounds we report, so only the notch itself takes clicks.
'use strict';

const $ = (id) => document.getElementById(id);
const notch = $('notch');
const CANVAS_W = 760, SH = 12, MARGIN = 26;
const V = { compact: $('vCompact'), hud: $('vHud'), drop: $('vDrop'), exp: $('vExp') };
const S = () => Frost.state.settings || { island: {} };
const I = (k) => S().island?.[k];
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

let view = 'compact', size = [210, 32, 14], hover = false, visible = true;
let transientT = 0, collapseT = 0, boundsT = 0, hoverT = 0, tickT = 0;
let lastTitle = null, mediaSeen = false, privacy = { camera: false, mic: false };
let tab = 'home';

// ------------------------------------------------ a real spring (damped harmonic oscillator → CSS linear())
(function spring(stiffness = 210, damping = 21) {
  let x = 0, v = 0; const pts = [], dt = 1 / 120;
  for (let i = 0; i < 240; i++) { const a = stiffness * (1 - x) - damping * v; v += a * dt; x += v * dt; pts.push(x); if (i > 30 && Math.abs(1 - x) < 0.0015 && Math.abs(v) < 0.01) break; }
  const step = Math.max(1, Math.floor(pts.length / 48));
  const vals = pts.filter((_, i) => i % step === 0).map((p) => p.toFixed(4));
  vals[vals.length - 1] = '1';
  document.documentElement.style.setProperty('--spring', `linear(0, ${vals.join(', ')})`);
  document.documentElement.style.setProperty('--dur', (pts.length * dt).toFixed(2) + 's');
})();
const DUR = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--dur')) * 1000 || 620;

// ------------------------------------------------ two looks: macOS notch (flush to the top edge) and iOS island (floating capsule)
const IOS = () => I('style') === 'ios';
const TOP = () => (IOS() ? 10 : 0);
// [width, height, corner radius] for each state, per look
const SZ = {
  compact: (w) => (IOS() ? [Math.max(150, w - 30) + (hover ? 10 : 0), hover ? 40 : 37, 999] : [w + (hover ? 12 : 0), hover ? 35 : 32, hover ? 15 : 14]),
  hud: () => (IOS() ? [300, 37, 999] : [380, 32, 14]),
  drop: (w) => (IOS() ? [w - 10, 84, 42] : [w, 100, 28]),
  exp: () => [640, Math.ceil(V.exp.offsetHeight), IOS() ? 44 : 32],
};
function applyStyle() {
  const ios = IOS();
  if (document.body.classList.contains('ios') === ios) return false;
  document.body.classList.toggle('ios', ios);
  return true;
}

// ------------------------------------------------ geometry + click region
function reportBounds(w, h) {
  const full = w + SH * 2 + MARGIN * 2;
  Frost.send({ type: 'bounds', x: Math.max(0, (CANVAS_W - full) / 2), y: 0, w: Math.min(CANVAS_W, full), h: h + TOP() + MARGIN });
}
function applySize(next) {
  const [w0, h0] = size, [w1, h1, r1] = next;
  reportBounds(Math.max(w0, w1), Math.max(h0, h1)); // during the morph, cover both shapes
  clearTimeout(boundsT); boundsT = setTimeout(() => reportBounds(w1, h1), DUR + 40);
  notch.style.width = w1 + 'px'; notch.style.height = h1 + 'px';
  const r = Math.min(r1, h1 / 2);
  notch.style.borderRadius = IOS() ? `${r}px` : `0 0 ${r}px ${r}px`;
  size = next;
}
function show(v) {
  const was = view; view = v;
  for (const [k, el] of Object.entries(V)) el.classList.toggle('on', k === v);
  if (v === 'exp' && was !== 'exp') { Frost.send({ type: 'subscribe', stats: !!I('system') }); }
  if (v !== 'exp' && was === 'exp') { Frost.send({ type: 'subscribe', stats: false }); }
  updateLevelsWanted(); updateTick();
}

// ------------------------------------------------ compact (idle / music / timer live activities)
function compactModel() {
  const m = Frost.state.media, w = Frost.state.weather;
  const dots = (I('privacy') ? (privacy.camera ? '<span class="dot cam"></span>' : '') + (privacy.mic ? '<span class="dot mic"></span>' : '') : '');
  const nDots = (dots.match(/dot /g) || []).length;
  if (Timer.running()) {
    const p = Timer.progress();
    return { w: 290 + nDots * 12, mode: 'timer',
      l: `<svg class="ring" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7.5" stroke="rgba(255,159,10,.25)"/><circle cx="10" cy="10" r="7.5" stroke="#ff9f0a" stroke-dasharray="47.1" stroke-dashoffset="${(47.1 * (1 - p)).toFixed(1)}" transform="rotate(-90 10 10)" stroke-linecap="round"/></svg>`,
      r: `<span class="tcount">${Timer.label()}</span>${dots}` };
  }
  if (I('media') && m?.active && m.playing) {
    const lyr = notchLyrics();
    return { w: (lyr ? 500 : 320) + nDots * 12, mode: 'music', lyr,
      l: `<div class="art" style="${m.art ? `background-image:url('${m.art}')` : ''}"></div>`,
      r: `${I('visualizer') ? vizHTML(5) : Icons.music}${dots}` };
  }
  if (I('idleInfo')) {
    const c = Frost.christmas();
    let r = '';
    if (I('weather') && w?.temp != null) r = `${weatherIcon(w.code, w.isDay)}<span>${w.temp}°</span>`;
    else r = `${Icons.tree}<span>${c.isEve || c.isDay ? 'God Jul' : c.days + 'd'}</span>`;
    return { w: 236 + nDots * 12, mode: 'idle', l: I('time') !== false ? `<span>${Frost.timeStr()}</span>` : '', r: r + dots };
  }
  return { w: nDots ? 210 + nDots * 14 : 200, mode: 'idle', l: '', r: dots };
}
let compactMode = 'idle';
function renderCompact(resize = true) {
  const c = compactModel();
  compactMode = c.mode;
  $('cL').innerHTML = c.l; $('cR').innerHTML = c.r;
  if (!c.lyr) { $('cM').innerHTML = ''; }
  if (view === 'compact' && resize) applySize(SZ.compact(c.w));
  updateLevelsWanted(); updateLyricTick();
}

// ------------------------------------------------ transient events
function transient(v, sz, ms) {
  if (view === 'exp') return;
  clearTimeout(transientT);
  show(v); applySize(sz);
  transientT = setTimeout(back, ms);
}
function back() { clearTimeout(transientT); show('compact'); renderCompact(); }

function showVolume(level, muted, force) {
  $('hIcon').innerHTML = muted || level === 0 ? Icons.mute : Icons.vol;
  $('hBar').style.transform = `scaleX(${muted ? 0 : level})`;
  $('hPct').textContent = muted ? 'Muted' : Math.round(level * 100) + '%';
  if (view === 'exp') { syncVolume(); return; }
  if (force || I('volume')) transient('hud', SZ.hud(), 1600);
}

function drop({ pic, picIcon, k, t1, t2, trail = '', ms = 3600, w = 430 }) {
  $('dPic').style.backgroundImage = pic ? `url('${pic}')` : '';
  $('dPic').innerHTML = pic ? '' : picIcon || '';
  $('dK').textContent = k || ''; $('dT1').textContent = t1 || ''; $('dT2').textContent = t2 || '';
  $('dTrail').innerHTML = trail;
  transient('drop', SZ.drop(w), ms);
}

function greet() {
  if (!I('events')) return;
  const h = new Date().getHours(), c = Frost.christmas();
  drop({ picIcon: `<span style="color:#30d158">${Icons.tree}</span>`, k: 'WinIsland',
    t1: c.isEve ? 'God Jul! Merry Christmas Eve' : c.isDay ? 'Merry Christmas' : h < 5 ? 'Good night' : h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening',
    t2: c.isEve || c.isDay ? 'Warm wishes from the north' : `${c.days} days until Christmas Eve`, ms: 4200 });
}

// ------------------------------------------------ album tint (dominant vivid colour of the cover)
let tintFor = '';
function updateTint(art) {
  if (!art || art === tintFor) { if (!art) document.documentElement.style.setProperty('--tint', getComputedStyle(document.documentElement).getPropertyValue('--accent')); return; }
  tintFor = art;
  const img = new Image();
  img.onload = () => {
    try {
      const c = document.createElement('canvas'); c.width = c.height = 14;
      const g = c.getContext('2d'); g.drawImage(img, 0, 0, 14, 14);
      const d = g.getImageData(0, 0, 14, 14).data;
      let best = null, bestScore = -1, sr = 0, sg = 0, sb = 0, n = 0;
      for (let i = 0; i < d.length; i += 4) {
        const r = d[i], gg = d[i + 1], b = d[i + 2], mx = Math.max(r, gg, b), mn = Math.min(r, gg, b);
        const sat = mx ? (mx - mn) / mx : 0, score = sat * 0.7 + (mx / 255) * 0.3;
        sr += r; sg += gg; sb += b; n++;
        if (score > bestScore && mx > 60) { bestScore = score; best = [r, gg, b]; }
      }
      const grey = !best || bestScore < 0.25;
      if (grey) best = [sr / n, sg / n, sb / n];
      // keep the hue, but force a lightness/saturation that reads well on pure black
      const [r, gg, b] = best.map((v) => v / 255), mx = Math.max(r, gg, b), mn = Math.min(r, gg, b), dd = mx - mn;
      let h = 0;
      if (dd) h = mx === r ? ((gg - b) / dd) % 6 : mx === gg ? (b - r) / dd + 2 : (r - gg) / dd + 4;
      h = Math.round(h * 60 + 360) % 360;
      document.documentElement.style.setProperty('--tint', grey ? 'hsl(0 0% 82%)' : `hsl(${h} 85% 68%)`);
    } catch {}
  };
  img.src = art;
}

// ------------------------------------------------ audio visualizer (real output levels from the host)
const vizHTML = (n, big) => `<div class="viz${big ? ' big' : ''}">${'<i></i>'.repeat(n)}</div>`;
let levelsOn = false, lvl = 0, lvlAt = 0, vizT = 0;
const phases = Array.from({ length: 8 }, (_, i) => [1.7 + i * 0.83, i * 1.3]);
function updateLevelsWanted() {
  const m = Frost.state.media;
  const want = visible && !!I('visualizer') && !!m?.active && !!m.playing && (view === 'exp' || (view === 'compact' && compactMode === 'music') || view === 'drop');
  if (want !== levelsOn) {
    levelsOn = want;
    Frost.send({ type: 'levels', on: want });
    clearInterval(vizT);
    if (want) vizT = setInterval(drawViz, 66);
    else document.querySelectorAll('.viz i').forEach((b) => (b.style.transform = 'scaleY(.15)'));
  }
}
function drawViz() {
  const t = performance.now() / 1000, fresh = performance.now() - lvlAt < 250 && lvl > 0.004;
  // no real signal (e.g. system volume at 0) → gentle synthetic motion so it never looks dead
  const base = fresh ? Math.min(1, Math.pow(lvl, 0.6) * 1.25) : 0.35 + 0.25 * Math.sin(t * 1.3);
  document.querySelectorAll('.viz').forEach((v) => {
    [...v.children].forEach((b, i) => {
      const [f, p] = phases[i % phases.length];
      const s = Math.max(0.15, Math.min(1, base * (0.45 + 0.55 * Math.abs(Math.sin(t * f * (fresh ? 2.2 : 1.4) + p)))));
      b.style.transform = `scaleY(${s.toFixed(2)})`;
    });
  });
}

// ------------------------------------------------ timer (live activity)
const Timer = (() => {
  let st = { end: 0, total: 0, paused: 0 };
  try { st = JSON.parse(localStorage.getItem('frost.timer')) || st; } catch {}
  const save = () => { try { localStorage.setItem('frost.timer', JSON.stringify(st)); } catch {} };
  const remaining = () => (st.paused ? st.paused : Math.max(0, st.end - Date.now()));
  return {
    running: () => st.total > 0 && (st.paused > 0 || st.end > Date.now()),
    paused: () => st.paused > 0,
    remaining,
    progress: () => (st.total ? 1 - remaining() / st.total : 0),
    label: () => { const s = Math.ceil(remaining() / 1000); return `${Math.floor(s / 60)}:${Frost.pad(s % 60)}`; },
    start(min) { st = { total: min * 60000, end: Date.now() + min * 60000, paused: 0 }; save(); },
    toggle() { if (st.paused) { st.end = Date.now() + st.paused; st.paused = 0; } else if (st.end > Date.now()) st.paused = st.end - Date.now(); save(); },
    reset() { st = { end: 0, total: 0, paused: 0 }; save(); },
    finishedNow() { return st.total > 0 && !st.paused && st.end > 0 && st.end <= Date.now(); },
  };
})();
function chime() {
  try {
    const ctx = new AudioContext(), t = ctx.currentTime;
    [[880, 0], [1318.5, 0.18], [1760, 0.36]].forEach(([f, d]) => {
      const o = ctx.createOscillator(), g = ctx.createGain();
      o.type = 'sine'; o.frequency.value = f; g.gain.setValueAtTime(0, t + d);
      g.gain.linearRampToValueAtTime(0.18, t + d + 0.02); g.gain.exponentialRampToValueAtTime(0.001, t + d + 1.2);
      o.connect(g).connect(ctx.destination); o.start(t + d); o.stop(t + d + 1.3);
    });
    setTimeout(() => ctx.close(), 2000);
  } catch {}
}
function updateTick() {
  clearInterval(tickT);
  const needs = Timer.running() || (view === 'exp' && Frost.state.media?.playing);
  if (needs) tickT = setInterval(tick, 1000);
  updateLyricTick();
}

// ------------------------------------------------ karaoke lyrics (time-synced, fetched on demand by the host)
const LEAD = 0.2; // show each line a hair early, like karaoke machines do
const Lyrics = { key: '', req: '', state: 'none', lines: [], plain: '' }; // state: none|loading|synced|plain|instrumental|missing
const trackKey = (m) => (m?.active ? `${m.app}|${m.title}|${m.artist}` : '');
const lyricsOn = () => I('lyrics') !== false && !!I('media');
const notchLyrics = () => lyricsOn() && I('lyricsNotch') !== false && Lyrics.state === 'synced' && Lyrics.key === trackKey(Frost.state.media);

/** Request lyrics for the current track if we don't have them yet. Returns true when a new lookup started. */
function ensureLyrics() {
  const m = Frost.state.media, k = trackKey(m);
  if (!lyricsOn() || !k || k === Lyrics.key) return false;
  Object.assign(Lyrics, { key: k, req: k + '#' + Date.now(), state: 'loading', lines: [], plain: '' });
  lyrIdx = -2;
  Frost.send({ type: 'lyrics', req: Lyrics.req, artist: m.artist, title: m.title, album: m.album, duration: m.duration || 0, browser: !!m.browser });
  return true;
}
function parseLRC(s) {
  const out = [];
  for (const line of s.split(/\r?\n/)) {
    const tags = [...line.matchAll(/\[(\d+):(\d+(?:[.:]\d+)?)\]/g)]; if (!tags.length) continue;
    const text = line.replace(/\[[^\]]*\]/g, '').trim();
    for (const t of tags) out.push({ t: +t[1] * 60 + parseFloat(t[2].replace(':', '.')), text });
  }
  return out.sort((a, b) => a.t - b.t);
}
Frost.on('lyrics', (r) => {
  if (r.req !== Lyrics.req) return; // a stale answer for a song that already ended
  Lyrics.lines = r.synced ? parseLRC(r.synced) : [];
  Lyrics.plain = r.plain || '';
  Lyrics.state = Lyrics.lines.length ? 'synced' : r.instrumental ? 'instrumental' : Lyrics.plain ? 'plain' : 'missing';
  lyrIdx = -2;
  if (view === 'exp' && tab === 'lyrics') renderLyrics();
  if (view === 'compact') renderCompact();
});

function lyricIndex(pos) {
  const L = Lyrics.lines; let lo = 0, hi = L.length - 1, i = -1;
  while (lo <= hi) { const mid = (lo + hi) >> 1; if (L[mid].t <= pos) { i = mid; lo = mid + 1; } else hi = mid - 1; }
  return i;
}
const lyricSurface = () => (view === 'exp' && tab === 'lyrics' ? 'page' : view === 'compact' && compactMode === 'music' && notchLyrics() ? 'notch' : null);

let lyrT = 0, lyrIdx = -2, lyrSig = '';
function updateLyricTick() {
  const m = Frost.state.media, surf = lyricSurface();
  const want = visible && Lyrics.state === 'synced' && !!m?.playing && !!surf;
  if (!!lyrT !== want) { clearInterval(lyrT); lyrT = want ? setInterval(() => lyricStep(false), 150) : 0; }
  // repaint fully when the surface, play state or timeline anchor changes (pause, seek, switching views)
  const sig = `${surf}|${!!m?.playing}|${m?.posStamp}|${m?.position}`;
  if (sig !== lyrSig) { lyrSig = sig; lyrIdx = -2; }
  if (surf) lyricStep(false);
}
function lyricStep(force) {
  if (Lyrics.state !== 'synced') return;
  const surf = lyricSurface(); if (!surf) return;
  const pos = mediaPos() + LEAD, i = lyricIndex(pos);
  if (i === lyrIdx && !force) return;
  lyrIdx = i;
  if (surf === 'page') paintLyricsPage(i, pos); else paintLyricsNotch(i, pos);
}
// The karaoke fill: a white copy of the line revealed left → right at roughly singing speed. Done with two opposing
// transforms (clip window slides right, text inside slides back) so the compositor animates it without repainting text.
const karHTML = (t) => `<span class="kar"><span class="base">${t}</span><span class="hl" aria-hidden="true"><span class="in">${t}</span></span></span>`;
function setFill(k, p) {
  const hl = k.querySelector('.hl'), inn = k.querySelector('.in'); if (!hl) return;
  hl.getAnimations().forEach((a) => a.cancel()); inn.getAnimations().forEach((a) => a.cancel());
  hl.style.transform = `translateX(${((p - 1) * 100).toFixed(1)}%)`; inn.style.transform = `translateX(${((1 - p) * 100).toFixed(1)}%)`;
}
function sweep(k, i, pos) {
  const L = Lyrics.lines, cur = L[i]; if (!cur) return;
  const gap = (L[i + 1] ? L[i + 1].t : cur.t + 5) - cur.t;
  const len = Math.max(0.4, Math.min(gap * 0.92, Math.max(1.4, cur.text.length * 0.085)));
  const p0 = Math.min(1, Math.max(0, (pos - cur.t) / len));
  setFill(k, p0);
  if (!Frost.state.media?.playing || p0 >= 1) return;
  const o = { duration: (1 - p0) * len * 1000, fill: 'forwards' };
  k.querySelector('.hl').animate([{ transform: `translateX(${((p0 - 1) * 100).toFixed(1)}%)` }, { transform: 'translateX(0)' }], o);
  k.querySelector('.in').animate([{ transform: `translateX(${((1 - p0) * 100).toFixed(1)}%)` }, { transform: 'translateX(0)' }], o);
}
const lineText = (l) => (l && l.text ? esc(l.text) : '♪');

function paintLyricsNotch(i, pos) {
  const el = $('cM');
  el.innerHTML = karHTML(i < 0 ? '♪' : lineText(Lyrics.lines[i]));
  if (i >= 0) sweep(el.firstChild, i, pos);
}

let lyrPageKey = null;
function renderLyrics() {
  const m = Frost.state.media, pg = $('pgLyrics');
  ensureLyrics();
  lyrPageKey = trackKey(m);
  const head = m?.active ? `<div class="lhead"><div class="art" style="${m.art ? `background-image:url('${m.art}')` : ''}">${m.art ? '' : Icons.music}</div>
      <div class="meta"><div class="t1">${esc(m.title)}</div><div class="t2">${esc(m.artist)}${m.app ? ` · ${esc(m.app)}` : ''}</div></div></div>` : '';
  let body;
  if (!m?.active) body = `<div class="lmsg">${Icons.mic}<div>Nothing playing<br><span>Play something in Spotify to sing along</span></div></div>`;
  else if (Lyrics.state === 'loading') body = `<div class="lmsg">${Icons.mic}<div>Looking up lyrics…</div></div>`;
  else if (Lyrics.state === 'instrumental') body = `<div class="lmsg">${Icons.music}<div>Instrumental<br><span>Nothing to sing on this one</span></div></div>`;
  else if (Lyrics.state === 'missing' || Lyrics.state === 'none') body = `<div class="lmsg">${Icons.mic}<div>No lyrics found<br><span>LRCLIB doesn't have this song yet</span></div></div>`;
  else if (Lyrics.state === 'plain') body = `<div class="lplain">${Lyrics.plain.split(/\r?\n/).map((l) => `<p>${esc(l) || '&nbsp;'}</p>`).join('')}<div class="lnote">Not time-synced for this song</div></div>`;
  else body = `<div class="lview" id="lyView"><div class="lines" id="lyLines">${Lyrics.lines.map((l, i) => `<div class="ln" data-i="${i}">${karHTML(lineText(l))}</div>`).join('')}</div></div>`;
  pg.innerHTML = `<div class="lyr">${head}${body}</div>`;
  pg.querySelectorAll('.ln').forEach((ln) => (ln.onclick = (e) => {
    e.stopPropagation();
    const mm = Frost.state.media, t = Lyrics.lines[+ln.dataset.i]?.t; if (t == null || !mm) return;
    Frost.send({ type: 'seek', position: Math.max(0, t - LEAD + 0.05) });
    mm.position = t; mm.posStamp = Date.now(); lyricStep(true);
  }));
  lyrIdx = -2; lyricStep(true);
}
function paintLyricsPage(i, pos) {
  const lines = $('lyLines'), view_ = $('lyView'); if (!lines) return;
  lines.querySelectorAll('.ln').forEach((ln) => {
    const j = +ln.dataset.i;
    ln.classList.toggle('cur', j === i); ln.classList.toggle('past', j < i);
    if (j !== i) setFill(ln.firstChild, j < i ? 1 : 0);
  });
  const cur = lines.querySelector(`.ln[data-i="${Math.max(0, i)}"]`);
  if (cur) {
    if (i >= 0) sweep(cur.firstChild, i, pos);
    const y = cur.offsetTop + cur.offsetHeight / 2 - view_.clientHeight * 0.42;
    lines.style.transform = `translateY(${-Math.max(0, y).toFixed(0)}px)`;
  }
}

function renderSources() {
  const el = $('xSrcs'), list = (I('media') && Frost.state.media?.sources) || [];
  if (list.length < 2) { el.innerHTML = ''; return; }
  el.innerHTML = list.map((s) => `<button class="src${s.selected ? ' on' : ''}" data-id="${esc(s.id)}" title="${esc(s.title)}${s.artist ? ' · ' + esc(s.artist) : ''}">`
    + `<span class="pl${s.playing ? ' on' : ''}"></span><b>${esc(s.app)}</b><em>${esc(s.title)}</em></button>`).join('');
  el.querySelectorAll('.src').forEach((b) => (b.onclick = (e) => {
    e.stopPropagation();
    if (b.classList.contains('on')) return;
    el.querySelectorAll('.src').forEach((x) => x.classList.toggle('on', x === b));
    Frost.send({ type: 'selectSource', id: b.dataset.id });
  }));
}
function tick() {
  if (Timer.finishedNow()) {
    Timer.reset(); chime(); renderCompact();
    if (view === 'exp') renderTimer();
    drop({ picIcon: `<span style="color:#ff9f0a">${Icons.clock}</span>`, k: 'Timer', t1: "Time's up", t2: 'Your timer has finished', ms: 8000, w: 400 });
    updateTick(); return;
  }
  if (Timer.running()) { if (view === 'compact') renderCompact(false); if (view === 'exp' && tab === 'timer') renderTimerDial(); }
  if (view === 'exp' && tab === 'home') { const p = $('xPos'); if (p) p.textContent = Frost.clock(mediaPos()); }
}

// ------------------------------------------------ expanded dashboard
function renderExpanded() {
  if (tab === 'lyrics' && !lyricsOn()) tab = 'home';
  $('tabHome').innerHTML = `${Icons.home}Home`;
  $('tabLyrics').innerHTML = `${Icons.mic}Lyrics`;
  $('tabLyrics').classList.toggle('hidden', !lyricsOn());
  $('tabTimer').innerHTML = `${Icons.clock}Timer`;
  $('bGear').innerHTML = Icons.gear;
  document.querySelectorAll('.tab').forEach((t) => t.classList.toggle('on', t.dataset.tab === tab));
  $('pgHome').classList.toggle('on', tab === 'home'); $('pgLyrics').classList.toggle('on', tab === 'lyrics'); $('pgTimer').classList.toggle('on', tab === 'timer');
  $('xDots').innerHTML = I('privacy') ? (privacy.camera ? '<span class="dot cam" title="Camera in use"></span>' : '') + (privacy.mic ? '<span class="dot mic" title="Microphone in use"></span>' : '') : '';
  renderSources();
  if (tab === 'home') renderHome(); else if (tab === 'lyrics') renderLyrics(); else renderTimer();
  updateLyricTick();
}

function renderHome() {
  const m = Frost.state.media, v = Frost.state.volume || { level: 0.5 }, w = Frost.state.weather, s = Frost.state.stats, c = Frost.christmas();
  let np;
  if (I('media') && m?.active) {
    np = `<div class="np">
      <div class="cover"><div class="halo" style="${m.art ? `background-image:url('${m.art}')` : ''}"></div><div class="img" style="${m.art ? `background-image:url('${m.art}')` : ''}">${m.art ? '' : Icons.music}</div></div>
      <div class="info">
        <div class="row1"><div class="titles"><div class="t1">${esc(m.title)}</div><div class="t2">${esc(m.artist)}</div>${m.app ? `<div class="app">${esc(m.app)}</div>` : ''}</div>${I('visualizer') ? vizHTML(6, true) : ''}</div>
        <div class="scrub"><div class="track" id="xTrack"><div class="rail"><div class="fill" id="xFill"></div></div></div>
          <div class="times"><span id="xPos">${Frost.clock(mediaPos())}</span><span>${m.duration ? '-' + Frost.clock(m.duration - mediaPos()) : ''}</span></div></div>
        <div class="ctrls"><button id="xPrev" ${m.canPrev ? '' : 'disabled'}>${Icons.prev}</button><button class="main" id="xPlay">${m.playing ? Icons.pause : Icons.play}</button><button id="xNext" ${m.canNext ? '' : 'disabled'}>${Icons.next}</button></div>
      </div></div>`;
  } else np = `<div class="np"><div class="empty"><div class="img">${Icons.music}</div><div>Nothing playing<br><span style="font-size:12px">Play something in Spotify or your browser</span></div></div></div>`;

  const sky = w?.temp != null ? `${Frost.wmo(w.code)[1]} ${w.isDay === false ? 'night' : 'day'}` : '';
  const wx = I('weather') ? (w?.temp != null
    ? `<div class="mini wxw" data-sky="${sky}"><div class="h">${esc((w.city || 'Weather').split(',')[0])}</div><div class="wx"><div class="tmp">${w.temp}°</div>${weatherIcon(w.code, w.isDay)}</div><div class="sub">${Frost.wmo(w.code)[0]} · H${w.hi}° L${w.lo}°</div></div>`
    : `<div class="mini"><div class="h">Weather</div><div class="sub" style="margin-top:4px">Set a city in Settings</div></div>`) : '';
  const ring = (id, r, col) => `<circle cx="32" cy="32" r="${r}" stroke="${col}" stroke-opacity=".22"/><circle id="${id}" cx="32" cy="32" r="${r}" stroke="${col}" stroke-dasharray="${(2 * Math.PI * r).toFixed(1)}" stroke-dashoffset="${(2 * Math.PI * r).toFixed(1)}" transform="rotate(-90 32 32)"/>`;
  const sys = !I('system') ? '' : IOS()
    ? `<div class="mini sysw"><svg class="rings" viewBox="0 0 64 64">${ring('rCpu', 27, '#ff375f')}${ring('rRam', 18, '#30d158')}</svg>
        <div class="rl"><div><i style="background:#ff375f"></i>CPU <b id="sCpuV">–</b></div><div><i style="background:#30d158"></i>RAM <b id="sRamV">–</b></div></div></div>`
    : `<div class="mini"><div class="h">System</div><div class="bars">
      <div><span>CPU</span><span class="b"><i id="sCpu"></i></span><b id="sCpuV">–</b></div>
      <div><span>RAM</span><span class="b"><i id="sRam"></i></span><b id="sRamV">–</b></div></div></div>`;
  const xm = `<div class="mini"><div class="h">Christmas Eve</div><div class="sub" style="color:var(--text);font-size:13px;margin-top:2px">${c.isEve || c.isDay ? 'God Jul! 🎄' : `${c.days} days to go`}</div></div>`;

  $('pgHome').innerHTML = `<div class="home">${np}<div class="side">${wx}${sys || xm}</div></div>
    <div class="volrow"><span id="xMute">${v.muted ? Icons.mute : Icons.vol}</span><input type="range" id="xVol" min="0" max="100" value="${Math.round(v.level * 100)}"></div>`;
  wireHome(); updateStats(); updateProgress();
}

function wireHome() {
  const on = (id, fn) => { const e = $(id); if (e) e.onclick = fn; };
  on('xPrev', () => Frost.send({ type: 'media', action: 'prev' }));
  on('xNext', () => Frost.send({ type: 'media', action: 'next' }));
  on('xPlay', () => Frost.send({ type: 'media', action: 'toggle' }));
  on('xMute', () => Frost.send({ type: 'toggleMute' }));
  const tr = $('xTrack');
  if (tr) tr.onclick = (e) => {
    const m = Frost.state.media; if (!m?.duration) return;
    const r = tr.getBoundingClientRect(), f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    Frost.send({ type: 'seek', position: f * m.duration });
    m.position = f * m.duration; m.posStamp = Date.now(); updateProgress();
  };
  const vol = $('xVol');
  if (vol) {
    let t = 0; vol.style.setProperty('--v', vol.value + '%');
    vol.oninput = () => { vol.style.setProperty('--v', vol.value + '%'); clearTimeout(t); t = setTimeout(() => Frost.send({ type: 'setVolume', level: vol.value / 100 }), 30); };
  }
}
function syncVolume() {
  const v = Frost.state.volume, el = $('xVol'); if (!v || !el || document.activeElement === el) return;
  el.value = Math.round(v.level * 100); el.style.setProperty('--v', el.value + '%');
  const mb = $('xMute'); if (mb) mb.innerHTML = v.muted ? Icons.mute : Icons.vol;
}
function mediaPos() {
  const m = Frost.state.media; if (!m?.active || !m.duration) return 0;
  return Math.min(m.duration, m.position + (m.playing ? Math.max(0, (Date.now() - m.posStamp) / 1000) : 0));
}
let progAnim = null;
function updateProgress() {
  const m = Frost.state.media, f = $('xFill'); if (!f || !m?.duration) return;
  progAnim?.cancel();
  const pos = mediaPos(), from = pos / m.duration;
  f.style.transform = `scaleX(${from})`;
  if (m.playing) progAnim = f.animate([{ transform: `scaleX(${from})` }, { transform: 'scaleX(1)' }], { duration: (m.duration - pos) * 1000, fill: 'forwards' });
}
function updateStats() {
  const s = Frost.state.stats; if (!s || !$('sCpuV')) return;
  $('sCpuV').textContent = Math.round(s.cpu) + '%'; $('sRamV').textContent = Math.round(s.ram) + '%';
  if ($('sCpu')) { $('sCpu').style.transform = `scaleX(${s.cpu / 100})`; $('sRam').style.transform = `scaleX(${s.ram / 100})`; }
  const arc = (id, r, v) => { const e = $(id); if (e) e.style.strokeDashoffset = (2 * Math.PI * r * (1 - Math.min(1, v / 100))).toFixed(1); };
  arc('rCpu', 27, s.cpu); arc('rRam', 18, s.ram);
}

function renderTimer() {
  $('pgTimer').innerHTML = `<div class="timer">
    <div class="dial"><svg viewBox="0 0 100 100"><circle class="bg" cx="50" cy="50" r="44"/><circle class="fg" id="tFg" cx="50" cy="50" r="44" stroke-dasharray="276.5" stroke-dashoffset="276.5"/></svg><div class="val" id="tVal">0:00</div></div>
    <div><div class="presets">${[1, 5, 10, 15, 25, 45, 60].map((m) => `<button data-m="${m}">${m} min</button>`).join('')}</div>
      <div class="tactions"><button class="go" id="tGo">${Timer.running() ? (Timer.paused() ? 'Resume' : 'Pause') : 'Start 25 min'}</button><button id="tReset">Reset</button></div>
      <div class="hint">The timer keeps running in the notch as a live activity, and chimes when it's done.</div></div></div>`;
  document.querySelectorAll('.presets button').forEach((b) => (b.onclick = () => { Timer.start(+b.dataset.m); renderTimer(); renderCompact(false); updateTick(); }));
  $('tGo').onclick = () => { if (Timer.running()) Timer.toggle(); else Timer.start(25); renderTimer(); renderCompact(false); updateTick(); };
  $('tReset').onclick = () => { Timer.reset(); renderTimer(); renderCompact(false); updateTick(); };
  renderTimerDial();
}
function renderTimerDial() {
  const fg = $('tFg'), val = $('tVal'); if (!fg) return;
  fg.style.strokeDashoffset = (276.5 * (1 - (Timer.running() ? 1 - Timer.progress() : 0))).toFixed(1);
  val.textContent = Timer.running() ? Timer.label() : '0:00';
}

function expand() {
  clearTimeout(transientT);
  show('exp');
  renderExpanded();
  applySize(SZ.exp());
}
document.querySelectorAll('.tab').forEach((t) => (t.onclick = (e) => { e.stopPropagation(); tab = t.dataset.tab; renderExpanded(); applySize(SZ.exp()); }));
$('bGear').onclick = (e) => { e.stopPropagation(); Frost.send({ type: 'openPanel' }); };

// ------------------------------------------------ interaction
notch.addEventListener('mouseenter', () => {
  hover = true; clearTimeout(collapseT);
  if (view === 'compact') { renderCompact(); if (I('hoverExpand')) hoverT = setTimeout(expand, 380); }
});
notch.addEventListener('mouseleave', () => {
  hover = false; clearTimeout(hoverT);
  // Leaving always closes the dashboard, however it was opened (click or hover). A short grace keeps a
  // pointer that just grazes the edge from slamming it shut.
  if (view === 'exp') collapseT = setTimeout(back, Math.max(0.3, I('collapseDelay') ?? 0) * 1000);
  else if (view === 'compact') renderCompact();
});
notch.addEventListener('click', (e) => { if (view !== 'exp' && !e.target.closest('button')) expand(); });
notch.addEventListener('wheel', (e) => {
  if (view === 'exp') return;
  const v = Frost.state.volume; if (!v) return;
  const nv = Math.min(1, Math.max(0, v.level + (e.deltaY < 0 ? 0.02 : -0.02)));
  Frost.state.volume = { ...v, level: nv };
  Frost.send({ type: 'setVolume', level: nv });
  showVolume(nv, v.muted, true);
}, { passive: true });

// ------------------------------------------------ host events
Frost.on('settings', (s) => {
  updateGlow();
  if (applyStyle()) { reportBounds(Math.max(size[0], 640), Math.max(size[1], 120)); } // morph between looks
  if (view === 'compact') renderCompact();
  else if (view === 'exp') { renderExpanded(); applySize(SZ.exp()); }
});
// the album glow only exists while music plays (never a haze around an idle island)
function updateGlow() { const m = Frost.state.media; notch.classList.toggle('glow', !!I('glow') && !!m?.active && !!m.playing); }
Frost.on('media', (m) => {
  updateTint(m.active ? m.art : null);
  updateGlow();
  const changed = m.active && m.title && m.title !== lastTitle;
  if (m.active) lastTitle = m.title;
  if (changed && mediaSeen && I('media') && I('trackPopup') !== false && view !== 'exp')
    drop({ pic: m.art, picIcon: Icons.music, k: m.app || 'Now playing', t1: m.title, t2: m.artist, trail: I('visualizer') ? vizHTML(5, true) : '' });
  mediaSeen = true;
  // fetch lyrics ahead of time only when the notch will sing along; otherwise when the Lyrics tab opens
  const newLookup = (m.playing && I('lyricsNotch') !== false) || (view === 'exp' && tab === 'lyrics') ? ensureLyrics() : false;
  if (view === 'compact') renderCompact();
  else if (view === 'exp') {
    renderSources();
    if (tab === 'home') { renderHome(); applySize(SZ.exp()); }
    else if (tab === 'lyrics' && (newLookup || lyrPageKey !== trackKey(m))) renderLyrics();
  }
  updateLevelsWanted(); updateTick();
});
Frost.on('volume', (v) => { if (v.event && !v.ours) showVolume(v.level, v.muted); else if (view === 'exp') syncVolume(); });
Frost.on('levels', (m) => { lvl = Math.max(m.l, m.r); lvlAt = performance.now(); });
Frost.on('weather', () => { if (view === 'compact') renderCompact(); });
Frost.on('stats', updateStats);
Frost.on('notification', (n) => { if (I('notifications')) drop({ picIcon: Icons.bell, k: n.app || 'Notification', t1: n.title, t2: n.body, ms: 5000, w: 450 }); });
Frost.on('privacy', (p) => {
  const newly = (p.camera && !privacy.camera) ? ['Camera', p.cameraApp, 'cam'] : (p.mic && !privacy.mic) ? ['Microphone', p.micApp, 'mic'] : null;
  privacy = p;
  if (newly && I('privacy') && I('events')) drop({ picIcon: `<span class="dot ${newly[2]}" style="width:12px;height:12px"></span>`, k: 'Privacy', t1: `${newly[0]} in use`, t2: newly[1] || 'An app is using it', ms: 2600, w: 400 });
  else if (view === 'compact') renderCompact();
  if (view === 'exp') renderExpanded();
});
Frost.on('visibility', (m) => { visible = m.visible; updateLevelsWanted(); updateLyricTick(); });

Frost.on('demo', (d) => setTimeout(() => {
  greeted = true; clearTimeout(transientT);
  if (d.view === 'reset') { Timer.reset(); back(); return; }
  if (d.view === 'timer') { Timer.start(25); tab = 'timer'; }
  if (d.view.startsWith('lyrics')) tab = 'lyrics';
  if (d.view === 'lyrics-spotify') { const s = Frost.state.media?.sources?.find((x) => x.app === 'Spotify'); if (s) Frost.send({ type: 'selectSource', id: s.id }); }
  if (d.view === 'exp' || d.view === 'timer' || d.view.startsWith('lyrics')) expand(); else if (d.view === 'compact-timer') { Timer.start(25); renderCompact(); updateTick(); }
}, 1500));

let greeted = false;
Frost.on('settings', () => { if (!greeted) { greeted = true; renderCompact(); setTimeout(greet, 1200); } });

Frost.everyMinute(() => { if (view === 'compact' && compactMode === 'idle') renderCompact(false); });
if (Timer.running()) updateTick();
reportBounds(210, 32);
Frost.ready();
