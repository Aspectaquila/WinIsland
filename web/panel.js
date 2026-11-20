// WinIsland settings
'use strict';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];

Object.assign(Icons, {
  island: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M5 4h14v4.5a3.5 3.5 0 0 1-3.5 3.5h-7A3.5 3.5 0 0 1 5 8.5z" fill="currentColor" fill-opacity=".25"/><path d="M8 17h8" stroke-linecap="round" opacity=".6"/></svg>',
  sparkle: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M12 3c.6 4.2 2.8 6.4 7 7-4.2.6-6.4 2.8-7 7-.6-4.2-2.8-6.4-7-7 4.2-.6 6.4-2.8 7-7z"/></svg>',
  fullscreen: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/></svg>',
  wave: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/></svg>',
  camera: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><rect x="3" y="6.5" width="13" height="11" rx="2.5"/><path d="m16 10.5 5-3v9l-5-3z"/></svg>',
  pin: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M12 21s-6.5-6-6.5-11a6.5 6.5 0 0 1 13 0c0 5-6.5 11-6.5 11z"/><circle cx="12" cy="10" r="2.3"/></svg>',
  thermo: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M10 14.5V5a2 2 0 1 1 4 0v9.5a4 4 0 1 1-4 0z"/></svg>',
  power: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M12 3.5v8"/><path d="M7 6.5a7.5 7.5 0 1 0 10 0"/></svg>',
  key: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><circle cx="8" cy="14" r="4"/><path d="m11 11 8.5-8.5M16 6l2.5 2.5M14 8l2 2"/></svg>',
  restore: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12a8 8 0 1 0 2.4-5.7"/><path d="M4 4.5v4h4"/></svg>',
  exit: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M14 4.5H6.5v15H14"/><path d="M10.5 12H20M16.5 8.5 20 12l-3.5 3.5"/></svg>',
  info: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5M12 7.8v.1"/></svg>',
  warn: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4 2.8 19.5h18.4z"/><path d="M12 10v4.5M12 17.3v.1"/></svg>',
  laptop: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><rect x="4" y="5" width="16" height="11" rx="2"/><path d="M10 5h4v1.3a.7.7 0 0 1-.7.7h-2.6a.7.7 0 0 1-.7-.7z" fill="currentColor"/><path d="M2 19h20" stroke-linecap="round"/></svg>',
  phone: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"><rect x="6.5" y="2.5" width="11" height="19" rx="3"/><rect x="10" y="4.6" width="4" height="1.6" rx=".8" fill="currentColor" stroke="none"/></svg>',
  pointer: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M6 3.5 18.5 12l-5.6 1.2 3.3 6.1-2.4 1.3-3.3-6.1L6 18.2z"/></svg>',
  drop: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="3.5" width="16" height="9" rx="4.5"/><path d="M12 15.5v5M9 18l3 2.5 3-2.5"/></svg>',
});
$$('[data-icon]').forEach((el) => (el.outerHTML = Icons[el.dataset.icon] || ''));

// ---------------------------------------------------------------- navigation
function showPage(name) {
  $$('.nav').forEach((n) => n.classList.toggle('on', n.dataset.page === name));
  $$('.page').forEach((s) => s.classList.toggle('on', s.id === 'p-' + name));
  $('.main').scrollTop = 0;
  preview(name === 'overview');
}
$$('.nav').forEach((b) => b.addEventListener('click', () => showPage(b.dataset.page)));

// ---------------------------------------------------------------- macOS / iOS look
const LOOKS = {
  macos: { eyebrow: 'macOS look', title: 'A notch that lives on your desktop.',
    lead: 'Flush with the top edge of your screen, with soft shoulders, like the MacBook notch. Music, lyrics, timers and alerts unfold right out of it.' },
  ios: { eyebrow: 'iOS look', title: 'The Dynamic Island, floating free.',
    lead: 'A capsule that hovers just below the top edge and morphs into live activities, like on iPhone. Inside: sky-coloured weather, activity rings and iOS-style controls.' },
};
let look = null;
function applyLook(next, animate) {
  if (next === look) return;
  // The settings window keeps one design; the switch changes the island (and the preview shows it morph).
  look = next;
  $('#vswitch').classList.toggle('ios', next === 'ios');
  $('#stage').classList.toggle('ios', next === 'ios');
  $$('#vswitch button').forEach((b) => b.classList.toggle('on', b.dataset.style === next));
  const txt = $('.hero-txt');
  const put = () => { $('#heroEyebrow').textContent = LOOKS[next].eyebrow; $('#heroTitle').textContent = LOOKS[next].title; $('#heroLead').textContent = LOOKS[next].lead; txt.classList.remove('swap'); };
  if (animate) { txt.classList.add('swap'); setTimeout(put, 220); } else put();
  framePost({ frost: 'style', value: next });
}
$$('#vswitch button').forEach((b) => b.addEventListener('click', () => {
  if (b.dataset.style === look) return;
  applyLook(b.dataset.style, true);
  set('island.style', b.dataset.style);
}));

// live preview: the real island page running on the demo bridge, only while the Overview is visible
const frame = $('#preview');
function preview(on) {
  if (on && !frame.src) frame.src = `island.html?demo=tour&transparent&style=${look || 'macos'}`;
  else if (!on && frame.src) frame.removeAttribute('src');
}
function framePost(m) { try { frame.contentWindow?.postMessage(m, '*'); } catch {} }
window.addEventListener('message', (e) => {
  if (e.source === frame.contentWindow && e.data?.frostDemo === 'ready') framePost({ frost: 'patch', value: { island: { ...Frost.state.settings?.island, events: false, style: look } } });
});

// ---------------------------------------------------------------- settings binding
const get = (k) => k.split('.').reduce((o, p) => o?.[p], Frost.state.settings);
const set = (k, v) => Frost.patch(k.split('.').reduceRight((acc, p) => ({ [p]: acc }), v));
const timers = new Map();
const setSoon = (k, v) => { clearTimeout(timers.get(k)); timers.set(k, setTimeout(() => set(k, v), 140)); };

$$('input[type=checkbox][data-key]').forEach((el) => el.addEventListener('change', () => set(el.dataset.key, el.checked)));
$$('input[type=range][data-key]').forEach((el) => el.addEventListener('input', () => { paintRange(el); setSoon(el.dataset.key, +el.value); }));
$$('.seg[data-key]').forEach((seg) => seg.addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) set(seg.dataset.key, b.dataset.v); }));

function paintRange(el) {
  el.style.setProperty('--v', ((el.value - el.min) / (el.max - el.min)) * 100 + '%');
  const out = el.parentElement.querySelector('output');
  if (out) out.textContent = el.dataset.key === 'island.collapseDelay' && +el.value === 0 ? 'Instantly' : el.value + (out.dataset.unit || '');
}

function sync(S) {
  $$('input[type=checkbox][data-key]').forEach((el) => (el.checked = !!get(el.dataset.key)));
  $$('input[type=range][data-key]').forEach((el) => { if (document.activeElement !== el) { el.value = get(el.dataset.key) ?? el.value; paintRange(el); } });
  $$('.seg[data-key]').forEach((seg) => $$('button', seg).forEach((b, i) => { const on = String(get(seg.dataset.key)) === b.dataset.v; b.classList.toggle('on', on); if (on) seg.dataset.on = i; }));
  $$('[data-req]').forEach((r) => r.classList.toggle('disabled', !get(r.dataset.req)));
  applyLook(S.island.style === 'ios' ? 'ios' : 'macos', look !== null);
  framePost({ frost: 'patch', value: { island: { ...S.island, events: false } } });
  $('#cityNow').textContent = S.weather.city ? `${S.weather.city} (${(+S.weather.lat).toFixed(2)}, ${(+S.weather.lon).toFixed(2)})` : 'Not set';
}

// ---------------------------------------------------------------- actions
let cityPending = false;
$('#bCity').onclick = () => { const v = $('#cityIn').value.trim(); if (v) { cityPending = true; Frost.send({ type: 'setCity', city: v }); toast('Looking up ' + v + '…'); } };
$('#cityIn').addEventListener('keydown', (e) => { if (e.key === 'Enter') $('#bCity').click(); });
$('#bLocate').onclick = () => { cityPending = true; Frost.send({ type: 'locate' }); toast('Asking Windows for your location…'); };
$('#bReset').onclick = () => confirmDlg('Reset all settings?', 'Every option returns to its default, and your weather city is cleared.', 'Reset', () => Frost.send({ type: 'resetSettings' }));
$('#bQuit').onclick = () => confirmDlg('Exit WinIsland?', 'The island closes until your next sign-in (if autostart is on) or until you start it again.', 'Exit', () => Frost.send({ type: 'quit' }));
$('#bUninstall').onclick = () => confirmDlg('Remove from startup?', 'Autostart is turned off and WinIsland exits. The files stay in place. Run Uninstall.ps1 to delete them.', 'Remove & exit', () => Frost.send({ type: 'uninstall' }));

function confirmDlg(title, text, yes, fn) {
  $('#dlgT').textContent = title; $('#dlgP').textContent = text; $('#dlgYes').textContent = yes;
  $('#dlg').classList.add('on');
  $('#dlgNo').onclick = () => $('#dlg').classList.remove('on');
  $('#dlgYes').onclick = () => { $('#dlg').classList.remove('on'); fn(); };
}
let toastT = 0;
function toast(t) { const el = $('#toast'); el.textContent = t; el.classList.add('on'); clearTimeout(toastT); toastT = setTimeout(() => el.classList.remove('on'), 2600); }

// ---------------------------------------------------------------- host messages
Frost.on('settings', sync);
Frost.on('toast', (m) => toast(m.text));
Frost.on('weather', (w) => { if (cityPending && w.city && !w.needsCity) { cityPending = false; toast(`Weather: ${w.temp}°${w.units} in ${w.city.split(',')[0]}`); } });
Frost.on('diagnostics', (d) => {
  $('#sideMem').textContent = `Running · ${d.totalMB} MB`;
  $('#stMem').textContent = `${d.totalMB} MB`; $('#stProcs').textContent = d.procs;
  const n = [];
  if (d.windhawkIsland && Frost.state.settings?.island?.enabled)
    n.push(`<div class="notice">${Icons.warn}<div><b>Two islands are running.</b> Your Windhawk mod <i>Dynamic Island for Windows</i> sits in the same spot. Open Windhawk → My Mods and switch it off.</div></div>`);
  if (d.fluentFlyout && Frost.state.settings?.island?.volume)
    n.push(`<div class="notice info">${Icons.info}<div>FluentFlyout also shows volume pop-ups. To avoid doubles, turn off <b>Volume changes</b> below or in FluentFlyout.</div></div>`);
  $('#notices').innerHTML = n.join('');
  $('#notifStatus').textContent = d.notifications === 'ok' ? 'Windows toast notifications. Access granted.'
    : 'Unavailable: Windows did not grant access to notifications (it usually reserves this for Store apps).';
  $('#files').innerHTML = `<span>Install folder</span><code>${d.root}</code><span>Settings</span><code>${d.data || d.root}\\settings.json</code>
    <span>Uninstall</span><code>Windows Settings → Apps → Installed apps → WinIsland</code>`;
});

setInterval(() => Frost.send({ type: 'diagnostics' }), 5000);
const startPage = new URLSearchParams(location.search).get('page');
Frost.ready();
showPage(startPage && $('#p-' + startPage) ? startPage : 'overview');
