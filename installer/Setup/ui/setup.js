// WinIsland Setup wizard. Talks to the host (Ui.cs) with JSON strings; runs a simulated host when opened in a browser.
'use strict';
const $ = (id) => document.getElementById(id);
const qs = new URLSearchParams(location.search);
const host = qs.has('demo') ? null : window.chrome?.webview; // ?demo = simulated host for previews
const send = (o) => (host ? host.postMessage(JSON.stringify(o)) : fake(o));

const INSTALL = ['welcome', 'look', 'location', 'options', 'install', 'done'];
const UNINSTALL = ['confirm', 'install', 'done'];
const LABELS = { welcome: 'Welcome', look: 'Look', location: 'Location', options: 'Preferences', install: 'Install', done: 'Finish', confirm: 'Confirm' };
let mode = qs.get('mode') === 'uninstall' ? 'uninstall' : 'install';
let flow = mode === 'install' ? INSTALL : UNINSTALL;
let idx = 0, state = { style: 'notch', dir: '', dirError: null, sizeMB: 0, freeMB: 0, busy: false, failed: false };

function renderSteps() {
  $('steps').innerHTML = flow.map((s, i) => `<li class="${i < idx ? 'done' : i === idx ? 'on' : ''}"><span class="n">${i + 1}</span>${LABELS[s]}</li>`).join('');
}
function go(i, back) {
  idx = Math.max(0, Math.min(flow.length - 1, i));
  const step = flow[idx];
  document.querySelectorAll('.page').forEach((p) => { p.classList.toggle('on', p.dataset.step === step); p.classList.toggle('back', !!back); });
  renderSteps(); renderFooter();
  if (step === 'install') start();
}
function renderFooter() {
  const step = flow[idx], next = $('bNext'), back = $('bBack'), cancel = $('bCancel');
  back.hidden = idx === 0 || step === 'install' || step === 'done';
  cancel.hidden = step === 'install' || step === 'done';
  next.className = 'btn primary';
  next.disabled = false;
  if (step === 'options') next.textContent = 'Install';
  else if (step === 'confirm') { next.textContent = 'Uninstall'; next.className = 'btn danger'; }
  else if (step === 'install') { next.textContent = mode === 'install' ? 'Installing…' : 'Removing…'; next.disabled = true; }
  else if (step === 'done') next.textContent = state.failed ? 'Close' : 'Finish';
  else next.textContent = 'Continue';
  if (step === 'location') next.disabled = !!state.dirError || !state.dir;
}

$('bNext').onclick = () => {
  const step = flow[idx];
  if (step === 'done') { send({ cmd: 'finish', launch: mode === 'install' && !state.failed && $('oLaunch').checked }); return; }
  go(idx + 1);
};
$('bBack').onclick = () => go(idx - 1, true);
$('bCancel').onclick = () => send({ cmd: 'cancel' });

// look
document.querySelectorAll('.look').forEach((b) => (b.onclick = () => {
  state.style = b.dataset.style;
  document.querySelectorAll('.look').forEach((x) => x.classList.toggle('on', x === b));
}));

// location
let checkT = 0;
$('dir').addEventListener('input', () => { state.dir = $('dir').value.trim(); clearTimeout(checkT); checkT = setTimeout(() => send({ cmd: 'checkDir', dir: state.dir }), 250); });
$('browse').onclick = () => send({ cmd: 'browse', dir: state.dir });
function fmtMB(mb) { return mb < 0 ? '–' : mb >= 1024 ? (mb / 1024).toFixed(mb >= 10240 ? 0 : 1) + ' GB' : Math.round(mb) + ' MB'; }
function renderSpace() {
  $('need').textContent = fmtMB(state.sizeMB); $('free').textContent = fmtMB(state.freeMB);
  $('meter').style.width = state.freeMB > 0 ? Math.max(2, Math.min(100, (state.sizeMB / state.freeMB) * 100 * 40)) + '%' : '0';
  let err = state.dirError;
  if (!err && state.freeMB >= 0 && state.freeMB < state.sizeMB + 20) err = 'There isn\'t enough free space on that drive.';
  state.dirError = err;
  $('dirErr').textContent = err || '';
  renderFooter();
}

// install / uninstall
function start() {
  if (state.busy) return;
  state.busy = true; state.failed = false;
  $('progTitle').textContent = mode === 'install' ? 'Installing WinIsland' : 'Removing WinIsland';
  $('progLead').textContent = mode === 'install' ? `Copying files to ${state.dir}` : 'Cleaning up everything WinIsland added.';
  setProgress(0, 'Preparing…');
  if (qs.get('demo') === 'progress') { setProgress(+(qs.get('pct') || 64), 'bin\\WinIsland.exe'); return; }
  if (mode === 'install') send({ cmd: 'install', dir: state.dir, style: state.style, autostart: $('oAutostart').checked, startMenu: $('oStart').checked,
    desktop: $('oDesktop').checked, lyrics: $('oLyrics').checked, launch: true });
  else send({ cmd: 'uninstall', keepSettings: $('oKeep').checked });
}
function setProgress(pct, file) {
  $('pfill').style.width = `calc(${Math.max(0, Math.min(100, pct))}% - 8px + ${36 * (1 - pct / 100)}px)`;
  $('ppct').textContent = Math.round(pct) + '%';
  if (file) $('pfile').textContent = file;
}

function onHost(m) {
  switch (m.type) {
    case 'init':
      mode = m.mode; flow = mode === 'install' ? INSTALL : UNINSTALL;
      Object.assign(state, { dir: m.dir, sizeMB: m.sizeMB, freeMB: m.freeMB, dirError: m.dirError });
      $('dir').value = m.dir;
      if (mode === 'uninstall') $('fine').textContent = 'Your music apps and everything else stay exactly as they are.';
      if (m.existing && mode === 'install') {
        $('upgradeNote').hidden = false;
        $('upgradeNote').textContent = `WinIsland ${m.existingVersion || ''} is already installed. Setup will update it in place and keep your settings.`;
      }
      renderSpace();
      if (['ios', 'island'].includes(qs.get('style'))) document.querySelector('.look[data-style="island"]').click();
      go(+qs.get('step') || 0);
      break;
    case 'dir':
      if (!m.quiet) { state.dir = m.dir; $('dir').value = m.dir; }
      state.freeMB = m.freeMB; state.dirError = m.dirError; renderSpace(); break;
    case 'progress': setProgress(m.pct, m.file); break;
    case 'done':
      state.busy = false; setProgress(100, 'Done');
      $('doneTitle').textContent = mode === 'install' ? 'WinIsland is ready' : 'WinIsland was removed';
      $('doneLead').textContent = mode === 'install' ? 'Look at the top of your screen. Click the island to open it, and press Ctrl + Alt + F for settings.' : 'Thanks for trying it. Everything WinIsland added to your PC is gone.';
      $('launchRow').hidden = mode !== 'install';
      setTimeout(() => go(flow.indexOf('done')), 450);
      break;
    case 'error':
      state.busy = false; state.failed = true;
      document.querySelector('.okmark').classList.add('fail');
      document.querySelector('.okmark path').setAttribute('d', 'M18 18l16 16M34 18 18 34');
      $('doneTitle').textContent = 'Something went wrong';
      $('doneLead').textContent = m.message || 'Setup could not finish.';
      $('launchRow').hidden = true;
      go(flow.indexOf('done'));
      break;
  }
}
if (host) host.addEventListener('message', (e) => { try { onHost(JSON.parse(e.data)); } catch (err) { console.error(err); } });

// ---------------------------------------------------------------- simulated host (browser preview / screenshots)
function fake(o) {
  const reply = (m) => setTimeout(() => onHost(m), 30);
  if (o.cmd === 'init') reply({ type: 'init', mode, version: '2.0.0', dir: 'C:\\Users\\you\\AppData\\Local\\Programs\\WinIsland', sizeMB: 158, freeMB: 412000, dirError: null, existing: false });
  if (o.cmd === 'checkDir') reply({ type: 'dir', dir: o.dir, freeMB: 412000, dirError: null, quiet: true });
  if (o.cmd === 'install' || o.cmd === 'uninstall') { let p = 0; const t = setInterval(() => { p += 4; onHost({ type: 'progress', pct: p, file: 'bin\\WinIsland.exe' }); if (p >= 100) { clearInterval(t); onHost({ type: 'done' }); } }, 60); }
}

// test hook (FROST_SETUP_AUTOTEST): walks the real wizard end to end, exactly as clicking would
window.autotest = async (dir, style) => {
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  await wait(600);
  $('bNext').click(); await wait(500);                                            // welcome → look
  document.querySelector(`.look[data-style="${style}"]`).click(); await wait(300);
  $('bNext').click(); await wait(500);                                            // look → location
  $('dir').value = dir; $('dir').dispatchEvent(new Event('input')); await wait(900);
  $('bNext').click(); await wait(500);                                            // location → options
  $('oAutostart').checked = false; $('oStart').checked = false;
  $('bNext').click();                                                             // install
  while (flow[idx] !== 'done') await wait(200);
  await wait(800);
  $('oLaunch').checked = true; $('bNext').click();                                // finish + launch
};

renderSteps();
send({ cmd: 'init' });
