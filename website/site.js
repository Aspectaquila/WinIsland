// WinIsland website: live island demos (the real app UI on its demo bridge), look switching, reveals, carousel.
'use strict';
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
let look = 'macos';

// ---------------------------------------------------------------- nav
const nav = document.getElementById('nav');
const onScroll = () => nav.classList.toggle('scrolled', scrollY > 12);
addEventListener('scroll', onScroll, { passive: true }); onScroll();

// ---------------------------------------------------------------- scroll reveal
const io = new IntersectionObserver((es) => es.forEach((e) => { if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target); } }), { threshold: 0.12, rootMargin: '0px 0px -40px 0px' });
$$('.reveal, .tile').forEach((el) => (reduced ? el.classList.add('in') : io.observe(el)));

// ---------------------------------------------------------------- live island frames
const frames = $$('.island-frame');
function fit(frame) {
  const host = frame.closest('[data-island-host]'); if (!host) return;
  const w = host.clientWidth;
  // on the laptop screen keep roughly real proportions, but give small screens a bigger island so it stays legible
  const s = host.classList.contains('screen') ? Math.min(1, (w / 1150) * (w < 600 ? 1.45 : 1)) : Math.min(1.05, (w - 32) / 760);
  frame.style.setProperty('--s', s.toFixed(3));
}
const ro = new ResizeObserver(() => frames.forEach(fit));
frames.forEach((f) => { fit(f); ro.observe(f.closest('[data-island-host]')); });
// load each demo only when it comes near the viewport (they're the real UI, so keep the page light)
const lazy = new IntersectionObserver((es) => es.forEach((e) => {
  if (!e.isIntersecting) return;
  const f = e.target; lazy.unobserve(f);
  f.src = f.dataset.src.replace(/style=\w+/, 'style=' + look);
}), { rootMargin: '300px' });
frames.forEach((f) => lazy.observe(f));
addEventListener('message', (e) => {
  const f = frames.find((x) => x.contentWindow === e.source);
  if (f && e.data?.frostDemo === 'ready') f.contentWindow.postMessage({ frost: 'style', value: look }, '*');
});

// ---------------------------------------------------------------- macOS / iOS look (every toggle on the page stays in sync)
function setLook(next) {
  if (next === look) return;
  look = next;
  document.body.classList.toggle('look-ios', next === 'ios');
  document.body.classList.toggle('look-macos', next !== 'ios');
  $$('.look-toggle button').forEach((b) => b.setAttribute('aria-selected', String(b.dataset.look === next)));
  $$('.look-name').forEach((el) => (el.textContent = next === 'ios' ? 'iOS look' : 'macOS look'));
  frames.forEach((f) => { if (f.src) f.contentWindow?.postMessage({ frost: 'style', value: next }, '*'); });
  $$('img[data-look-src]').forEach((img) => {
    const src = 'assets/' + img.dataset.lookSrc.replace('{look}', next);
    const pre = new Image();
    pre.onload = () => { img.classList.add('swap'); setTimeout(() => { img.src = src; img.classList.remove('swap'); }, reduced ? 0 : 280); };
    pre.src = src;
  });
}
$$('.look-toggle button').forEach((b) => b.addEventListener('click', () => setLook(b.dataset.look)));

// ---------------------------------------------------------------- setup wizard carousel
const wz = document.getElementById('wz'), dots = document.getElementById('wzDots');
const slides = $$('img', wz);
let cur = 0, wzT = 0, wzVisible = false;
slides.forEach((_, i) => {
  const b = document.createElement('button');
  b.setAttribute('aria-label', 'Step ' + (i + 1)); b.setAttribute('role', 'tab');
  b.onclick = () => { show(i); restart(); };
  dots.appendChild(b);
});
function show(i) {
  cur = (i + slides.length) % slides.length;
  slides.forEach((s, j) => s.classList.toggle('on', j === cur));
  $$('button', dots).forEach((d, j) => { d.classList.toggle('on', j === cur); d.setAttribute('aria-selected', String(j === cur)); });
}
function restart() { clearInterval(wzT); if (wzVisible && !reduced) wzT = setInterval(() => show(cur + 1), 3200); }
new IntersectionObserver((es) => { wzVisible = es[0].isIntersecting; restart(); }, { threshold: 0.3 }).observe(wz);
show(0);
