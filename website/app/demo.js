// WinIsland demo bridge. Only active when the page URL contains "demo" (e.g. island.html?demo=tour&style=ios).
// It stands in for the Windows host with made-up tracks, generated album art and original lyrics, so the real
// island and settings UI can run in a browser: the settings preview, the website and the promo-video renders.
'use strict';
(() => {
  const qs = new URLSearchParams(location.search);
  if (!qs.has('demo')) return;
  const scene = qs.get('demo') || 'tour';
  let style = qs.get('style') === 'ios' ? 'ios' : 'macos';
  document.documentElement.classList.add('demo');
  try { localStorage.removeItem('frost.timer'); } catch {} // demos never inherit a timer from another demo page
  if (qs.has('transparent')) document.documentElement.style.background = 'transparent';
  if (qs.has('bg')) document.documentElement.style.background = qs.get('bg') || 'radial-gradient(120% 90% at 50% 0%, #2b3a67 0%, #141a2e 55%, #0a0d17 100%)';

  // ---------------------------------------------------------------- generated album art (seeded, deterministic)
  function rng(seed) { return () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646; }
  function art(seed, [a, b, c, d]) {
    const cv = document.createElement('canvas'); cv.width = cv.height = 360;
    const g = cv.getContext('2d'), r = rng(seed);
    const bg = g.createLinearGradient(0, 0, 360, 360); bg.addColorStop(0, a); bg.addColorStop(1, b);
    g.fillStyle = bg; g.fillRect(0, 0, 360, 360);
    for (let i = 0; i < 6; i++) {
      const x = r() * 360, y = r() * 220, rad = 90 + r() * 160, gr = g.createRadialGradient(x, y, 0, x, y, rad);
      gr.addColorStop(0, i % 2 ? c : d); gr.addColorStop(1, 'transparent');
      g.globalAlpha = 0.55; g.fillStyle = gr; g.fillRect(0, 0, 360, 360);
    }
    // aurora ribbons
    g.globalCompositeOperation = 'screen';
    for (let k = 0; k < 3; k++) {
      g.beginPath(); g.moveTo(-20, 120 + k * 30);
      g.bezierCurveTo(100, 40 + r() * 80, 220, 200 - r() * 90, 380, 90 + k * 25);
      g.strokeStyle = k === 1 ? c : d; g.lineWidth = 18 - k * 4; g.globalAlpha = 0.5; g.shadowColor = k === 1 ? c : d; g.shadowBlur = 40; g.stroke();
    }
    g.globalCompositeOperation = 'source-over'; g.shadowBlur = 0; g.globalAlpha = 1;
    // mountains
    g.fillStyle = 'rgba(5,8,20,.82)'; g.beginPath(); g.moveTo(0, 360); g.lineTo(0, 270);
    for (let x = 0; x <= 360; x += 30) g.lineTo(x, 250 + Math.sin(x * 0.03 + seed) * 22 - (x > 120 && x < 240 ? 40 * Math.sin((x - 120) / 120 * Math.PI) : 0));
    g.lineTo(360, 360); g.closePath(); g.fill();
    // grain
    const im = g.getImageData(0, 0, 360, 360), px = im.data;
    for (let i = 0; i < px.length; i += 4) { const n = (r() - 0.5) * 16; px[i] += n; px[i + 1] += n; px[i + 2] += n; }
    g.putImageData(im, 0, 0);
    return cv.toDataURL('image/jpeg', 0.9);
  }

  // ---------------------------------------------------------------- catalogue (fictional)
  const LRC1 = `[00:00.00]
[00:09.60]Cold light over the water
[00:13.80]Silver lines across the sky
[00:18.10]Hold on, the night is getting brighter
[00:22.70]Every star is falling into time
[00:27.30]We were running through the snowfall
[00:31.60]Chasing colours we can't name
[00:36.00]Northern lights, northern lights
[00:40.20]Burning softly, calling out my name
[00:44.90]Stay with me until the morning
[00:49.20]Let the whole world fade to white
[00:53.60]Northern lights, northern lights
[00:57.90]You and me beneath the frozen sky
[01:02.40]
[01:10.80]Footprints in the quiet
[01:15.00]Breathing clouds into the cold
[01:19.40]Every heartbeat lighting up the silence
[01:23.90]Like a story that we've never told
[01:28.50]Northern lights, northern lights
[01:32.80]Burning softly, calling out my name`;
  const LRC2 = `[00:00.00]
[00:06.20]Glass harbor, glass harbor
[00:10.40]Boats asleep in frozen light
[00:14.80]I can hear the city humming
[00:19.00]Through the windows of the night
[00:23.60]Hold the lantern a little closer
[00:27.90]Let it melt the winter through
[00:32.30]Glass harbor, glass harbor
[00:36.50]Every road leads back to you`;
  const tracks = [
    { title: 'Northern Lights', artist: 'Aurora Fields', album: 'Polar Hours', duration: 214, app: 'Spotify', lrc: LRC1, art: art(7, ['#0b1b4a', '#3a0f5c', '#3ee6c1', '#7f6bff']) },
    { title: 'Glass Harbor', artist: 'Lumen', album: 'Glass Harbor', duration: 198, app: 'Spotify', lrc: LRC2, art: art(23, ['#3b0d2e', '#f26b3a', '#ffb36b', '#ff4f8b']) },
    { title: 'Snowfall at Night (Live Session)', artist: 'Winter Room', album: '', duration: 263, app: 'Chrome', browser: true, lrc: '', art: art(41, ['#0e2233', '#2d6a8f', '#bfe6ff', '#6fb7ff']) },
  ];
  let ti = 0, playing = scene !== 'idle', position = 34.5, posStamp = Date.now(), selected = 0;

  // ---------------------------------------------------------------- fake host
  const listeners = [];
  const post = (m) => setTimeout(() => listeners.forEach((fn) => fn({ data: JSON.parse(JSON.stringify(m)) })), 0);
  const settings = {
    island: { enabled: true, time: true, weather: true, media: true, volume: true, system: true, notifications: true, events: false, collapseDelay: 0,
      hideInFullscreen: true, glow: true, visualizer: true, trackPopup: true, lyrics: true, lyricsNotch: true, privacy: true, hoverExpand: false, idleInfo: true, style },
    weather: { city: 'Oslo, Norway', lat: 59.91, lon: 10.75, units: 'c' },
    general: { autostart: true, hotkey: true },
  };
  const merge = (t, s) => { for (const k in s) { if (s[k] && typeof s[k] === 'object' && !Array.isArray(s[k]) && t[k]) merge(t[k], s[k]); else t[k] = s[k]; } };
  const pos = () => position + (playing ? (Date.now() - posStamp) / 1000 : 0);
  function media() {
    const t = tracks[ti];
    return { type: 'media', active: true, title: t.title, artist: t.artist, album: t.album, app: t.app, browser: !!t.browser, playing,
      canPrev: true, canNext: true, position: pos(), duration: t.duration, posStamp: Date.now(), art: t.art,
      sources: [{ i: 0, app: 'Spotify', t: tracks[ti < 2 ? ti : 0] }, { i: 2, app: 'Chrome', t: tracks[2] }].map((s) => ({
        id: 'demo#' + s.i, app: s.app, browser: s.i === 2, title: s.t.title, artist: s.t.artist, playing: playing && (ti === s.i || (s.i === 0 && ti < 2)), selected: s.i === 2 ? ti === 2 : ti < 2 })) };
  }
  const weather = { type: 'weather', city: 'Oslo, Norway', units: 'C', temp: -4, code: 73, isDay: true, hi: -2, lo: -9 };
  let volume = { level: 0.62, muted: false };
  let levelT = 0, statT = 0;
  function handle(m) {
    switch (m.type) {
      case 'ready':
        if (typeof Frost !== 'undefined') Frost.timeStr = () => qs.get('time') || '9:41';
        post({ type: 'settings', settings });
        post(media()); post({ type: 'volume', ...volume, event: false }); post(weather);
        post({ type: 'stats', cpu: 9.4, ram: 46 });
        post({ type: 'privacy', camera: false, mic: false });
        post({ type: 'diagnostics', totalMB: 108, procs: 5, root: 'C:\\Users\\you\\AppData\\Local\\Programs\\WinIsland', notifications: 'ok', windhawkIsland: false, fluentFlyout: false });
        if (window.parent !== window) window.parent.postMessage({ frostDemo: 'ready' }, '*');
        setTimeout(() => runScene(scene), 400);
        break;
      case 'media':
        if (m.action === 'toggle') { position = pos(); posStamp = Date.now(); playing = !playing; }
        else { ti = ti === 2 ? 2 : (ti + (m.action === 'next' ? 1 : 1)) % 2; position = 0; posStamp = Date.now(); playing = true; }
        post(media()); break;
      case 'seek': position = m.position; posStamp = Date.now(); post(media()); break;
      case 'selectSource': ti = m.id === 'demo#2' ? 2 : 0; position = ti === 2 ? 72 : 34.5; posStamp = Date.now(); post(media()); break;
      case 'levels':
        clearInterval(levelT);
        if (m.on) { let ph = 0; levelT = setInterval(() => { ph += 0.33; const v = 0.35 + 0.3 * Math.abs(Math.sin(ph)) + 0.25 * Math.abs(Math.sin(ph * 2.7 + 1)); post({ type: 'levels', l: v, r: v * 0.95 }); }, 66); }
        break;
      case 'subscribe':
        clearInterval(statT);
        if (m.stats) { let k = 0; statT = setInterval(() => { k++; post({ type: 'stats', cpu: 8 + 6 * Math.abs(Math.sin(k / 3)), ram: 46 + (k % 3) }); }, 1000); post({ type: 'stats', cpu: 11.2, ram: 46 }); }
        break;
      case 'setVolume': volume.level = m.level; volume.muted = false; post({ type: 'volume', ...volume, event: false }); break;
      case 'toggleMute': volume.muted = !volume.muted; post({ type: 'volume', ...volume, event: true, ours: true }); break;
      case 'lyrics': {
        const t = tracks.find((x) => x.title === m.title) || tracks[ti];
        setTimeout(() => post({ type: 'lyrics', req: m.req, found: !!t.lrc, synced: t.lrc || null, plain: null, instrumental: false }), 350);
        break;
      }
      case 'settings': merge(settings, m.patch); if (m.patch.island?.style) style = m.patch.island.style; post({ type: 'settings', settings }); break;
      case 'diagnostics': post({ type: 'diagnostics', totalMB: 104 + Math.round(Math.random() * 8), procs: 5, root: 'C:\\Users\\you\\AppData\\Local\\Programs\\WinIsland', notifications: 'ok' }); break;
    }
  }
  window.__frostBridge = {
    postMessage: (m) => { try { handle(m); } catch (e) { console.error(e); } if (window.parent !== window && m.type !== 'bounds') window.parent.postMessage({ frostDemoOut: m }, '*'); },
    addEventListener: (type, fn) => { if (type === 'message') listeners.push(fn); },
  };

  // ---------------------------------------------------------------- scenes (drive the real island code)
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  let runId = 0;
  async function runScene(name) {
    const id = ++runId, alive = () => id === runId;
    const isIsland = !!document.getElementById('notch');
    if (!isIsland) return;
    switch (name) {
      case 'idle': playing = false; post(media()); break;
      case 'music': break;
      case 'exp': await wait(300); expand(); break;
      case 'lyrics': await wait(300); tab = 'lyrics'; expand(); break;
      case 'timer': await wait(300); Timer.start(25); tab = 'timer'; expand(); break;
      case 'hud': await wait(300); showVolume(0.62, false, true); break;
      case 'drop': await wait(300); drop({ pic: tracks[ti].art, k: 'Spotify', t1: tracks[ti].title, t2: tracks[ti].artist, trail: vizHTML(5, true), ms: 600000 }); break;
      case 'tour':
        while (alive()) {
          playing = false; post(media()); back(); await wait(2600); if (!alive()) return;
          playing = true; position = 34.5; posStamp = Date.now(); ti = ti === 0 ? 1 : 0; post(media()); await wait(4200); if (!alive()) return; // drop-down, then music live activity
          await wait(5200); if (!alive()) return; // sing along in the notch
          showVolume(0.7, false, true); await wait(1900); if (!alive()) return;
          tab = 'home'; expand(); await wait(5200); if (!alive()) return;
          tab = 'lyrics'; renderExpanded(); applySize(SZ.exp()); await wait(6500); if (!alive()) return;
          back(); await wait(1500);
        }
        break;
    }
  }
  window.FrostDemo = {
    scene: (n) => runScene(n),
    style: (s) => { style = s; settings.island.style = s; post({ type: 'settings', settings }); },
    stop: () => { runId++; },
    tracks,
  };
  window.addEventListener('message', (e) => {
    const d = e.data || {};
    if (d.frost === 'style') FrostDemo.style(d.value);
    if (d.frost === 'scene') FrostDemo.scene(d.value);
    if (d.frost === 'patch') handle({ type: 'settings', patch: d.value });
  });
})();
