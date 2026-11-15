// Safe DOM construction, formatting helpers, and crisp inline SVG icons.

export function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === null || v === undefined || v === false) continue;
    if (k === 'class') node.className = v;
    else if (k === 'text') node.textContent = String(v);
    else if (k.startsWith('on') && typeof v === 'function') {
      node.addEventListener(k.slice(2).toLowerCase(), v);
    } else if (k === 'style' && typeof v === 'object') {
      Object.assign(node.style, v);
    } else {
      node.setAttribute(k, String(v));
    }
  }
  const arr = Array.isArray(children) ? children : [children];
  for (const child of arr) {
    if (child === null || child === undefined || child === false) continue;
    if (typeof child === 'string' || typeof child === 'number') {
      node.appendChild(document.createTextNode(String(child)));
    } else {
      node.appendChild(child);
    }
  }
  return node;
}

export function clear(node) {
  while (node.firstChild) node.removeChild(node.firstChild);
}

export function formatTimeShort(date = new Date(), use24h = true) {
  return date.toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    hour12: !use24h,
  });
}

export function formatDuration(seconds) {
  const s = Math.max(0, Math.floor(Number(seconds) || 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}`;
  return `${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}`;
}

export function formatBytes(bytes) {
  const b = Number(bytes) || 0;
  if (b >= 1024 ** 3) return `${(b / 1024 ** 3).toFixed(1)} GB`;
  if (b >= 1024 ** 2) return `${(b / 1024 ** 2).toFixed(1)} MB`;
  if (b >= 1024) return `${(b / 1024).toFixed(0)} KB`;
  return `${Math.round(b)} B`;
}

export function formatSpeed(bps) {
  const v = Number(bps) || 0;
  if (v >= 1024 ** 2) return `${(v / 1024 ** 2).toFixed(1)} MB/s`;
  if (v >= 1024) return `${(v / 1024).toFixed(0)} KB/s`;
  return `${Math.round(v)} B/s`;
}

export function formatRelativeTime(tsMs) {
  const diff = Math.max(0, Math.floor((Date.now() - (Number(tsMs) || 0)) / 1000));
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
}

const SVG_NS = 'http://www.w3.org/2000/svg';

const ICON_PATHS = {
  music: ['M9 18V5l12-2v13', 'M6 18a3 3 0 1 0 0 .01', 'M18 16a3 3 0 1 0 0 .01'],
  play: ['M6 4l14 8-14 8V4z'],
  pause: ['M6 4h4v16H6z', 'M14 4h4v16h-4z'],
  prev: ['M19 20L9 12l10-8v16z', 'M5 19V5'],
  next: ['M5 4l10 8-10 8V4z', 'M19 5v14'],
  volume: ['M11 5L6 9H2v6h4l5 4V5z', 'M15.54 8.46a5 5 0 0 1 0 7.07', 'M19.07 4.93a10 10 0 0 1 0 14.14'],
  mute: ['M11 5L6 9H2v6h4l5 4V5z', 'M23 9l-6 6', 'M17 9l6 6'],
  mic: ['M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z', 'M19 10v2a7 7 0 0 1-14 0v-2', 'M12 19v4', 'M8 23h8'],
  sun: ['M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10z', 'M12 1v2', 'M12 21v2', 'M4.22 4.22l1.42 1.42', 'M18.36 18.36l1.42 1.42', 'M1 12h2', 'M21 12h2', 'M4.22 19.78l1.42-1.42', 'M18.36 5.64l1.42-1.42'],
  cpu: ['M9 9h6v6H9z', 'M4 4h16v16H4z', 'M9 1v3', 'M15 1v3', 'M9 20v3', 'M15 20v3', 'M1 9h3', 'M1 14h3', 'M20 9h3', 'M20 14h3'],
  gamepad: ['M6 12h4', 'M8 10v4', 'M15 13h.01', 'M18 11h.01', 'M17.32 5H6.68a4 4 0 0 0-3.978 3.59l-.687 6.866A3 3 0 0 0 5 18.75a3 3 0 0 0 2.55-1.42L9 15h6l1.45 2.33A3 3 0 0 0 19 18.75a3 3 0 0 0 2.976-3.294l-.687-6.866A4 4 0 0 0 17.32 5z'],
  download: ['M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4', 'M7 10l5 5 5-5', 'M12 15V3'],
  clipboard: ['M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2', 'M8 2h8v4H8z'],
  wifi: ['M5 12.55a11 11 0 0 1 14.08 0', 'M1.42 9a16 16 0 0 1 21.16 0', 'M8.53 16.11a6 6 0 0 1 6.95 0', 'M12 20h.01'],
  bell: ['M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9', 'M13.73 21a2 2 0 0 1-3.46 0'],
  bellOff: ['M13.73 21a2 2 0 0 1-3.46 0', 'M18.63 13A17.89 17.89 0 0 1 18 8', 'M6.26 6.26A5.86 5.86 0 0 0 6 8c0 7-3 9-3 9h14', 'M18 8a6 6 0 0 0-9.33-5', 'M1 1l22 22'],
  zap: ['M13 2L3 14h9l-1 8 10-12h-9l1-8z'],
  clock: ['M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z', 'M12 6v6l4 2'],
  grid: ['M3 3h7v7H3z', 'M14 3h7v7h-7z', 'M14 14h7v7h-7z', 'M3 14h7v7H3z'],
  sliders: ['M4 21v-7', 'M4 10V3', 'M12 21v-9', 'M12 8V3', 'M20 21v-5', 'M20 12V3', 'M1 14h6', 'M9 8h6', 'M17 16h6'],
  settings: ['M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z', 'M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z'],
  minimize: ['M5 12h14'],
  chevronUp: ['M18 15l-6-6-6 6'],
  moon: ['M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z'],
  plug: ['M12 22v-5', 'M9 8V2', 'M15 8V2', 'M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8h12z'],
  sparkles: ['M12 3l1.9 5.8L20 11l-6.1 2.2L12 19l-1.9-5.8L4 11l6.1-2.2L12 3z'],
  camera: ['M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z', 'M12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8z'],
  terminal: ['M4 17l6-6-6-6', 'M12 19h8'],
  activity: ['M22 12h-4l-3 9L9 3l-3 9H2'],
  hash: ['M4 9h16', 'M4 15h16', 'M10 3L8 21', 'M16 3l-2 18'],
  shield: ['M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z'],
  trash: ['M3 6h18', 'M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2'],
  pin: ['M12 17v5', 'M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z'],
  home: ['M3 10.5L12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1v-9.5z'],
  close: ['M18 6L6 18', 'M6 6l12 12'],
  check: ['M20 6L9 17l-5-5'],
  chevronLeft: ['M15 18l-6-6 6-6'],
  chevronRight: ['M9 18l6-6-6-6'],
  layers: ['M12 2L2 7l10 5 10-5-10-5z', 'M2 17l10 5 10-5', 'M2 12l10 5 10-5'],
  battery: ['M17 7H3a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2z', 'M23 11v2'],
  globe: ['M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z', 'M2 12h20', 'M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z'],
};

export function pillvynMark(size = 15) {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', String(size));
  svg.setAttribute('height', String(size));
  svg.setAttribute('fill', 'none');
  const p = document.createElementNS(SVG_NS, 'path');
  p.setAttribute('d', 'M7.5 19.5V5.5h6.2a4.8 4.8 0 0 1 0 9.6H10.5');
  p.setAttribute('stroke', 'currentColor');
  p.setAttribute('stroke-width', '2.1');
  p.setAttribute('stroke-linecap', 'round');
  p.setAttribute('stroke-linejoin', 'round');
  svg.appendChild(p);
  return svg;
}

export function icon(name, size = 14) {
  if (name === 'brand') return pillvynMark(size);
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', String(size));
  svg.setAttribute('height', String(size));
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '1.65');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  const paths = ICON_PATHS[name] || ICON_PATHS.sparkles;
  for (const d of paths) {
    const p = document.createElementNS(SVG_NS, 'path');
    p.setAttribute('d', d);
    svg.appendChild(p);
  }
  return svg;
}
