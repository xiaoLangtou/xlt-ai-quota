/* ============================================================
   XLT Workbench 改版原型 · 应用逻辑
   哈希路由 / 双主题 / 手绘 SVG 图表 / 演示数据
   ============================================================ */
'use strict';

/* ---------------- 图标（手绘描边 SVG） ---------------- */
const ICONS = {
  grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  chart: '<path d="M3 3v16a2 2 0 0 0 2 2h16"/><path d="M7 14l4-5 3.5 3.5L19 7"/>',
  wallet: '<rect x="3" y="6" width="18" height="13" rx="2.5"/><path d="M3 10h18"/><path d="M16 15h2"/>',
  branch: '<circle cx="6" cy="6" r="2.5"/><circle cx="6" cy="18" r="2.5"/><circle cx="18" cy="8" r="2.5"/><path d="M6 8.5v7"/><path d="M18 10.5c0 4-4 4.5-7.5 5"/>',
  clipboard: '<rect x="5" y="4" width="14" height="17" rx="2"/><path d="M9 4a3 3 0 0 1 6 0"/><path d="M9 11h6M9 15h4"/>',
  code: '<path d="M8 6l-5 6 5 6"/><path d="M16 6l5 6-5 6"/>',
  lock: '<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M2 12h3M19 12h3M4.9 19.1L7 17M17 7l2.1-2.1"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  sync: '<path d="M21 12a9 9 0 1 1-2.6-6.4"/><path d="M21 3v6h-6"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>',
  arrow: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  check: '<path d="M4 12.5l5 5L20 6.5"/>',
  doc: '<path d="M6 2h8l5 5v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z"/><path d="M14 2v5h5"/>',
  image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="M21 16l-5-5-9 9"/>',
  file: '<path d="M6 2h8l5 5v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z"/>',
  pin: '<path d="M12 17v5"/><path d="M8 3h8l-1 7 3 3H6l3-3z"/>',
  eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/>',
  copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
  edit: '<path d="M17 3l4 4L8 20l-5 1 1-5z"/>',
  trash: '<path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>',
  link: '<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>',
  chevron: '<path d="M6 9l6 6 6-6"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.5 2"/>',
  alert: '<path d="M12 3L2 20h20z"/><path d="M12 10v4M12 17.5v.5"/>',
  fuel: '<path d="M5 21V5a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v16"/><path d="M3 21h14"/><path d="M15 9h3a2 2 0 0 1 2 2v6a2 2 0 0 0 4 0V9.8a2 2 0 0 0-.6-1.4L21 6"/><path d="M8 7h4v4H8z"/>',
  calendar: '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M8 3v4M16 3v4M3 10h18"/>',
  user: '<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 3.5-6.5 8-6.5s8 2.5 8 6.5"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  terminal: '<path d="M5 7l5 5-5 5"/><path d="M12 17h7"/>',
  shield: '<path d="M12 2l8 3.5v5.5c0 5-3.4 9.3-8 11-4.6-1.7-8-6-8-11V5.5z"/>',
  up: '<path d="M12 19V5M6 11l6-6 6 6"/>',
  down: '<path d="M12 5v14M6 13l6 6 6-6"/>',
  pause: '<path d="M9 5v14M15 5v14"/>',
  key: '<circle cx="8" cy="15" r="4.5"/><path d="M11.5 11.5L21 2"/><path d="M16 7l3 3"/>',
  sparkle: '<path d="M12 3l1.9 5.6L19.5 10l-5.6 1.9L12 17.5l-1.9-5.6L4.5 10l5.6-1.4z"/>',
  monitor: '<rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.5v.5"/>',
};
function icon(name, size = 17, sw = 1.7) {
  return '<svg width="' + size + '" height="' + size + '" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="' + sw + '" stroke-linecap="round" stroke-linejoin="round">' + (ICONS[name] || '') + '</svg>';
}

/* ---------------- 主题 ---------------- */
const THEME_KEY = 'xlt-theme';
function getTheme() { return localStorage.getItem(THEME_KEY) || 'dark'; }
function applyTheme(t) {
  document.documentElement.setAttribute('data-theme', t);
  localStorage.setItem(THEME_KEY, t);
}
function setTheme(t) {
  applyTheme(t);
  if (typeof NAV !== 'undefined' && document.getElementById('sidebar').innerHTML) renderSidebar();
}
const urlTheme = new URLSearchParams(location.search).get('theme');
applyTheme(urlTheme === 'light' || urlTheme === 'dark' ? urlTheme : getTheme());

/* ---------------- 工具函数 ---------------- */
function esc(s) { return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
function fmt(n) { return n.toLocaleString('en-US'); }
function toast(msg) {
  let t = document.querySelector('.toast');
  if (!t) { t = document.createElement('div'); t.className = 'toast'; document.body.appendChild(t); }
  t.innerHTML = '<span class="ok">' + icon('check', 15, 2.2) + '</span>' + esc(msg);
  requestAnimationFrame(() => t.classList.add('show'));
  clearTimeout(t._tm);
  t._tm = setTimeout(() => t.classList.remove('show'), 2200);
}
/* 确定性伪随机 */
function seeded(seed) { let s = seed; return () => { s = (s * 9301 + 49297) % 233280; return s / 233280; }; }

/* ---------------- 图表助手 ---------------- */
function sparkline(points, w, h, color) {
  w = w || 104; h = h || 32; color = color || 'var(--accent)';
  const max = Math.max.apply(null, points), min = Math.min.apply(null, points);
  const span = (max - min) || 1;
  const step = w / (points.length - 1);
  let d = '';
  points.forEach((p, i) => {
    const x = (i * step).toFixed(1), y = (h - 3 - ((p - min) / span) * (h - 6)).toFixed(1);
    d += (i === 0 ? 'M' : 'L') + x + ' ' + y;
  });
  return '<svg width="' + w + '" height="' + h + '" viewBox="0 0 ' + w + ' ' + h + '" fill="none"><path d="' + d + '" stroke="' + color + '" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" opacity=".85"/></svg>';
}

function areaChart(points, labels, w, h) {
  w = w || 560; h = h || 180;
  const max = Math.max.apply(null, points) * 1.15;
  const stepX = w / (points.length - 1);
  const py = p => (h - 24 - (p / max) * (h - 44));
  let d = '';
  points.forEach((p, i) => {
    const x = i * stepX, y = py(p);
    if (i === 0) d = 'M0 ' + y.toFixed(1);
    else {
      const px = (i - 1) * stepX, ppy = py(points[i - 1]);
      const cx = (px + x) / 2;
      d += ' C' + cx.toFixed(1) + ' ' + ppy.toFixed(1) + ' ' + cx.toFixed(1) + ' ' + y.toFixed(1) + ' ' + x.toFixed(1) + ' ' + y.toFixed(1);
    }
  });
  const area = d + ' L' + w + ' ' + (h - 24) + ' L0 ' + (h - 24) + ' Z';
  let grid = '';
  for (let g = 0; g < 4; g++) {
    const y = 8 + g * ((h - 44) / 3);
    grid += '<line x1="0" y1="' + y + '" x2="' + w + '" y2="' + y + '" stroke="var(--border)" stroke-width="1" stroke-dasharray="3 5"/>';
  }
  let dots = '', labs = '';
  points.forEach((p, i) => {
    dots += '<circle cx="' + (i * stepX).toFixed(1) + '" cy="' + py(p).toFixed(1) + '" r="3" fill="var(--raise)" stroke="var(--accent)" stroke-width="1.8"/>';
    if (labels && labels[i]) {
      const anchor = i === 0 ? 'start' : i === points.length - 1 ? 'end' : 'middle';
      labs += '<text x="' + (i * stepX).toFixed(1) + '" y="' + (h - 6) + '" font-size="10" fill="var(--text-3)" text-anchor="' + anchor + '" font-family="var(--mono)">' + labels[i] + '</text>';
    }
  });
  const gid = 'ag' + Math.floor(Math.random() * 1e6);
  return '<svg width="100%" height="' + h + '" viewBox="0 0 ' + w + ' ' + h + '" preserveAspectRatio="none" style="display:block">' +
    '<defs><linearGradient id="' + gid + '" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".28"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs>' +
    grid +
    '<path d="' + area + '" fill="url(#' + gid + ')"/>' +
    '<path d="' + d + '" fill="none" stroke="var(--accent)" stroke-width="2" stroke-linecap="round"/>' +
    dots + labs + '</svg>';
}

function stackedBars(days, w, h) {
  w = w || 560; h = h || 180;
  const max = Math.max.apply(null, days.map(d => d.segs.reduce((a, s) => a + s.v, 0))) * 1.1;
  const bw = Math.min(34, (w / days.length) * 0.52);
  const gap = w / days.length;
  let bars = '', labs = '';
  days.forEach((d, i) => {
    let y = h - 24;
    const x = i * gap + (gap - bw) / 2;
    d.segs.forEach(s => {
      const sh = (s.v / max) * (h - 46);
      y -= sh;
      if (sh > 0.5) bars += '<rect x="' + x.toFixed(1) + '" y="' + y.toFixed(1) + '" width="' + bw + '" height="' + Math.max(sh - 1.5, 0.5).toFixed(1) + '" rx="2.5" fill="' + s.c + '"/>';
    });
    labs += '<text x="' + (i * gap + gap / 2).toFixed(1) + '" y="' + (h - 6) + '" font-size="10" fill="var(--text-3)" text-anchor="middle" font-family="var(--mono)">' + d.label + '</text>';
  });
  let grid = '';
  for (let g = 0; g < 4; g++) {
    const y = 8 + g * ((h - 46) / 3);
    grid += '<line x1="0" y1="' + y + '" x2="' + w + '" y2="' + y + '" stroke="var(--border)" stroke-width="1" stroke-dasharray="3 5"/>';
  }
  return '<svg width="100%" height="' + h + '" viewBox="0 0 ' + w + ' ' + h + '" preserveAspectRatio="none" style="display:block">' + grid + bars + labs + '</svg>';
}

function donut(segs, size, thick) {
  size = size || 168; thick = thick || 21;
  const r = (size - thick) / 2, c = 2 * Math.PI * r, cx = size / 2;
  let off = 0, arcs = '';
  segs.forEach(s => {
    const len = (s.pct / 100) * c;
    arcs += '<circle cx="' + cx + '" cy="' + cx + '" r="' + r + '" fill="none" stroke="' + s.c + '" stroke-width="' + thick + '" stroke-dasharray="' + Math.max(len - 2.5, 0.5).toFixed(1) + ' ' + (c - len + 2.5).toFixed(1) + '" stroke-dashoffset="' + (-off).toFixed(1) + '" stroke-linecap="round" style="transition:stroke-dasharray .8s var(--ease)"/>';
    off += len;
  });
  return '<svg width="' + size + '" height="' + size + '" viewBox="0 0 ' + size + ' ' + size + '" style="transform:rotate(-90deg)">' + arcs + '</svg>';
}

/* ---------------- 共享组件 ---------------- */
function statCard(o, i) {
  const delta = o.delta ? '<span class="delta ' + o.deltaDir + '">' + icon(o.deltaDir === 'up' ? 'up' : 'down', 11, 2.4) + esc(o.delta) + '</span>' : '';
  const spark = o.spark ? '<div class="stat-spark"' + (o.delta ? ' style="top:50px"' : '') + '>' + sparkline(o.spark, 96, 30, o.sparkColor) + '</div>' : '';
  return '<div class="card stat-card hoverable rv" style="--i:' + i + '">' +
    '<div class="stat-label">' + esc(o.label) + delta + '</div>' +
    '<div class="stat-value">' + o.value + (o.unit ? '<span class="unit">' + esc(o.unit) + '</span>' : '') + '</div>' +
    (o.sub ? '<div class="stat-sub">' + esc(o.sub) + '</div>' : '') +
    spark + '</div>';
}

function svcBadge(name, color, letter) {
  return '<span class="svc" style="color:' + color + ';background:color-mix(in srgb,' + color + ' 12%,transparent)">' + (letter || name[0]) + '</span>';
}

function meterRow(label, sub, pct, status) {
  const cls = status || (pct >= 95 ? 'danger' : pct >= 75 ? 'warn' : '');
  const pctCls = pct >= 95 ? 'red' : pct >= 75 ? 'amber' : '';
  return '<div class="quota-row">' +
    '<div class="quota-row-head"><span>' + esc(label) + (sub ? ' <span class="muted">· ' + esc(sub) + '</span>' : '') + '</span><span class="quota-pct ' + pctCls + '">' + pct + '%</span></div>' +
    '<div class="meter ' + cls + '"><i style="width:' + Math.min(pct, 100) + '%"></i></div></div>';
}

function pageHead(eyebrow, title, desc, actions) {
  return '<div class="page-head rv" style="--i:0"><div>' +
    '<div class="page-eyebrow">' + esc(eyebrow) + '</div>' +
    '<div class="page-title">' + esc(title) + '</div>' +
    (desc ? '<div class="page-desc">' + esc(desc) + '</div>' : '') +
    '</div><div class="page-actions">' + (actions || '') + '</div></div>';
}

function sec(title, sub, link) {
  return '<div class="sec rv" style="--i:2"><div><div class="sec-title">' + esc(title) + '</div>' +
    (sub ? '<div class="sec-sub">' + esc(sub) + '</div>' : '') + '</div>' +
    (link ? '<span class="sec-link">' + esc(link) + ' ' + icon('arrow', 13) + '</span>' : '') + '</div>';
}

/* ---------------- 路由与侧边栏 ---------------- */
const NAV = [
  { group: '洞察' },
  { id: 'overview', label: '概览', icon: 'grid', kbd: '1' },
  { id: 'analytics', label: '用量分析', icon: 'chart', kbd: '2' },
  { id: 'billing', label: '费用中心', icon: 'wallet', kbd: '3' },
  { group: '工作' },
  { id: 'reports', label: 'Git 报告', icon: 'branch', kbd: '4' },
  { group: '工具箱' },
  { id: 'clipboard', label: '剪贴板历史', icon: 'clipboard', kbd: '5' },
  { id: 'snippets', label: '代码片段', icon: 'code', kbd: '6' },
  { id: 'vault', label: '密钥库', icon: 'lock', kbd: '7' },
];

function currentRoute() {
  const h = location.hash.replace(/^#\/?/, '');
  return (typeof VIEWS !== 'undefined' && VIEWS[h]) ? h : 'overview';
}

function renderSidebar() {
  const cur = currentRoute();
  const t = getTheme();
  let html = '<div class="brand">' +
    '<svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 7l5 5-5 5"/><path d="M12 17h7"/></svg>' +
    '<div><div class="brand-name">XLT Workbench</div>' +
    '<div class="brand-sub"><span class="dot"></span>5 个平台已连接</div></div></div><nav class="nav">';
  NAV.forEach(n => {
    if (n.group) { html += '<div class="nav-group"><div class="nav-group-label">' + n.group + '</div>'; return; }
    html += '<div class="nav-item' + (n.id === cur ? ' active' : '') + '" data-nav="' + n.id + '">' +
      icon(n.icon, 16) + '<span>' + n.label + '</span><span class="kbd">⌘' + n.kbd + '</span></div>';
  });
  html += '</nav>';
  html += '<div class="nav-group" style="padding:0 0 4px"><div class="nav-item' + (cur === 'settings' ? ' active' : '') + '" data-nav="settings" style="margin:0 0 2px">' + icon('gear', 16) + '<span>设置</span><span class="kbd">⌘,</span></div></div>';
  html += '<div class="sidebar-foot"><button class="theme-toggle" id="themeBtn">' +
    icon(t === 'dark' ? 'moon' : 'sun', 15) +
    '<span>' + (t === 'dark' ? '深色模式' : '浅色模式') + '</span><span class="pill"></span></button></div>';
  document.getElementById('sidebar').innerHTML = html;
  document.querySelectorAll('[data-nav]').forEach(el => {
    el.addEventListener('click', () => { location.hash = '#/' + el.dataset.nav; });
  });
  document.getElementById('themeBtn').addEventListener('click', () => {
    setTheme(getTheme() === 'dark' ? 'light' : 'dark');
  });
}

/* 键盘导航 */
document.addEventListener('keydown', e => {
  if ((e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey) {
    const map = { '1': 'overview', '2': 'analytics', '3': 'billing', '4': 'reports', '5': 'clipboard', '6': 'snippets', '7': 'vault', ',': 'settings' };
    if (map[e.key]) { e.preventDefault(); location.hash = '#/' + map[e.key]; }
  }
});

/* ============================================================
   演示数据（取自现有产品截图）
   ============================================================ */
const TOOL_COLORS = { 'Codex': '#4F9D69', 'OpenCode Go': '#8fbf9f', 'Kimi': '#e3d06b', 'Qoder': '#e8a4d6', 'Claude Code': '#b08d6a', 'Kiro': '#8a8a8a' };

const ANALYTICS = {
  week: {
    label: '7 天',
    stats: [
      { label: '总 TOKEN', value: '150.1', unit: 'M', delta: '52.5%', deltaDir: 'down', spark: [42, 58, 38, 61, 30, 26, 18] },
      { label: '输入 TOKEN', value: '147.6', unit: 'M', delta: '52.9%', deltaDir: 'down', sub: '占总量 98%', spark: [40, 56, 36, 59, 29, 25, 17] },
      { label: '输出 TOKEN', value: '2.5', unit: 'M', delta: '4%', deltaDir: 'down', sub: '占总量 2%', spark: [0.9, 1.1, 0.8, 1.3, 0.7, 0.9, 0.6] },
      { label: '请求数', value: '3.5', unit: 'K', delta: '19.4%', deltaDir: 'down', spark: [640, 720, 580, 690, 420, 380, 310] },
      { label: '预估费用', value: '$296.38', delta: '139.3%', deltaDir: 'up', sub: '按模型单价估算', spark: [18, 24, 16, 60, 52, 88, 96], sparkColor: 'var(--amber)' },
    ],
    trend: { points: [42, 58, 38, 61, 30, 26, 18], labels: ['9/18', '9/19', '9/20', '9/21', '9/22', '9/23', '9/24'] },
    daily: [
      { label: '9/18', segs: [{ v: 40, c: TOOL_COLORS['Codex'] }] },
      { label: '9/19', segs: [{ v: 1.2, c: TOOL_COLORS['Qoder'] }] },
      { label: '9/20', segs: [{ v: 36, c: TOOL_COLORS['Codex'] }] },
      { label: '9/21', segs: [{ v: 6, c: TOOL_COLORS['Codex'] }, { v: 12, c: TOOL_COLORS['OpenCode Go'] }] },
      { label: '9/22', segs: [{ v: 1.6, c: TOOL_COLORS['OpenCode Go'] }] },
      { label: '9/23', segs: [{ v: 9, c: TOOL_COLORS['Codex'] }, { v: 8, c: TOOL_COLORS['Kimi'] }, { v: 5, c: TOOL_COLORS['Qoder'] }, { v: 4, c: TOOL_COLORS['OpenCode Go'] }] },
      { label: '9/24', segs: [{ v: 4, c: TOOL_COLORS['Qoder'] }, { v: 1.4, c: TOOL_COLORS['OpenCode Go'] }] },
    ],
    byTool: [
      { name: 'Codex', pct: 69.2, tokens: '103.9M', cost: '$22.12' },
      { name: 'OpenCode Go', pct: 11.2, tokens: '16.8M', cost: '$252.97' },
      { name: 'Kimi', pct: 11.1, tokens: '16.6M', cost: '$2.58' },
      { name: 'Qoder', pct: 5.3, tokens: '7.9M', cost: '$14.00' },
      { name: 'Claude Code', pct: 2.9, tokens: '4.4M', cost: '$3.51' },
      { name: 'Kiro', pct: 0.3, tokens: '423.0K', cost: '$1.21' },
    ],
    byModel: [
      { name: 'gpt-5.6-sol', tool: 'Codex', pct: 51.5, tokens: '77.3M', cost: '$15.27' },
      { name: 'gpt-5.6-terra', tool: 'Codex', pct: 17.4, tokens: '26.2M', cost: '$6.66' },
      { name: 'k2-0905-preview', tool: 'Kimi', pct: 11.1, tokens: '16.6M', cost: '$2.58' },
      { name: 'glm-4.6-air', tool: 'OpenCode Go', pct: 8.2, tokens: '12.3M', cost: '$188.40' },
      { name: 'qwen3-coder-max', tool: 'Qoder', pct: 5.3, tokens: '7.9M', cost: '$14.00' },
      { name: 'claude-sonnet-5', tool: 'Claude Code', pct: 2.9, tokens: '4.4M', cost: '$3.51' },
    ],
  },
};
/* 其它时间范围按周数据缩放生成 */
[['today', '今天', 0.09], ['month', '30 天', 4.6], ['quarter', '90 天', 13.2]].forEach(([k, label, f]) => {
  const rnd = seeded(f * 1000 | 0);
  const n = k === 'today' ? 1 : k === 'month' ? 30 : 90;
  const pts = [];
  for (let i = 0; i < Math.min(n, 30); i++) pts.push(+(8 + rnd() * 46 * (0.5 + f / 6)).toFixed(1));
  ANALYTICS[k] = {
    label,
    stats: ANALYTICS.week.stats.map(s => ({
      ...s,
      value: s.value.startsWith('$') ? '$' + (parseFloat(s.value.slice(1)) * f).toFixed(2) : (parseFloat(s.value) * f).toFixed(1),
      spark: pts.slice(0, 7),
    })),
    trend: { points: pts, labels: pts.map((_, i) => (k === 'today' ? '现在' : (i % 5 === 0 ? (i + 1) + '日' : ''))) },
    daily: ANALYTICS.week.daily.map(d => ({ label: d.label, segs: d.segs.map(s => ({ v: +(s.v * f).toFixed(1), c: s.c })) })),
    byTool: ANALYTICS.week.byTool,
    byModel: ANALYTICS.week.byModel,
  };
});

const QUOTAS = [
  { name: 'Codex', plan: 'Plus', color: TOOL_COLORS['Codex'], rows: [
    { label: '5h 额度', sub: '4h 50m 重置', pct: 0 },
    { label: '周额度', sub: '20m 重置', pct: 100, tag: '已耗尽' },
  ]},
  { name: '火山方舟', plan: 'Agent Plan', color: '#7bc496', rows: [
    { label: '5h 额度', sub: '即将重置', pct: 5 },
    { label: '近 1 周', sub: '3d 重置', pct: 39 },
    { label: '近 1 月', sub: '周四 23:59 重置', pct: 39 },
  ]},
  { name: 'Kiro', plan: 'KIRO PRO+', color: TOOL_COLORS['Kiro'], rows: [
    { label: '总额度', sub: '2,000 Credits · 已用 1,939.12 · 6d 重置', pct: 97 },
  ]},
  { name: 'Qoder', plan: 'Pro', color: TOOL_COLORS['Qoder'], rows: [
    { label: '总额度', sub: '2,000 Credits · 已用 2,000 · 周二 00:00 到期', pct: 100 },
    { label: '加购额度', sub: '2,200 Credits · 已用 2,200 · 剩余 0', pct: 100 },
  ]},
  { name: 'Kimi', plan: 'Plus', color: TOOL_COLORS['Kimi'], rows: [
    { label: '5h 额度', sub: '4h 2m 重置', pct: 0 },
  ]},
];

const SUBSCRIPTIONS = [
  { name: 'ChatGPT', plan: 'Plus', account: 'p91574282@gmail.c…', amount: 'US$23.00', cycle: '月付', next: '2026-09-01', nextSub: '已逾期 23 天', status: ['red', '已逾期'] },
  { name: '火山方舟', plan: 'Pro', account: 'weipengcheng(210…', amount: '¥50.00', cycle: '月付', next: '2026-09-17', nextSub: '已逾期 7 天', status: ['grey', '已暂停'] },
  { name: 'Kiro', plan: 'Pro+', account: 'p91574282@gmail.c…', amount: 'US$40.00', cycle: '月付', next: '2026-10-01', nextSub: '7 天后扣费', status: ['amber', '即将续费'] },
  { name: '蚂蚁数字科技', plan: '探索版', account: '755197142@qq.com', amount: '¥29.00', cycle: '月付', next: '2026-10-04', nextSub: '10 天后扣费', status: ['grey', '已暂停'] },
  { name: 'Qoder', plan: 'Plus', account: 'weipc35@gmail.com', amount: 'US$20.00', cycle: '月付', next: '2026-10-06', nextSub: '12 天后扣费', status: ['grey', '已暂停'] },
];

const BILLS = [
  { date: '09-20', name: 'Kimi', cat: '模型订阅', pay: '支付宝', src: ['green', '手动记账'], amount: '¥99.00' },
  { date: '09-16', name: '0011.ai', cat: 'API 中转', pay: '微信', src: ['green', '手动记账'], amount: 'US$20.00', cny: '≈ ¥144.00' },
  { date: '09-16', name: '火山方舟', cat: 'API 中转', pay: '微信', src: ['green', '手动记账'], amount: '¥49.90' },
  { date: '09-16', name: '蚂蚁数字科技', cat: '模型订阅', pay: '支付宝', src: ['green', '手动记账'], amount: '¥29.00' },
  { date: '09-06', name: 'Qoder', plan: 'Plus', cat: '模型订阅', pay: '其他', src: ['amber', '用量结算'], amount: 'US$20.00', cny: '≈ ¥144.00' },
  { date: '09-04', name: '蚂蚁数字科技', plan: '探索版', cat: '模型订阅', pay: '其他', src: ['amber', '用量结算'], amount: '¥29.00' },
  { date: '09-01', name: 'ChatGPT', plan: 'Plus', cat: '模型订阅', pay: '其他', src: ['amber', '用量结算'], amount: 'US$23.00', cny: '≈ ¥165.60' },
  { date: '09-01', name: 'Kiro', plan: 'Pro+', cat: '模型订阅', pay: '其他', src: ['amber', '用量结算'], amount: 'US$40.00', cny: '≈ ¥288.00' },
];

const CLIPS = [
  { type: '文本', title: "const scope = 'server' /** * 登录渠道：网关按 URL 末尾的渠道编…", app: 'webstorm', time: '9月22日 16:31', size: '491 B', uses: 0, body: "const scope = 'server'\n\n/**\n * 登录渠道：网关按 URL 末尾的渠道编码注入 Authorization、TENANT-ID，\n * 前端不再持有任何 Basic 凭证。渠道说明见《登录渠道接口改造-前端对接说明》。\n */\n\n// 当前环境渠道：gok（内网）、gw（国网生产）；console 统一跟随 gok\nconst getLoginChannel = () => {\n  const env = process.env['VUE_APP_ENV'] || 'gok'\n  if (env === 'gw' && window.location.hash === '#/console') return 'gok'\n  return env\n}" },
  { type: '文本', title: '根据这个文档改造登录接口', app: 'webstorm', time: '9月22日 15:02', size: '36 B', uses: 2, body: '根据这个文档改造登录接口' },
  { type: '文本', title: "// let password = 'JFatOZdc'\nlet password = userInfo.ty…", app: 'webstorm', time: '9月22日 14:47', size: '208 B', uses: 0, body: "// let password = 'JFatOZdc'\nlet password = userInfo.ty_token\n\n// 历史凭证已下线，保留注释便于回溯" },
  { type: '文本', title: '// 判断当前登录地址如果是 console 的话，就使用conso…', app: 'webstorm', time: '9月22日 14:20', size: '156 B', uses: 0, body: '// 判断当前登录地址如果是 console 的话，就使用 console 专用通道\nconst isConsole = location.hash.includes(\'#/console\')' },
  { type: '图片', title: '图片 · 2636 × 516', app: 'Google Chrome', time: '9月22日 11:08', size: '1.2 MB', uses: 0, body: null },
  { type: '文本', title: 'const getLoginChannel = () => { const env =…', app: 'webstorm', time: '9月22日 10:31', size: '341 B', uses: 0, body: "const getLoginChannel = () => {\n  const env = process.env['VUE_APP_ENV'] || 'gok'\n  return env\n}" },
  { type: '文件', title: '登录渠道接口改造-前端对接说明.pdf', app: 'Finder', time: '9月21日 18:44', size: '860 KB', uses: 0, body: null },
];

const SNIPPETS = [
  { title: 'TeamAI 网关登录渠道', body: "const getLoginChannel = () => {\n  const env = process.env['VUE_APP_ENV'] || 'gok'\n  if (env === 'gw' && window.location.hash === '#/console') return 'gok'\n  return env\n}", tag: '文本', uses: '尚未使用', updated: '9/23 14:33' },
  { title: 'TeamAI 环境说明', body: '// 当前环境渠道：gok（内网）、gw（国网生产）\n// console 统一跟随 gok', tag: '文本', uses: '尚未使用', updated: '9/23 14:33' },
];

const VAULT = [
  { name: 'deepseek', provider: 'DeepSeek', type: 'AI Key', pinned: true, link: true },
  { name: 'Gitlab', provider: 'GokGitLab', type: '其他 Key', pinned: true },
  { name: 'accessToken', provider: 'Npm', type: '其他 Key', pinned: true },
  { name: 'opencode', provider: 'Moonshot (Kimi)', type: 'AI Key', tag: '海外', link: true },
];

const COMMITS = [
  { hash: 'a3f9c21', msg: 'feat(quota): 套餐额度卡片支持多周期进度展示', repo: 'xlt-workbench', time: '今天 16:42', author: 'weipengcheng' },
  { hash: '7b2e8d4', msg: 'fix(sync): 修复 ark 平台同步异常未序列化的问题', repo: 'xlt-workbench', time: '今天 15:10', author: 'weipengcheng' },
  { hash: 'c91f0a6', msg: 'refactor(billing): 拆分订阅列表与账单流水数据层', repo: 'xlt-workbench', time: '今天 11:26', author: 'weipengcheng' },
  { hash: 'e4d7b19', msg: 'feat(heatmap): 活跃热力图支持 UTC+8 时区对齐', repo: 'xlt-workbench', time: '昨天 18:03', author: 'weipengcheng' },
  { hash: '2a6c5f8', msg: 'chore(deps): 升级 tauri 至 2.8.1', repo: 'xlt-workbench', time: '昨天 09:51', author: 'teamai-bot' },
  { hash: 'f83d0e2', msg: 'feat(vault): 密钥库主密码 PBKDF2 迭代次数提升至 60 万', repo: 'xlt-workbench', time: '9/22 17:35', author: 'weipengcheng' },
];

/* ============================================================
   页面 · 概览
   ============================================================ */
function viewOverview() {
  const quotas = QUOTAS.map((q, i) =>
    '<div class="card card-pad hoverable rv" style="--i:' + (i + 4) + '">' +
    '<div class="quota-head">' + svcBadge(q.name, q.color) +
    '<div><div class="svc-name">' + q.name + '</div></div>' +
    '<span class="badge grey plan">' + q.plan + '</span></div>' +
    q.rows.map(r => meterRow(r.label, r.sub, r.pct)).join('') +
    (q.rows.some(r => r.tag) ? '<div class="mt8"><span class="badge red"><span class="dot"></span>周额度已耗尽</span></div>' : '') +
    '</div>').join('');

  return pageHead('OVERVIEW', '概览', '周三 · 2026 年 9 月 24 日',
      '<span class="sync-chip"><span class="dot amber"></span>10 分钟前 · 部分同步</span>' +
      '<button class="btn" onclick="toast(\'已开始同步全部平台\')">' + icon('sync', 14) + '同步全部</button>') +

    '<div class="alert-strip red rv" style="--i:1">' + icon('alert', 16) +
    '<span class="grow"><b>Codex 周额度已耗尽</b>，Qoder 总额度与加购额度均已用完 — 建议切换至火山方舟或 Kimi 继续工作</span>' +
    '<span class="sec-link" onclick="location.hash=\'#/billing\'">查看额度 ' + icon('arrow', 12) + '</span></div>' +

    '<div class="stat-grid g4 mt16">' +
    statCard({ label: '订阅月支出 · 计划', value: '¥602.50', sub: '4 项生效中', spark: [580, 596, 590, 602, 602, 602, 602] }, 2) +
    statCard({ label: '本月实际支出 · 已记', value: '¥948.50', sub: '仅统计账单流水', spark: [120, 288, 320, 349, 493, 592, 948], sparkColor: 'var(--amber)' }, 3) +
    statCard({ label: '7 天 TOKEN', value: '150.1', unit: 'M', delta: '52.5%', deltaDir: 'down', spark: [42, 58, 38, 61, 30, 26, 18] }, 4) +
    statCard({ label: '峰值利用率', value: '100', unit: '%', sub: 'Codex · 周额度', spark: [22, 41, 56, 78, 92, 100, 100], sparkColor: 'var(--red)' }, 5) +
    '</div>' +

    sec('套餐额度', '5 个平台 · 可用额度与重置时间', '全部') +
    '<div class="quota-grid">' + quotas + '</div>' +

    sec('小组件', '可按需配置与排序') +
    '<div class="ov-grid">' +

    '<div class="card card-pad col-5 hoverable rv" style="--i:4">' +
      '<div class="flex" style="justify-content:space-between"><div class="flex" style="gap:8px;color:var(--amber-t)">' + icon('fuel', 16) + '<b style="font-size:13.5px;color:var(--text)">油价监控</b></div><span class="badge grey">福建</span></div>' +
      '<div class="sec-sub mt8">92号汽油 · 下一调价窗口 9/24 24时</div>' +
      '<div class="fuel-main mt16"><div class="fuel-price">¥8.25<span class="unit"> /升</span></div><span class="delta up">' + icon('up', 11, 2.4) + '预计上调 +0.632</span></div>' +
      '<div class="meter warn mt16"><i style="width:86%"></i></div>' +
      '<div class="sec-sub mt8">5 小时后 · 预测幅度 +853 元/吨 · 高置信度</div>' +
      '<div class="fuel-grades">' +
      '<div class="fuel-grade"><div class="g-name">95号汽油</div><div class="g-price">¥8.81</div></div>' +
      '<div class="fuel-grade"><div class="g-name">98号汽油</div><div class="g-price">¥10.31</div></div>' +
      '<div class="fuel-grade"><div class="g-name">0号柴油</div><div class="g-price">¥7.96</div></div></div>' +
    '</div>' +

    '<div class="card card-pad col-4 hoverable rv" style="--i:5">' +
      '<div class="flex" style="gap:8px;color:var(--accent)">' + icon('branch', 16) + '<b style="font-size:13.5px;color:var(--text)">今日工作</b></div>' +
      '<div class="sec-sub mt8">从本机 Git 提交生成日报或周报</div>' +
      '<div class="mt16" style="font-family:var(--mono);font-size:28px;font-weight:600">6 <span style="font-size:13px;color:var(--text-3)">次提交 · 2 个仓库</span></div>' +
      '<div class="hr"></div>' +
      '<div class="clip-item-meta" style="margin-top:0">' + icon('clock', 13) + '最近一次提交 · 今天 16:42</div>' +
      '<button class="btn primary mt16" style="width:100%;justify-content:center" onclick="location.hash=\'#/reports\'">' + icon('sparkle', 14) + '生成今日日报</button>' +
    '</div>' +

    '<div class="card card-pad col-3 hoverable rv" style="--i:6">' +
      '<div class="flex" style="gap:8px;color:var(--text-2)">' + icon('clipboard', 16) + '<b style="font-size:13.5px;color:var(--text)">剪贴板</b></div>' +
      '<div class="mt16" style="font-family:var(--mono);font-size:28px;font-weight:600">35 <span style="font-size:13px;color:var(--text-3)">条 · 过去 7 天</span></div>' +
      '<div class="hr"></div>' +
      '<div class="clip-item-meta" style="margin-top:0"><span class="dot" style="width:6px;height:6px;border-radius:50%;background:var(--accent)"></span>正在记录</div>' +
      '<button class="btn mt16" style="width:100%;justify-content:center" onclick="location.hash=\'#/clipboard\'">查看历史</button>' +
    '</div>' +

    '</div>';
}

/* ============================================================
   页面 · 用量分析
   ============================================================ */
let anaRange = 'week', anaModelTab = 'tokens';

function viewAnalytics() {
  const d = ANALYTICS[anaRange];
  const ranges = [['today', '今天'], ['week', '7 天'], ['month', '30 天'], ['quarter', '90 天']];
  const rangeTabs = '<div class="tabs">' + ranges.map(([k, l]) =>
    '<div class="tab' + (k === anaRange ? ' active' : '') + '" data-range="' + k + '">' + l + '</div>').join('') + '</div>';

  const stats = '<div class="stat-grid g5">' + d.stats.map((s, i) => statCard(s, i + 1)).join('') + '</div>';

  const toolRows = d.byTool.map((t, i) =>
    '<tr class="rv" style="--i:' + (i + 3) + '"><td><div class="svc-row">' + svcBadge(t.name, TOOL_COLORS[t.name]) + '<span class="svc-name">' + t.name + '</span></div></td>' +
    '<td><div class="rank-cell"><div class="rank-bar"><i style="width:' + t.pct + '%;background:' + TOOL_COLORS[t.name] + '"></i></div><span class="rank-pct">' + t.pct + '%</span></div></td>' +
    '<td class="num">' + t.tokens + '</td><td class="num">' + t.cost + '</td></tr>').join('');

  const donutSegs = d.byTool.map(t => ({ pct: t.pct, c: TOOL_COLORS[t.name] }));
  const legend = d.byTool.map(t =>
    '<div class="legend-item"><span class="legend-dot" style="background:' + TOOL_COLORS[t.name] + '"></span>' + t.name +
    '<span class="num">' + t.tokens + '</span><span class="pct">' + t.pct + '%</span></div>').join('');

  const modelRows = d.byModel.map((m, i) =>
    '<tr class="rv" style="--i:' + (i + 3) + '"><td><div class="svc-row"><span class="svc" style="color:var(--accent);background:var(--accent-soft);font-size:10px">AI</span><div><div class="svc-name mono" style="font-size:12.5px">' + m.name + '</div><div class="svc-sub">' + m.tool + '</div></div></div></td>' +
    '<td><div class="rank-cell"><div class="rank-bar"><i style="width:' + m.pct + '%;background:var(--accent)"></i></div><span class="rank-pct">' + m.pct + '%</span></div></td>' +
    '<td class="num">' + m.tokens + '</td><td class="num">' + m.cost + '</td></tr>').join('');

  return pageHead('ANALYTICS', '用量分析', 'Token 消耗、费用与工具分布', rangeTabs) +
    stats +
    '<div class="ov-grid mt16">' +
    '<div class="card card-pad col-7 rv" style="--i:3"><div class="stat-label">Token 趋势 · ' + d.label + '</div><div class="mt16">' + areaChart(d.trend.points, d.trend.labels, 620, 190) + '</div></div>' +
    '<div class="card card-pad col-5 rv" style="--i:4"><div class="stat-label">每日分布 · 按工具</div><div class="mt16">' + stackedBars(d.daily, 420, 190) + '</div></div>' +
    '</div>' +

    '<div class="ov-grid mt16">' +
    '<div class="card col-7 rv" style="--i:4;overflow:hidden"><div class="card-pad" style="padding-bottom:6px"><div class="sec-title">按工具用量</div><div class="sec-sub">' + d.label + ' · 各 AI 工具的 Token 分布</div></div>' +
    '<table class="tbl"><thead><tr><th>工具</th><th>占比</th><th class="r">Token</th><th class="r">费用</th></tr></thead><tbody>' + toolRows + '</tbody></table></div>' +

    '<div class="card card-pad col-5 rv" style="--i:5"><div class="sec-title">工具分布</div><div class="sec-sub">按 AI 工具查看 Token 占比</div>' +
    '<div class="donut-flex mt16"><div style="position:relative">' + donut(donutSegs) +
    '<div style="position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center"><div class="muted" style="font-size:11px">Tokens</div><div class="mono" style="font-size:22px;font-weight:650">' + (anaRange === 'week' ? '150.1M' : d.stats[0].value + 'M') + '</div></div></div>' +
    '<div class="legend grow">' + legend + '</div></div></div>' +
    '</div>' +

    '<div class="card mt16 rv" style="--i:5;overflow:hidden"><div class="card-pad" style="padding-bottom:6px;display:flex;align-items:center;justify-content:space-between"><div><div class="sec-title">按模型用量</div><div class="sec-sub">' + d.label + ' · 各模型的 Token 分布</div></div>' +
    '<div class="tabs"><div class="tab' + (anaModelTab === 'tokens' ? ' active' : '') + '" data-mtab="tokens">Token</div><div class="tab' + (anaModelTab === 'cost' ? ' active' : '') + '" data-mtab="cost">费用</div></div></div>' +
    '<table class="tbl"><thead><tr><th>模型</th><th>占比</th><th class="r">Token</th><th class="r">费用</th></tr></thead><tbody>' + modelRows + '</tbody></table></div>' +

    sec('活跃热力图', '每日 Token 用量分布（近一年）· UTC+08:00') +
    '<div class="card card-pad rv" style="--i:6">' + heatmap() +
    '<div class="flex mt16" style="justify-content:space-between"><span class="muted" style="font-size:11.5px">9 月起用量显著上升</span>' +
    '<div class="hm-legend">少 <span class="hm-c"></span><span class="hm-c hm-1"></span><span class="hm-c hm-2"></span><span class="hm-c hm-3"></span><span class="hm-c hm-4"></span> 多</div></div></div>';
}

function heatmap() {
  const rnd = seeded(20260924);
  const weeks = 53;
  let cells = '';
  for (let w = 0; w < weeks; w++) {
    for (let d = 0; d < 7; d++) {
      let lv = 0;
      const recency = (w - 40) / 13; // 最近 13 周
      if (recency > 0) {
        const r = rnd();
        if (r < 0.3 + recency * 0.5) lv = 1 + Math.floor(rnd() * 4);
      } else if (rnd() < 0.05) lv = 1;
      cells += '<span class="hm-c' + (lv ? ' hm-' + lv : '') + '"></span>';
    }
  }
  const months = ['11月', '12月', '1月', '2月', '3月', '4月', '5月', '6月', '7月', '8月', '9月'];
  const mlabels = months.map((m, i) => '<span style="left:' + (4 + i * 8.4) + '%">' + m + '</span>').join('');
  return '<div class="hm-wrap"><div class="hm-months">' + mlabels + '</div>' +
    '<div class="hm-flex"><div class="hm-days"><i>日</i><i>一</i><i>二</i><i>三</i><i>四</i><i>五</i><i>六</i></div>' +
    '<div class="hm">' + cells + '</div></div></div>';
}

function mountAnalytics() {
  document.querySelectorAll('[data-range]').forEach(el => el.addEventListener('click', () => { anaRange = el.dataset.range; render(); }));
  document.querySelectorAll('[data-mtab]').forEach(el => el.addEventListener('click', () => { anaModelTab = el.dataset.mtab; render(); }));
}

/* ============================================================
   页面 · 费用中心（套餐额度 / 订阅列表 / 账单流水）
   ============================================================ */
let billTab = 'quota';

function viewBilling() {
  const tabs = '<div class="tabs">' +
    [['quota', '套餐额度'], ['subs', '订阅列表'], ['bills', '账单流水']].map(([k, l]) =>
      '<div class="tab' + (k === billTab ? ' active' : '') + '" data-btab="' + k + '">' + l + '</div>').join('') + '</div>';

  let body = '';
  if (billTab === 'quota') {
    body = '<div class="sec-sub rv" style="--i:2;margin-bottom:14px">5 个平台 · 5 个套餐的可用额度与重置时间</div>' +
      '<div class="quota-grid">' + QUOTAS.map((q, i) =>
        '<div class="card card-pad hoverable rv" style="--i:' + (i + 3) + '">' +
        '<div class="quota-head">' + svcBadge(q.name, q.color) +
        '<div class="svc-name">' + q.name + '</div>' +
        '<span class="badge grey plan">' + q.plan + '</span></div>' +
        q.rows.map(r => meterRow(r.label, r.sub, r.pct)).join('') +
        (q.rows.some(r => r.tag) ? '<div class="mt8"><span class="badge red"><span class="dot"></span>周额度已耗尽</span></div>' : '') +
        '</div>').join('') + '</div>';
  } else if (billTab === 'subs') {
    const rows = SUBSCRIPTIONS.map((s, i) =>
      '<tr class="rv" style="--i:' + (i + 3) + '"><td><div class="svc-row">' + svcBadge(s.name, TOOL_COLORS[s.name] || '#8a8a8a') +
      '<div><div class="svc-name">' + s.name + '</div><div class="svc-sub">' + s.plan + '</div></div></div></td>' +
      '<td class="muted mono" style="font-size:12px">' + s.account + '</td>' +
      '<td class="num" style="font-weight:600">' + s.amount + '</td><td class="muted">' + s.cycle + '</td>' +
      '<td><div class="mono" style="font-size:12.5px">' + s.next + '</div><div class="svc-sub">' + s.nextSub + '</div></td>' +
      '<td><span class="badge ' + s.status[0] + '"><span class="dot"></span>' + s.status[1] + '</span></td></tr>').join('');
    body = '<div class="card rv" style="--i:3;overflow:hidden;margin-top:4px"><table class="tbl"><thead><tr><th>服务</th><th>账号</th><th class="r">金额</th><th>周期</th><th>下次续费</th><th>状态</th></tr></thead><tbody>' + rows + '</tbody></table></div>';
  } else {
    const rows = BILLS.map((b, i) =>
      '<tr class="rv" style="--i:' + (i + 3) + '"><td class="mono muted" style="font-size:12.5px">' + b.date + '</td>' +
      '<td><div class="svc-row">' + svcBadge(b.name, TOOL_COLORS[b.name] || '#8a8a8a') + '<div><div class="svc-name">' + b.name + '</div>' + (b.plan ? '<div class="svc-sub">' + b.plan + '</div>' : '') + '</div></div></td>' +
      '<td><span class="badge grey">' + b.cat + '</span></td><td class="muted">' + b.pay + '</td>' +
      '<td><span class="badge ' + b.src[0] + '">' + b.src[1] + '</span></td>' +
      '<td class="num" style="font-weight:600">' + b.amount + (b.cny ? '<div class="svc-sub" style="text-align:right">' + b.cny + '</div>' : '') + '</td></tr>').join('');
    body = '<div class="filter-row rv" style="--i:2;margin-bottom:14px">' +
      '<div class="sel-wrap"><select class="input"><option>2026 年 9 月</option><option>2026 年 8 月</option></select>' + icon('chevron', 14) + '</div>' +
      '<div class="sel-wrap"><select class="input"><option>全部分类</option><option>模型订阅</option><option>API 中转</option></select>' + icon('chevron', 14) + '</div>' +
      '<div class="grow"></div><button class="btn sm" onclick="toast(\'已打开手动记账\')">' + icon('plus', 13) + '手动记账</button></div>' +
      '<div class="card rv" style="--i:3;overflow:hidden"><table class="tbl"><thead><tr><th>日期</th><th>服务</th><th>分类</th><th>支付方式</th><th>来源</th><th class="r">金额</th></tr></thead><tbody>' + rows + '</tbody></table>' +
      '<div style="display:flex;justify-content:space-between;padding:14px 18px;border-top:1px solid var(--border);font-size:13px"><span class="muted">2026 年 9 月合计（截至今日）</span><b class="mono">¥948.50</b></div></div>';
  }

  return pageHead('BILLING', '费用中心', '套餐额度、订阅计划与账单流水统一对账',
      '<span class="sync-chip">1 USD = ¥ 7.2</span>' +
      '<button class="btn primary" onclick="toast(\'已打开新增订阅\')">' + icon('plus', 14) + '新增订阅</button>') +

    '<div class="stat-grid g3">' +
    statCard({ label: '计划月支出', value: '¥602.50', sub: '4 项生效中' }, 1) +
    statCard({ label: '本月实际已记', value: '¥948.50', sub: '仅统计账单流水' }, 2) +
    statCard({ label: '7 天内续费', value: '1', sub: 'Kiro Pro+ · 请核对付款账户' }, 3) +
    '</div>' +

    '<div class="mt24 rv" style="--i:3">' + tabs + '</div><div class="mt16">' + nbody(body) + '</div>';
}
function nbody(b) { return b; }

function mountBilling() {
  document.querySelectorAll('[data-btab]').forEach(el => el.addEventListener('click', () => { billTab = el.dataset.btab; render(); }));
}

/* ============================================================
   页面 · Git 报告
   ============================================================ */
let repType = 'day', repTab = 'preview', repGenerated = false, repLoading = false;

function viewReports() {
  const typeTabs = '<div class="tabs">' +
    '<div class="tab' + (repType === 'day' ? ' active' : '') + '" data-rtype="day">日报</div>' +
    '<div class="tab' + (repType === 'week' ? ' active' : '') + '" data-rtype="week">周报</div></div>';

  const toolbar = '<div class="card card-pad rv" style="--i:1"><div class="report-toolbar">' +
    typeTabs +
    '<div class="sel-wrap"><select class="input"><option>今天</option><option>昨天</option><option>近 7 天</option></select>' + icon('chevron', 14) + '</div>' +
    '<div class="sel-wrap"><select class="input"><option>项目 · 8/8</option><option>xlt-workbench</option><option>teamai-gateway</option></select>' + icon('chevron', 14) + '</div>' +
    '<div class="sel-wrap"><select class="input"><option>作者 · 2 人</option><option>weipengcheng</option><option>teamai-bot</option></select>' + icon('chevron', 14) + '</div>' +
    '<div class="grow"></div>' +
    '<span class="muted" style="font-size:12px">仅读取 Git 记录</span>' +
    '<button class="btn primary" id="genBtn">' + (repLoading ? icon('sync', 14) + '<span class="spin" style="display:inline-flex"></span>生成中…' : icon('arrow', 14) + '生成' + (repType === 'day' ? '日报' : '周报')) + '</button>' +
    '</div></div>';

  const viewTabs = '<div class="tabs">' +
    '<div class="tab' + (repTab === 'preview' ? ' active' : '') + '" data-rtab="preview">报告预览</div>' +
    '<div class="tab' + (repTab === 'commits' ? ' active' : '') + '" data-rtab="commits">Git 记录</div></div>' +
    '<span class="badge grey" style="margin-left:10px">8 个仓库</span>';

  let body = '';
  if (repTab === 'commits') {
    body = '<div class="card rv" style="--i:3;overflow:hidden">' + COMMITS.map(c =>
      '<div class="commit-row"><span class="commit-hash">' + c.hash + '</span>' +
      '<span class="commit-msg">' + esc(c.msg) + '</span>' +
      '<span class="badge grey">' + c.repo + '</span>' +
      '<span class="commit-meta">' + c.author + ' · ' + c.time + '</span></div>').join('') + '</div>';
  } else if (repGenerated) {
    body = '<div class="card card-pad rv" style="--i:3"><div class="report-body">' +
      '<h3>' + icon('sparkle', 15) + ' xlt-workbench · 用量与费用</h3><ul>' +
      '<li>套餐额度卡片重构：支持 5h / 周 / 月多周期进度展示，耗尽状态改为红色预警条</li>' +
      '<li>修复 ark 平台同步异常未序列化导致「部分数据未更新」横幅只显示 [object Object] 的问题</li>' +
      '<li>活跃热力图按 UTC+8 对齐，修复跨时区日期偏移</li></ul>' +
      '<h3>' + icon('sparkle', 15) + ' xlt-workbench · 安全</h3><ul>' +
      '<li>密钥库主密码 PBKDF2 迭代次数提升至 60 万，AES-256-GCM 加密参数升级</li></ul>' +
      '<h3>' + icon('sparkle', 15) + ' 明日计划</h3><ul>' +
      '<li>账单流水支持按月导出 CSV；订阅逾期前 3 天增加本地通知</li></ul>' +
      '</div>' +
      '<div class="hr"></div>' +
      '<div class="flex"><button class="btn sm" onclick="toast(\'报告已复制到剪贴板\')">' + icon('copy', 13) + '复制报告</button>' +
      '<button class="btn sm ghost" onclick="toast(\'已导出 Markdown\')">' + icon('doc', 13) + '导出 Markdown</button></div></div>';
  } else {
    body = '<div class="card rv" style="--i:3"><div class="empty">' + icon('doc', 42, 1.2) +
      '<div class="empty-title">在上方选好条件，点击「生成' + (repType === 'day' ? '日报' : '周报') + '」</div>' +
      '<div class="empty-desc">AI 会读取所选仓库在这段时间内的提交，按项目整理成可以直接复制粘贴的工作报告。</div></div></div>';
  }

  return pageHead('GIT REPORT', 'Git 报告', '从本机提交记录生成日报与周报') +
    toolbar +
    '<div class="flex mt24 rv" style="--i:2">' + viewTabs + '</div>' +
    '<div class="mt16">' + body + '</div>';
}

function mountReports() {
  document.querySelectorAll('[data-rtype]').forEach(el => el.addEventListener('click', () => { repType = el.dataset.rtype; repGenerated = false; render(); }));
  document.querySelectorAll('[data-rtab]').forEach(el => el.addEventListener('click', () => { repTab = el.dataset.rtab; render(); }));
  const btn = document.getElementById('genBtn');
  if (btn) btn.addEventListener('click', () => {
    if (repLoading) return;
    repLoading = true; render();
    setTimeout(() => { repLoading = false; repGenerated = true; repTab = 'preview'; render(); toast('报告已生成'); }, 900);
  });
}

/* ============================================================
   页面 · 剪贴板历史
   ============================================================ */
let clipFilter = '全部', clipSel = 0;

function viewClipboard() {
  const counts = { '全部': 100, '文本': 60, '图片': 29, '文件': 11, '已收藏': 0 };
  const pills = Object.keys(counts).map(k =>
    '<span class="f-pill' + (k === clipFilter ? ' active' : '') + '" data-cf="' + k + '">' +
    (k === '文本' ? icon('doc', 13) : k === '图片' ? icon('image', 13) : k === '文件' ? icon('file', 13) : k === '已收藏' ? icon('pin', 13) : icon('clipboard', 13)) +
    k + ' <span class="cnt">' + counts[k] + '</span></span>').join('');

  const list = CLIPS.filter(c => clipFilter === '全部' || c.type === clipFilter);
  const items = list.map((c, i) =>
    '<div class="clip-item' + (i === clipSel ? ' active' : '') + '" data-clip="' + i + '">' +
    '<div class="clip-item-title">' + esc(c.title) + '</div>' +
    '<div class="clip-item-meta"><span class="badge grey">' + c.type + '</span>' + esc(c.app) + ' · ' + esc(c.time) +
    (c.uses ? '<span style="margin-left:auto">使用 ' + c.uses + ' 次</span>' : '') + '</div></div>').join('');

  const sel = list[clipSel] || list[0];
  const preview = sel ?
    '<div class="card card-pad clip-preview rv" style="--i:3">' +
    '<div class="flex" style="justify-content:space-between"><span class="badge green">' + sel.type + '</span><span class="muted mono" style="font-size:11.5px">' + (clipSel + 1) + ' / ' + list.length + '</span></div>' +
    (sel.body ? '<div class="clip-code mt16">' + esc(sel.body) + '</div>'
      : '<div class="empty" style="padding:44px 20px">' + icon(sel.type === '图片' ? 'image' : 'file', 38, 1.2) + '<div class="empty-desc">' + esc(sel.title) + ' · 原型中不展示二进制内容</div></div>') +
    '<div class="preview-meta"><span>来源<b>' + esc(sel.app) + '</b></span><span>时间<b>' + esc(sel.time) + '</b></span><span>大小<b>' + esc(sel.size) + '</b></span></div>' +
    '<div class="hr"></div>' +
    '<div class="flex"><button class="btn primary sm" onclick="toast(\'已复制内容\')">' + icon('copy', 13) + '复制内容</button>' +
    '<button class="btn sm" onclick="toast(\'已复制纯文本\')">复制纯文本</button>' +
    '<button class="btn sm" onclick="toast(\'已保存为代码片段\')">' + icon('plus', 13) + '保存为片段</button>' +
    '<div class="grow"></div><button class="btn sm ghost" onclick="toast(\'已删除该记录\')">' + icon('trash', 13) + '</button></div></div>'
    : '<div class="card"><div class="empty"><div class="empty-title">该分类下暂无记录</div></div></div>';

  return pageHead('CLIPBOARD', '剪贴板历史', '未收藏保留 3 天 · 上限 100 条',
      '<span class="sync-chip"><span class="dot"></span>正在记录</span>' +
      '<button class="btn" onclick="toast(\'已暂停记录\')">' + icon('pause', 13) + '暂停记录</button>') +
    '<div class="flex rv" style="--i:1"><div class="search">' + icon('search', 15) + '<input class="input" placeholder="搜索内容或来源应用…"><span class="kbd">⌘F</span></div></div>' +
    '<div class="filter-row mt16 rv" style="--i:2">' + pills + '</div>' +
    '<div class="clip-layout mt16"><div class="clip-list rv" style="--i:2">' + (items || '<div class="empty"><div class="empty-desc">该分类下暂无记录</div></div>') + '</div>' + preview + '</div>';
}

function mountClipboard() {
  document.querySelectorAll('[data-cf]').forEach(el => el.addEventListener('click', () => { clipFilter = el.dataset.cf; clipSel = 0; render(); }));
  document.querySelectorAll('[data-clip]').forEach(el => el.addEventListener('click', () => { clipSel = +el.dataset.clip; render(); }));
}

/* ============================================================
   页面 · 代码片段
   ============================================================ */
function viewSnippets() {
  const cards = SNIPPETS.map((s, i) =>
    '<div class="card snip-card hoverable rv" style="--i:' + (i + 3) + '">' +
    '<div class="snip-title"><span class="svc" style="color:var(--accent);background:var(--accent-soft)">' + icon('code', 14) + '</span>' + esc(s.title) +
    '<div class="grow"></div><button class="btn sm ghost" onclick="toast(\'已复制片段\')">' + icon('copy', 13) + '</button></div>' +
    '<div class="snip-body">' + esc(s.body) + '</div>' +
    '<div class="snip-meta"><span class="badge grey">' + s.tag + '</span>' + s.uses + '<span class="grow"></span>更新于 ' + s.updated + '</div></div>').join('');

  return pageHead('SNIPPETS', '代码片段', '点击片段即可复制到剪贴板',
      '<button class="btn primary" onclick="toast(\'已打开新建片段\')">' + icon('plus', 14) + '新建 <span class="kbd" style="background:transparent;border-color:rgba(255,255,255,.25);color:inherit">⌘N</span></button>') +
    '<div class="flex rv" style="--i:1"><div class="search">' + icon('search', 15) + '<input class="input" placeholder="搜索片段标题、内容或标签…"><span class="kbd">⌘K</span></div>' +
    '<div class="tabs"><div class="tab active">全部</div><div class="tab">最近</div><div class="tab">置顶</div></div></div>' +
    '<div class="filter-row mt16 rv" style="--i:2"><span class="muted" style="font-size:12.5px">' + SNIPPETS.length + ' 个片段</span></div>' +
    '<div class="mt16" style="display:flex;flex-direction:column;gap:13px">' + cards + '</div>';
}

/* ============================================================
   页面 · 密钥库
   ============================================================ */
let vaultOpen = false;

function viewVault() {
  if (!vaultOpen) {
    return pageHead('VAULT', '密钥库', '本机加密存储各类 Key 与凭证') +
      '<div class="vault-lock rv" style="--i:1">' +
      '<div class="lock-icon">' + icon('lock', 26) + '</div>' +
      '<div class="page-title" style="font-size:19px">解锁密钥库</div>' +
      '<div class="page-desc">输入主密码以访问本机加密记录</div>' +
      '<div class="vault-form"><input class="input mono" type="password" id="vaultPwd" placeholder="主密码" onkeydown="if(event.key===\'Enter\')unlockVault()">' +
      '<button class="btn primary" onclick="unlockVault()">解锁</button></div>' +
      '<div class="vault-note">' + icon('shield', 13) + 'AES-256-GCM · PBKDF2 · 数据仅保存在本机</div></div>';
  }
  const rows = VAULT.map((v, i) =>
    '<tr class="rv" style="--i:' + (i + 3) + '"><td><div class="svc-row">' + svcBadge(v.name, v.type === 'AI Key' ? '#4F9D69' : '#8a8a8a') +
    '<div><div class="svc-name">' + v.name + (v.pinned ? ' <span style="color:var(--amber-t)">' + icon('pin', 11) + '</span>' : '') + '</div>' +
    '<div class="svc-sub"><span class="badge grey" style="margin-right:5px">' + v.type + '</span>' + (v.tag ? '<span class="badge amber">' + v.tag + '</span>' : '') + '</div></div></div></td>' +
    '<td><div class="svc-name" style="font-size:13px">' + v.provider + '</div>' + (v.link ? '<div class="svc-sub" style="color:var(--accent);cursor:pointer">打开地址 ↗</div>' : '') + '</td>' +
    '<td class="masked">••••••••••••</td>' +
    '<td><div class="flex" style="gap:2px;justify-content:flex-end">' +
    '<button class="btn sm ghost" onclick="toast(\'已显示 Key\')">' + icon('eye', 14) + '</button>' +
    '<button class="btn sm ghost" onclick="toast(\'Key 已复制\')">' + icon('copy', 14) + '</button>' +
    '<button class="btn sm ghost" onclick="toast(\'编辑记录\')">' + icon('edit', 14) + '</button>' +
    '<button class="btn sm ghost" onclick="toast(\'已删除记录\')">' + icon('trash', 14) + '</button></div></td></tr>').join('');

  return pageHead('VAULT', '密钥库', '4 条记录 · AES-256-GCM 加密',
      '<button class="btn" onclick="lockVault()">' + icon('lock', 13) + '锁定</button>' +
      '<button class="btn primary" onclick="toast(\'已打开新增记录\')">' + icon('plus', 14) + '新增记录</button>') +
    '<div class="filter-row rv" style="--i:1">' +
    '<span class="f-pill active">全部 <span class="cnt">4</span></span><span class="f-pill">账号密码 <span class="cnt">0</span></span>' +
    '<span class="f-pill">AI Key <span class="cnt">2</span></span><span class="f-pill">其他 Key <span class="cnt">2</span></span>' +
    '<div class="search" style="max-width:280px">' + icon('search', 14) + '<input class="input" placeholder="搜索名称或账号…"></div></div>' +
    '<div class="filter-row mt16 rv" style="--i:2"><span class="f-pill active">所有标签</span><span class="f-pill">海外</span></div>' +
    '<div class="card mt16 rv" style="--i:3;overflow:hidden"><table class="tbl"><thead><tr><th>名称</th><th>账号 / 服务商</th><th>密码 / Key</th><th class="r">操作</th></tr></thead><tbody>' + rows + '</tbody></table></div>';
}

function unlockVault() {
  const p = document.getElementById('vaultPwd');
  if (p && p.value.length === 0) { p.focus(); p.style.borderColor = 'var(--red)'; return; }
  vaultOpen = true; render(); toast('密钥库已解锁');
}
function lockVault() { vaultOpen = false; render(); toast('密钥库已锁定'); }

/* ============================================================
   页面 · 设置
   ============================================================ */
let setTab = 'general';
const SET_STATE = { platforms: { 'Codex': true, '火山方舟 · Coding Plan': false, '火山方舟 · Agent Plan': true, 'Kiro': true, 'Qoder': true, 'Kimi': true } };

function viewSettings() {
  const tabs = '<div class="tabs">' +
    [['general', '通用'], ['platforms', '平台连接'], ['clipboard', '剪贴板'], ['advanced', '高级']].map(([k, l]) =>
      '<div class="tab' + (k === setTab ? ' active' : '') + '" data-stab="' + k + '">' + l + '</div>').join('') + '</div>';

  let body = '';
  if (setTab === 'general') {
    body =
      '<div class="card set-card rv" style="--i:2"><div class="set-title">' + icon('sun', 16) + '外观</div>' +
      '<div class="set-desc">明暗双主题，跟随原型全局切换</div>' +
      '<div class="mt16"><div class="tabs">' +
      '<div class="tab' + (getTheme() === 'dark' ? ' active' : '') + '" data-theme-set="dark">' + icon('moon', 13) + ' 深色</div>' +
      '<div class="tab' + (getTheme() === 'light' ? ' active' : '') + '" data-theme-set="light">' + icon('sun', 13) + ' 浅色</div></div></div></div>' +

      '<div class="card set-card rv" style="--i:3"><div class="set-title">' + icon('fuel', 16) + '油价监控</div>' +
      '<div class="set-desc">省级指导价、下一调价窗口与国家发改委正式公告</div>' +
      '<div class="mt16"><div class="field-label">油品标号</div>' +
      '<div class="tabs"><div class="tab active">92#</div><div class="tab">95#</div><div class="tab">98#</div><div class="tab">0#</div></div></div>' +
      '<div class="mt16"><div class="field-label">监控省份</div><div class="sel-wrap" style="max-width:320px"><select class="input"><option>福建</option><option>广东</option><option>浙江</option><option>北京</option></select>' + icon('chevron', 14) + '</div></div>' +
      '<div class="mt16"><div class="field-label">极数本源预测 API KEY（可选）</div><input class="input mono" style="max-width:420px" placeholder="匿名额度不足时填写">' +
      '<div class="field-hint">今日指导价与预测分开同步；加油站实际挂牌价可能浮动，正式调价以国家发改委公告为准。</div></div></div>' +

      '<div class="card set-card rv" style="--i:4"><div class="set-title">' + icon('grid', 16) + '套餐额度展示</div>' +
      '<div class="set-desc">只展示你正在订阅或需要关注的平台；隐藏后已采集的数据不会删除</div>' +
      '<div class="set-grid">' + Object.keys(SET_STATE.platforms).map(name =>
        '<div class="check' + (SET_STATE.platforms[name] ? ' on' : '') + '" data-plat="' + name + '"><span class="box">' + icon('check', 12, 2.6) + '</span><span style="font-size:13px;font-weight:500">' + name + '</span></div>').join('') + '</div></div>';
  } else if (setTab === 'platforms') {
    body = '<div class="card set-card rv" style="--i:2"><div class="set-title">' + icon('link', 16) + '平台连接</div>' +
      '<div class="set-desc">连接配置只保存在本机，不写入项目文件</div>' +
      ['Codex', '火山方舟', 'Kiro', 'Qoder', 'Kimi'].map((n, i) =>
        '<div class="set-row"><div class="svc-row">' + svcBadge(n, TOOL_COLORS[n] || '#7bc496') + '<div><div class="svc-name">' + n + '</div><div class="svc-sub">' + (i === 0 ? '已连接 · 10 分钟前同步' : '已连接') + '</div></div></div>' +
        '<div class="flex"><span class="badge green"><span class="dot"></span>正常</span><button class="btn sm" onclick="toast(\'已重新连接 ' + n + '\')">重新连接</button></div></div>').join('') + '</div>';
  } else if (setTab === 'clipboard') {
    body = '<div class="card set-card rv" style="--i:2"><div class="set-title">' + icon('clipboard', 16) + '剪贴板</div>' +
      '<div class="set-desc">监听与保留策略</div>' +
      '<div class="set-row"><div><div class="svc-name">启动时自动记录</div><div class="svc-sub">开机后自动开始监听系统剪贴板</div></div><span class="switch on" data-sw></span></div>' +
      '<div class="set-row"><div><div class="svc-name">忽略密码管理器</div><div class="svc-sub">不记录来自 1Password 等应用的复制内容</div></div><span class="switch on" data-sw></span></div>' +
      '<div class="set-row"><div><div class="svc-name">未收藏保留时长</div><div class="svc-sub">超过时长的未收藏记录自动清理</div></div><div class="sel-wrap"><select class="input"><option>3 天</option><option>7 天</option><option>30 天</option></select>' + icon('chevron', 14) + '</div></div>' +
      '<div class="set-row"><div><div class="svc-name">记录上限</div><div class="svc-sub">达到上限后覆盖最旧的未收藏记录</div></div><div class="sel-wrap"><select class="input"><option>100 条</option><option>300 条</option><option>1000 条</option></select>' + icon('chevron', 14) + '</div></div></div>';
  } else {
    body = '<div class="card set-card rv" style="--i:2"><div class="set-title">' + icon('gear', 16) + '高级</div>' +
      '<div class="set-desc">面向开发者的底层选项</div>' +
      '<div class="set-row"><div><div class="svc-name">开发者模式</div><div class="svc-sub">显示同步日志与原始 API 响应</div></div><span class="switch" data-sw></span></div>' +
      '<div class="set-row"><div><div class="svc-name">数据目录</div><div class="svc-sub mono" style="font-size:11.5px">~/Library/Application Support/xlt-workbench</div></div><button class="btn sm" onclick="toast(\'已在 Finder 中打开\')">' + icon('folder', 13) + '打开</button></div>' +
      '<div class="set-row"><div><div class="svc-name">导出全部数据</div><div class="svc-sub">订阅、账单、片段与设置的 JSON 快照</div></div><button class="btn sm" onclick="toast(\'已导出数据快照\')">导出</button></div>' +
      '<div class="set-row"><div><div class="svc-name" style="color:var(--red-t)">清除全部数据</div><div class="svc-sub">不可恢复，密钥库除外</div></div><button class="btn sm" style="color:var(--red-t);border-color:rgba(208,91,91,.4)" onclick="toast(\'需要二次确认\')">清除</button></div></div>';
  }

  return pageHead('SETTINGS', '设置', '连接配置只保存在本机，不写入项目文件',
      '<button class="btn" onclick="toast(\'已开始同步\')">' + icon('sync', 14) + '立即同步</button>' +
      '<button class="btn primary" onclick="toast(\'设置已保存\')">保存</button>') +
    '<div class="rv" style="--i:1">' + tabs + '</div><div class="mt16">' + body + '</div>';
}

function mountSettings() {
  document.querySelectorAll('[data-stab]').forEach(el => el.addEventListener('click', () => { setTab = el.dataset.stab; render(); }));
  document.querySelectorAll('[data-theme-set]').forEach(el => el.addEventListener('click', () => { setTheme(el.dataset.themeSet); render(); }));
  document.querySelectorAll('[data-plat]').forEach(el => el.addEventListener('click', () => {
    const k = el.dataset.plat; SET_STATE.platforms[k] = !SET_STATE.platforms[k]; render();
  }));
  document.querySelectorAll('[data-sw]').forEach(el => el.addEventListener('click', () => el.classList.toggle('on')));
}

/* ============================================================
   路由渲染
   ============================================================ */
const VIEWS = {
  overview: { fn: viewOverview },
  analytics: { fn: viewAnalytics, mount: mountAnalytics },
  billing: { fn: viewBilling, mount: mountBilling },
  reports: { fn: viewReports, mount: mountReports },
  clipboard: { fn: viewClipboard, mount: mountClipboard },
  snippets: { fn: viewSnippets },
  vault: { fn: viewVault },
  settings: { fn: viewSettings, mount: mountSettings },
};

function render() {
  const r = currentRoute();
  const v = VIEWS[r] || VIEWS.overview;
  document.getElementById('view').innerHTML = v.fn();
  if (v.mount) v.mount();
  renderSidebar();
  document.getElementById('main').scrollTop = 0;
  document.title = 'XLT Workbench — ' + (document.querySelector('.page-title') || {}).textContent;
}

window.addEventListener('hashchange', render);
render();
