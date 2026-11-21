// WinIsland website: live island demos (the real app UI on its demo bridge), look switching, reveals, setup carousel.
'use strict';
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const root = document.documentElement;
const reducedQuery = matchMedia('(prefers-reduced-motion: reduce)');
let reduced = reducedQuery.matches;
reducedQuery.addEventListener?.('change', (e) => { reduced = e.matches; });
let look = 'macos';

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

// ------------------------------------------------------------ live island frames
const frames = $$('.island-frame');
function fit(frame) {
  const host = frame.closest('[data-island-host]'); if (!host) return;
  const w = host.clientWidth;
  // On the hero screen keep near-real proportions; small screens get a slightly larger island so it stays legible.
  const s = host.dataset.islandHost === 'screen'
    ? Math.min(1, (w / 1000) * (w < 600 ? 1.38 : 1))
    : Math.min(1, (w - 32) / 760);
  frame.style.setProperty('--s', s.toFixed(3));
}
const ro = new ResizeObserver(() => frames.forEach(fit));
frames.forEach((f) => { fit(f); ro.observe(f.closest('[data-island-host]')); });

// Load each demo only when it nears the viewport: they run the real UI, so keep the page light.
const lazyIO = new IntersectionObserver((entries) => entries.forEach((e) => {
  if (!e.isIntersecting) return;
  const f = e.target; lazyIO.unobserve(f);
  f.src = f.dataset.src.replace(/style=\w+/, 'style=' + look);
}), { rootMargin: '400px 0px' });
frames.forEach((f) => lazyIO.observe(f));
addEventListener('message', (e) => {
  const f = frames.find((x) => x.contentWindow === e.source);
  if (f && e.data && e.data.frostDemo === 'ready') f.contentWindow.postMessage({ frost: 'style', value: look }, '*');
});

// ------------------------------------------------------------ macOS / iOS look (every toggle on the page stays in sync)
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
function setLook(next) {
  if (next !== 'ios') next = 'macos';
  if (next === look) return;
  look = next;
  root.classList.toggle('look-ios', next === 'ios');
  root.classList.toggle('look-macos', next === 'macos');
  $$('.seg button').forEach((b) => {
    const on = b.dataset.look === next;
    b.setAttribute('aria-checked', String(on));
    b.tabIndex = on ? 0 : -1;
  });
  $$('.look-name').forEach((el) => (el.textContent = next === 'ios' ? 'iOS look' : 'macOS look'));
  frames.forEach((f) => { if (f.getAttribute('src')) f.contentWindow?.postMessage({ frost: 'style', value: next }, '*'); });
  $$('img[data-macos]').forEach((img) => swapImage(img, img.dataset[next]));
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
