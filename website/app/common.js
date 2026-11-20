// WinIsland shared runtime: host messaging, settings, formatting helpers, icons.
'use strict';

const Frost = (() => {
  const handlers = {};
  const state = { settings: null, media: null, volume: null, weather: null, stats: null };

  const on = (type, fn) => ((handlers[type] ||= []).push(fn), fn);
  const bridge = window.__frostBridge || window.chrome?.webview; // __frostBridge = demo mode (demo.js)
  const send = (msg) => { try { bridge?.postMessage(msg); } catch {} };
  const emit = (type, msg) => (handlers[type] || []).forEach((fn) => { try { fn(msg); } catch (e) { console.error(e); } });

  bridge?.addEventListener('message', (e) => {
    const m = e.data;
    if (!m || !m.type) return;
    if (m.type in state) state[m.type] = m.type === 'settings' ? m.settings : m;
    emit(m.type, m.type === 'settings' ? m.settings : m);
  });

  const ready = () => send({ type: 'ready' });
  const patch = (obj) => send({ type: 'settings', patch: obj });

  const pad = (n) => String(n).padStart(2, '0');
  const timeStr = (d = new Date()) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

  // Calls fn now and at every minute boundary (one timer per minute, no polling).
  function everyMinute(fn) {
    fn();
    const tick = () => { fn(); setTimeout(tick, 60000 - (Date.now() % 60000) + 30); };
    setTimeout(tick, 60000 - (Date.now() % 60000) + 30);
  }

  function christmas(now = new Date()) {
    let y = now.getFullYear(), xmas = new Date(y, 11, 24);
    const isEve = now.getMonth() === 11 && now.getDate() === 24;
    const isDay = now.getMonth() === 11 && (now.getDate() === 25 || now.getDate() === 26);
    if (now > new Date(y, 11, 27)) xmas = new Date(y + 1, 11, 24);
    return { days: Math.max(0, Math.floor((xmas - now) / 86400000)), isEve, isDay };
  }

  const WMO = {
    0: ['Clear', 'clear'], 1: ['Mostly clear', 'clear'], 2: ['Partly cloudy', 'partly'], 3: ['Overcast', 'cloud'],
    45: ['Fog', 'fog'], 48: ['Freezing fog', 'fog'], 51: ['Light drizzle', 'rain'], 53: ['Drizzle', 'rain'], 55: ['Heavy drizzle', 'rain'],
    56: ['Freezing drizzle', 'sleet'], 57: ['Freezing drizzle', 'sleet'], 61: ['Light rain', 'rain'], 63: ['Rain', 'rain'], 65: ['Heavy rain', 'rain'],
    66: ['Freezing rain', 'sleet'], 67: ['Freezing rain', 'sleet'], 71: ['Light snow', 'snow'], 73: ['Snow', 'snow'], 75: ['Heavy snow', 'snow'],
    77: ['Snow grains', 'snow'], 80: ['Showers', 'rain'], 81: ['Showers', 'rain'], 82: ['Heavy showers', 'rain'],
    85: ['Snow showers', 'snow'], 86: ['Heavy snow showers', 'snow'], 95: ['Thunderstorm', 'storm'], 96: ['Thunderstorm', 'storm'], 99: ['Thunderstorm', 'storm'],
  };
  const wmo = (c) => WMO[c] || ['—', 'cloud'];
  const clock = (s) => (!isFinite(s) || s < 0 ? '0:00' : `${Math.floor(s / 60)}:${pad(Math.floor(s % 60))}`);

  return { on, send, ready, patch, state, pad, timeStr, everyMinute, christmas, wmo, clock };
})();

// ---------------- icons (single stroke style, 24px grid) ----------------
const Icons = {
  play: '<svg viewBox="0 0 24 24"><path d="M8 5.5v13l10.5-6.5z" fill="currentColor"/></svg>',
  pause: '<svg viewBox="0 0 24 24"><rect x="6.5" y="5" width="4" height="14" rx="1.2" fill="currentColor"/><rect x="13.5" y="5" width="4" height="14" rx="1.2" fill="currentColor"/></svg>',
  next: '<svg viewBox="0 0 24 24"><path d="M5.5 6v12l9-6z" fill="currentColor"/><rect x="16" y="6" width="2.6" height="12" rx="1" fill="currentColor"/></svg>',
  prev: '<svg viewBox="0 0 24 24"><path d="M18.5 6v12l-9-6z" fill="currentColor"/><rect x="5.4" y="6" width="2.6" height="12" rx="1" fill="currentColor"/></svg>',
  vol: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" stroke="none"/><path d="M15.5 9a4.2 4.2 0 0 1 0 6"/><path d="M18.2 6.5a7.8 7.8 0 0 1 0 11"/></svg>',
  mute: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" stroke="none"/><path d="M16 9.5l5 5M21 9.5l-5 5"/></svg>',
  gear: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  cpu: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><rect x="6" y="6" width="12" height="12" rx="2.5"/><rect x="9.5" y="9.5" width="5" height="5" rx="1"/><path d="M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3"/></svg>',
  bell: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M6 16V11a6 6 0 1 1 12 0v5l1.5 2h-15z"/><path d="M10 20.5a2.2 2.2 0 0 0 4 0"/></svg>',
  tree: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M12 2.8 15 7h-1.8l3.3 4.4h-2l3.8 5H5.7l3.8-5h-2L10.8 7H9z"/><path d="M12 16.4V21"/></svg>',
  music: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M9 18V6l11-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="17.5" cy="16" r="2.5"/></svg>',
  home: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M4 11 12 4l8 7v8.5a1 1 0 0 1-1 1h-4.5V15h-5v5.5H5a1 1 0 0 1-1-1z"/></svg>',
  clock: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/></svg>',
  mic: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><rect x="9" y="3.5" width="6" height="11" rx="3"/><path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v2.5"/></svg>',
  // weather
  clear: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/></svg>',
  clearNight: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/></svg>',
  partly: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M8.5 4.5v1.2M3.6 9.4h1.2M5 6l.9.9M12 6l-.9.9"/><path d="M5.6 12.4a3.6 3.6 0 1 1 6.3-3"/><path d="M8 19.5h9a3.5 3.5 0 0 0 .3-7 5 5 0 0 0-9.6 1.4A2.8 2.8 0 0 0 8 19.5z"/></svg>',
  cloud: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M7 18.5h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 18.5z"/></svg>',
  fog: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4 9h16M3 13h14M6 17h15"/></svg>',
  rain: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M7 15h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 15z"/><path d="M9 18l-1 2.5M13 18l-1 2.5M17 18l-1 2.5"/></svg>',
  sleet: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M7 15h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 15z"/><path d="M9 18l-1 2.5M16 18.5v.1M12.5 20v.1"/></svg>',
  snow: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M7 14.5h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 14.5z"/><path d="M8.5 18v.1M12 19.5v.1M15.5 18v.1M10 21.5v.1M14 21.5v.1"/></svg>',
  storm: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M7 15h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 15z"/><path d="M12.5 15.5 10.5 19h3l-2 3.5"/></svg>',
};
const weatherIcon = (code, isDay = true) => { const k = Frost.wmo(code)[1]; return Icons[k === 'clear' && !isDay ? 'clearNight' : k] || Icons.cloud; };
