// Sections fade in as they scroll into view.
const io = 'IntersectionObserver' in window && new IntersectionObserver(es => es.forEach(e => {
  if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target); }
}), { threshold: .12 });
document.querySelectorAll('.reveal').forEach(el => io ? io.observe(el) : el.classList.add('in'));

// The download button names the visitor's system and links its file straight from the latest release.
(async () => {
  const API = 'https://api.github.com/repos/artemspt/PrismLauncher-Horizon/releases/latest';
  const ua = navigator.userAgent, plat = (navigator.userAgentData?.platform || navigator.platform || '') + ' ' + ua;
  const os = /Win/i.test(plat) ? 'Windows' : /Mac|iPhone|iPad/i.test(plat) ? 'macOS'
    : /Linux|X11|CrOS/i.test(plat) && !/Android/i.test(ua) ? 'Linux' : null;
  let arm = /aarch64|arm64/i.test(ua);
  try { const h = await navigator.userAgentData?.getHighEntropyValues(['architecture']); if (h?.architecture) arm = h.architecture === 'arm'; } catch {}
  const btn = document.getElementById('dl-btn'), note = document.getElementById('dl-note');
  const all = document.getElementById('dl-all');
  if (!btn) return;
  if (os === 'Linux') {  // these names carry no version, so they work without asking GitHub
    btn.href = 'https://github.com/artemspt/PrismLauncher-Horizon/releases/latest/download/PrismLauncher-Linux-' + (arm ? 'aarch64' : 'x86_64') + '.AppImage';
  }
  if (os) btn.textContent = 'Скачать для ' + os + (arm ? ' (ARM)' : '');
  let rel = null;
  try { const r = await fetch(API); if (r.ok) rel = await r.json(); } catch {}
  if (!rel) return;
  // Prism also ships each Windows build as a plain zip beside the portable one: list the portable only
  const files = rel.assets.filter(a => !a.name.endsWith('.zsync') && !(/Windows/.test(a.name) && !/Portable|Setup/.test(a.name)));
  // MSVC before MinGW, x64 before ARM: the usual pick first
  const rank = n => (/MinGW/.test(n) ? 2 : 0) + (/arm64|aarch64/.test(n) ? 1 : 0);
  files.sort((a, b) => rank(a.name) - rank(b.name));
  const pick = {
    Linux: arm ? /Linux-aarch64\.AppImage$/ : /Linux-x86_64\.AppImage$/,
    Windows: arm ? /Windows-MSVC-arm64-Setup.*\.exe$/ : /Windows-MSVC-Setup.*\.exe$/,
    macOS: /macOS.*\.(dmg|zip)$/,
  };
  const mb = n => (n / 1048576).toFixed(0) + ' МБ';
  const group = n => /Linux/.test(n) ? 'Linux' : /Windows/.test(n) ? 'Windows' : /macOS/.test(n) ? 'macOS' : 'Исходники';
  const label = n => {
    const ext = (n.match(/\.(AppImage|exe|zip|tar\.gz|dmg)$/) || [, ''])[1];
    let parts = n.replace(/^PrismLauncher-/, '').replace(/-?v[\d.]+-horizon/, '').replace('.' + ext, '')
      .split('-').filter(p => p && !['Linux', 'Windows', 'macOS', 'Qt6', 'Setup', 'w64'].includes(p));
    const g = group(n);
    if (g === 'Linux' && !parts.some(p => /aarch64|x86_64/.test(p))) parts.unshift('x86_64');
    if (g === 'Windows' && !parts.includes('arm64')) parts.unshift('x64');
    parts.sort((x, y) => /arm64|aarch64|x86_64|x64/.test(y) - /arm64|aarch64|x86_64|x64/.test(x));  // arch first
    if (g === 'Исходники') parts = ['исходный код'];
    return [...parts, ext === 'exe' ? 'установщик' : ext].join(' · ');
  };
  const mine = os && files.find(a => pick[os].test(a.name));
  if (mine) {
    btn.href = mine.browser_download_url;
    note.textContent = `${rel.tag_name} · ${label(mine.name)} · ${mb(mine.size)}`;
  } else if (os === 'macOS') {
    btn.href = rel.html_url;
    note.textContent = 'Сборка для macOS ещё не готова — она появится на странице релиза.';
  } else note.textContent = rel.tag_name;
  if (!all) return;
  const groups = {};
  for (const a of files) (groups[group(a.name)] ||= []).push(a);
  all.innerHTML = '';
  for (const g of ['Linux', 'Windows', 'macOS', 'Исходники']) {
    if (!groups[g]) continue;
    const box = document.createElement('div');
    box.innerHTML = `<h4>${g}</h4>`;
    for (const a of groups[g]) {
      const l = document.createElement('a');
      l.href = a.browser_download_url;
      l.textContent = label(a.name);
      l.title = a.name;
      l.insertAdjacentHTML('beforeend', `<small>${mb(a.size)}</small>`);
      box.append(l);
    }
    all.append(box);
  }
})();


// Copy buttons.
document.querySelectorAll('[data-copy]').forEach(b => b.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(document.getElementById(b.dataset.copy).innerText.split('\n').filter(l => !l.startsWith('#')).join('\n'));
    b.textContent = 'Скопировано';
  } catch { b.textContent = 'Не вышло'; }
  setTimeout(() => b.textContent = 'Копировать', 1600);
}));
