// WinIsland website: island captures (crossfading tour), look switching, reveals, setup carousel.
'use strict';
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const root = document.documentElement;
const reducedQuery = matchMedia('(prefers-reduced-motion: reduce)');
let reduced = reducedQuery.matches;
reducedQuery.addEventListener?.('change', (e) => { reduced = e.matches; });
let look = 'notch';

// ------------------------------------------------------------ nav
const nav = document.getElementById('nav');
const onScroll = () => nav.classList.toggle('scrolled', scrollY > 8);
addEventListener('scroll', onScroll, { passive: true }); onScroll();

// ------------------------------------------------------------ scroll reveal
const revealIO = new IntersectionObserver((entries) => entries.forEach((e) => {
  if (!e.isIntersecting) return;
  e.target.classList.add('in');
  revealIO.unobserve(e.target);
}), { threshold: 0.15, rootMargin: '0px 0px -48px 0px' });
$$('.reveal').forEach((el) => (reduced ? el.classList.add('in') : revealIO.observe(el)));

// ------------------------------------------------------------ island captures
// Real captures of the native app (assets/shots, made with Windows.Graphics.Capture, transparent).
// Each host shows one state, or with data-tour cycles through a sequence like the app does.
const SHOT = { scale: 2, pad: { notch: 0, island: 48 } }; // captures are 2x; island shots keep 48 px of transparent padding on top
const TOP = { notch: 0, island: 10 };                        // the island floats 10 px below the edge
const TOURS = {
  notch: [['idle', 2200], ['music', 2600], ['drop', 2400], ['exp', 3600], ['lyrics', 3600], ['hud', 2000], ['timer', 3000]],
  island: [['idle', 2200], ['music', 2600], ['two', 2600], ['drop', 2400], ['exp', 3600], ['hud', 2000], ['widgets', 3200], ['timer', 3000]],
};
const shotHosts = $$('.island-shots');
const shotSrc = (lk, st) => `assets/shots/${lk}-${st}.png`;
function placeShot(img, lk) {
  const w = img.naturalWidth / SHOT.scale;
  img.style.width = w + 'px';
  img.style.top = (TOP[lk] - SHOT.pad[lk] / SHOT.scale) + 'px';
}
function showShot(host, st) {
  const lk = look, src = shotSrc(lk, st);
  const prev = host.querySelector('img.on');
  if (prev && prev.dataset.key === lk + st) return;
  const img = new Image();
  img.alt = ''; img.decoding = 'async'; img.dataset.key = lk + st;
  img.onload = () => {
    placeShot(img, lk);
    host.appendChild(img);
    requestAnimationFrame(() => requestAnimationFrame(() => img.classList.add('on')));
    if (prev) { prev.classList.remove('on'); setTimeout(() => prev.remove(), reduced ? 0 : 420); }
  };
  img.src = src;
}
function fit(host) {
  const box = host.closest('[data-island-host]'); if (!box) return;
  const w = box.clientWidth;
  // On the hero screen keep near-real proportions; small screens get a slightly larger island so it stays legible.
  const s = box.dataset.islandHost === 'screen'
    ? Math.min(1, (w / 1000) * (w < 600 ? 1.38 : 1))
    : Math.min(1, (w - 32) / 760);
  host.style.setProperty('--s', s.toFixed(3));
}
const ro = new ResizeObserver(() => shotHosts.forEach(fit));
shotHosts.forEach((h) => { fit(h); ro.observe(h.closest('[data-island-host]')); });

const tours = new Map(); // host -> { i, timer, visible }
function tourStep(host) {
  const t = tours.get(host); clearTimeout(t.timer);
  const seq = TOURS[look]; t.i %= seq.length;
  showShot(host, seq[t.i][0]);
  if (!reduced && t.visible) t.timer = setTimeout(() => { t.i++; tourStep(host); }, seq[t.i][1]);
}
function renderShots(restart) {
  shotHosts.forEach((h) => {
    if (h.hasAttribute('data-tour')) { const t = tours.get(h); if (t?.started) { if (restart) t.i = 0; tourStep(h); } }
    else if (h.dataset.loaded) showShot(h, h.dataset.shot);
  });
}
// load each one only when it nears the viewport; tours pause while off screen
const shotIO = new IntersectionObserver((entries) => entries.forEach((e) => {
  const h = e.target;
  if (h.hasAttribute('data-tour')) {
    const t = tours.get(h) || { i: 0, timer: 0 };
    tours.set(h, t);
    t.visible = e.isIntersecting;
    if (t.visible) { t.started = true; tourStep(h); } else clearTimeout(t.timer);
  } else if (e.isIntersecting && !h.dataset.loaded) { h.dataset.loaded = '1'; showShot(h, h.dataset.shot); }
}), { rootMargin: '300px 0px' });
shotHosts.forEach((h) => shotIO.observe(h));

// ------------------------------------------------------------ Dynamic Notch / Dynamic Island (every toggle on the page stays in sync)
const swapTimers = new WeakMap();
function swapImage(img, src) {
  if (img.getAttribute('src') === src) return;
  clearTimeout(swapTimers.get(img));
  const pre = new Image();
  pre.onload = () => {
    if (reduced) { img.src = src; return; }
    img.classList.add('swap');
    swapTimers.set(img, setTimeout(() => { img.src = src; img.classList.remove('swap'); }, 180));
  };
  pre.src = src;
}
const LOOK_NAME = { notch: 'Dynamic Notch', island: 'Dynamic Island' };
function setLook(next) {
  if (next !== 'island') next = 'notch';
  if (next === look) return;
  look = next;
  root.classList.toggle('look-island', next === 'island');
  root.classList.toggle('look-notch', next === 'notch');
  $$('.seg button').forEach((b) => {
    const on = b.dataset.look === next;
    b.setAttribute('aria-checked', String(on));
    b.tabIndex = on ? 0 : -1;
  });
  $$('.look-name').forEach((el) => (el.textContent = LOOK_NAME[next]));
  renderShots(true);
  $$('img[data-notch]').forEach((img) => swapImage(img, img.dataset[next]));
}
$$('.seg').forEach((seg) => {
  const buttons = $$('button', seg);
  buttons.forEach((b, i) => {
    b.addEventListener('click', () => setLook(b.dataset.look));
    // radio-group keyboard pattern: arrows move and select
    b.addEventListener('keydown', (e) => {
      const dir = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
      if (!dir) return;
      e.preventDefault();
      const t = buttons[(i + dir + buttons.length) % buttons.length];
      setLook(t.dataset.look); t.focus();
    });
  });
});
// ------------------------------------------------------------ setup wizard carousel
const wizard = document.getElementById('wizard');
const slides = $$('#wzShots img');
const steps = $$('#wzSteps button');
const DUR = 3600;
let cur = 0, inView = false, hold = false;
wizard.style.setProperty('--wz-dur', DUR + 'ms');
function show(i) {
  cur = (i + slides.length) % slides.length;
  slides.forEach((s, j) => {
    s.classList.toggle('on', j === cur);
    s.setAttribute('aria-hidden', String(j !== cur));
    if (Math.abs(j - cur) <= 1) s.loading = 'eager'; // warm up the neighbours
  });
  steps.forEach((b, j) => {
    b.setAttribute('aria-selected', String(j === cur));
    b.classList.toggle('done', j < cur);
    b.tabIndex = j === cur ? 0 : -1;
  });
  // restart the progress bar animation on the active step
  const bar = steps[cur].querySelector('i');
  bar.style.animation = 'none'; void bar.offsetWidth; bar.style.animation = '';
}
function sync() {
  const play = inView && !reduced;
  wizard.classList.toggle('playing', play);
  wizard.classList.toggle('paused', play && hold);
}
steps.forEach((b, i) => {
  b.addEventListener('click', () => show(i));
  b.addEventListener('keydown', (e) => {
    const dir = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
    if (e.key === 'Home' || e.key === 'End') { e.preventDefault(); show(e.key === 'Home' ? 0 : slides.length - 1); steps[cur].focus(); return; }
    if (!dir) return;
    e.preventDefault(); show(i + dir); steps[cur].focus();
  });
  b.querySelector('i').addEventListener('animationend', () => { if (wizard.classList.contains('playing')) show(cur + 1); });
});
wizard.addEventListener('pointerenter', () => { hold = true; sync(); });
wizard.addEventListener('pointerleave', () => { hold = false; sync(); });
wizard.addEventListener('focusin', () => { hold = true; sync(); });
wizard.addEventListener('focusout', (e) => { if (!wizard.contains(e.relatedTarget)) { hold = false; sync(); } });
new IntersectionObserver((es) => { inView = es[0].isIntersecting; sync(); }, { threshold: 0.35 }).observe(wizard);
reducedQuery.addEventListener?.('change', sync);
show(0);
