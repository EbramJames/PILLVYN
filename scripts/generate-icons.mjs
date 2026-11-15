import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const iconsDir = path.join(rootDir, 'src-tauri', 'icons');
const nsisDir = path.join(rootDir, 'src-tauri', 'nsis');

fs.mkdirSync(iconsDir, { recursive: true });
fs.mkdirSync(nsisDir, { recursive: true });

function clamp(v, min = 0, max = 1) {
  return v < min ? min : v > max ? max : v;
}

function smoothstep(edge0, edge1, x) {
  const t = clamp((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}

function mix(a, b, t) {
  return a + (b - a) * t;
}

function mixColor(c1, c2, t) {
  return [
    mix(c1[0], c2[0], t),
    mix(c1[1], c2[1], t),
    mix(c1[2], c2[2], t),
    mix(c1[3], c2[3], t),
  ];
}

function blendOver(dst, src) {
  const sa = src[3];
  const da = dst[3];
  const outA = sa + da * (1 - sa);
  if (outA <= 1e-6) return [0, 0, 0, 0];
  return [
    (src[0] * sa + dst[0] * da * (1 - sa)) / outA,
    (src[1] * sa + dst[1] * da * (1 - sa)) / outA,
    (src[2] * sa + dst[2] * da * (1 - sa)) / outA,
    outA,
  ];
}

function sdRoundedBox(px, py, bx, by, r) {
  const qx = Math.abs(px) - bx + r;
  const qy = Math.abs(py) - by + r;
  const ox = Math.max(qx, 0);
  const oy = Math.max(qy, 0);
  return Math.hypot(ox, oy) + Math.min(Math.max(qx, qy), 0) - r;
}

function sampleIconPixel(u, v) {
  // Normalized coordinates [-1, 1]
  const x = u * 2 - 1;
  const y = v * 2 - 1;

  // Outer rounded squircle badge
  const dBadge = sdRoundedBox(x, y, 0.90, 0.90, 0.36);
  const badgeAlpha = 1 - smoothstep(-0.005, 0.005, dBadge);
  if (badgeAlpha <= 0.001) return [0, 0, 0, 0];

  // Deep obsidian background gradient + subtle top prism glow
  const radial = clamp(1 - Math.hypot(x, y + 0.35) * 0.72);
  let col = mixColor([9, 12, 20, 1], [22, 29, 48, 1], radial * 0.75);

  // Subtle ambient aurora behind the pill
  const glow = Math.exp(-((x * 1.1) ** 2 + ((y + 0.05) * 2.2) ** 2) * 2.8);
  const aurora = mixColor([68, 125, 255, 0], [45, 224, 198, 1], clamp((x + 0.6) / 1.2));
  col = blendOver(col, [aurora[0], aurora[1], aurora[2], glow * 0.35]);

  // Crisp subtle inner rim
  const rim = smoothstep(-0.024, -0.007, dBadge) * (1 - smoothstep(-0.007, 0.003, dBadge));
  col = blendOver(col, [180, 210, 255, rim * 0.28]);

  // Center floating Dynamic Island capsule
  const dPill = sdRoundedBox(x, y, 0.64, 0.21, 0.21);
  const pillAlpha = 1 - smoothstep(-0.004, 0.004, dPill);
  const pillDropShadow = (1 - smoothstep(-0.01, 0.14, sdRoundedBox(x, y - 0.05, 0.64, 0.21, 0.21))) * 0.55;
  col = blendOver(col, [0, 0, 0, pillDropShadow]);

  if (pillAlpha > 0.001) {
    const gradT = clamp((x + 0.64) / 1.28);
    let pillCol = mixColor([10, 13, 21, 1], [18, 23, 36, 1], clamp((0.21 - y) / 0.42));
    // Luminous glass border around capsule
    const pillEdge = smoothstep(-0.022, -0.005, dPill) * (1 - smoothstep(-0.005, 0.004, dPill));
    const borderTint = mixColor([94, 152, 255, 1], [46, 230, 197, 1], gradT);
    pillCol = blendOver(pillCol, [borderTint[0], borderTint[1], borderTint[2], pillEdge * 0.9]);
    col = blendOver(col, [pillCol[0], pillCol[1], pillCol[2], pillAlpha]);
  }

  // Left live-activity glowing prism orb inside the capsule
  const orbDist = Math.hypot(x + 0.39, y) - 0.085;
  const orbAlpha = 1 - smoothstep(-0.004, 0.004, orbDist);
  if (orbAlpha > 0.001) {
    const orbCol = mixColor([88, 138, 255, 1], [46, 230, 197, 1], clamp((x + 0.48) / 0.18));
    col = blendOver(col, [orbCol[0], orbCol[1], orbCol[2], orbAlpha]);
  }

  // Right animated waveform bars inside the capsule
  const bars = [
    { cx: 0.14, h: 0.075 },
    { cx: 0.24, h: 0.115 },
    { cx: 0.34, h: 0.065 },
    { cx: 0.44, h: 0.095 },
  ];
  for (let i = 0; i < bars.length; i++) {
    const b = bars[i];
    const dBar = sdRoundedBox(x - b.cx, y, 0.026, b.h, 0.025);
    const barAlpha = 1 - smoothstep(-0.004, 0.004, dBar);
    if (barAlpha > 0.001) {
      const barTint = mixColor([124, 165, 255, 1], [86, 238, 210, 1], i / 3);
      col = blendOver(col, [barTint[0], barTint[1], barTint[2], barAlpha * 0.96]);
    }
  }

  return [col[0], col[1], col[2], col[3] * badgeAlpha];
}

function renderRGBA(size, sampler) {
  const buf = Buffer.alloc(size * size * 4);
  const ss = 3; // 3x3 supersampling for smooth anti-aliasing
  for (let py = 0; py < size; py++) {
    for (let px = 0; px < size; px++) {
      let r = 0, g = 0, b = 0, a = 0;
      for (let sy = 0; sy < ss; sy++) {
        for (let sx = 0; sx < ss; sx++) {
          const u = (px + (sx + 0.5) / ss) / size;
          const v = (py + (sy + 0.5) / ss) / size;
          const c = sampler(u, v);
          r += c[0] * c[3];
          g += c[1] * c[3];
          b += c[2] * c[3];
          a += c[3];
        }
      }
      const n = ss * ss;
      const outA = a / n;
      const idx = (py * size + px) * 4;
      if (outA > 1e-6) {
        buf[idx] = Math.round(clamp(r / a, 0, 255));
        buf[idx + 1] = Math.round(clamp(g / a, 0, 255));
        buf[idx + 2] = Math.round(clamp(b / a, 0, 255));
        buf[idx + 3] = Math.round(clamp(outA * 255, 0, 255));
      }
    }
  }
  return buf;
}

const crcTable = new Uint32Array(256);
for (let i = 0; i < 256; i++) {
  let c = i;
  for (let k = 0; k < 8; k++) c = (c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1);
  crcTable[i] = c >>> 0;
}
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function makeChunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, 'ascii');
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}

function encodePNG(width, height, rgba) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0; // filter None
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  const compressed = zlib.deflateSync(raw, { level: 9 });
  return Buffer.concat([
    sig,
    makeChunk('IHDR', ihdr),
    makeChunk('IDAT', compressed),
    makeChunk('IEND', Buffer.alloc(0)),
  ]);
}

function encodeICO(entries) {
  // Embed PNG frames directly inside ICO (supported since Windows Vista+)
  const count = entries.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2); // ICON
  header.writeUInt16LE(count, 4);

  const dir = Buffer.alloc(count * 16);
  let offset = 6 + count * 16;
  const payloads = [];

  for (let i = 0; i < count; i++) {
    const { size, png } = entries[i];
    const base = i * 16;
    dir[base] = size >= 256 ? 0 : size;
    dir[base + 1] = size >= 256 ? 0 : size;
    dir[base + 2] = 0;
    dir[base + 3] = 0;
    dir.writeUInt16LE(1, base + 4);
    dir.writeUInt16LE(32, base + 6);
    dir.writeUInt32LE(png.length, base + 8);
    dir.writeUInt32LE(offset, base + 12);
    offset += png.length;
    payloads.push(png);
  }

  return Buffer.concat([header, dir, ...payloads]);
}

function encodeBMP24(width, height, rgbSampler) {
  const rowSize = Math.ceil((width * 3) / 4) * 4;
  const imgSize = rowSize * height;
  const fileSize = 54 + imgSize;
  const buf = Buffer.alloc(fileSize);

  buf.write('BM', 0, 'ascii');
  buf.writeUInt32LE(fileSize, 2);
  buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(width, 18);
  buf.writeInt32LE(height, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(imgSize, 34);

  for (let y = 0; y < height; y++) {
    const bmpY = height - 1 - y; // bottom-up
    for (let x = 0; x < width; x++) {
      const [r, g, b] = rgbSampler(x / width, y / height);
      const idx = 54 + bmpY * rowSize + x * 3;
      buf[idx] = Math.round(clamp(b, 0, 255));
      buf[idx + 1] = Math.round(clamp(g, 0, 255));
      buf[idx + 2] = Math.round(clamp(r, 0, 255));
    }
  }
  return buf;
}

const pngSizes = [16, 24, 32, 48, 64, 128, 256, 512];
const pngBySize = new Map();
for (const size of pngSizes) {
  const rgba = renderRGBA(size, sampleIconPixel);
  pngBySize.set(size, encodePNG(size, size, rgba));
}

fs.writeFileSync(path.join(iconsDir, '32x32.png'), pngBySize.get(32));
fs.writeFileSync(path.join(iconsDir, '128x128.png'), pngBySize.get(128));
fs.writeFileSync(path.join(iconsDir, '128x128@2x.png'), pngBySize.get(256));
fs.writeFileSync(path.join(iconsDir, 'icon.png'), pngBySize.get(512));

const ico = encodeICO([
  { size: 16, png: pngBySize.get(16) },
  { size: 24, png: pngBySize.get(24) },
  { size: 32, png: pngBySize.get(32) },
  { size: 48, png: pngBySize.get(48) },
  { size: 64, png: pngBySize.get(64) },
  { size: 128, png: pngBySize.get(128) },
  { size: 256, png: pngBySize.get(256) },
]);
fs.writeFileSync(path.join(iconsDir, 'icon.ico'), ico);

// NSIS header banner (150x57) and welcome sidebar (164x314)
const headerBmp = encodeBMP24(150, 57, (u, v) => {
  const bg = mixColor([11, 14, 23, 1], [23, 31, 52, 1], u * 0.7 + (1 - v) * 0.3);
  const iconSize = 0.76;
  const iconU = (u - 0.76) / (iconSize * (57 / 150)) + 0.5;
  const iconV = (v - 0.5) / iconSize + 0.5;
  if (iconU >= 0 && iconU <= 1 && iconV >= 0 && iconV <= 1) {
    const px = sampleIconPixel(iconU, iconV);
    const out = blendOver(bg, px);
    return [out[0], out[1], out[2]];
  }
  return [bg[0], bg[1], bg[2]];
});
fs.writeFileSync(path.join(nsisDir, 'header.bmp'), headerBmp);

const sidebarBmp = encodeBMP24(164, 314, (u, v) => {
  let bg = mixColor([9, 12, 20, 1], [19, 27, 47, 1], 1 - v);
  const glow = Math.exp(-((u - 0.5) ** 2 * 4 + (v - 0.34) ** 2 * 6));
  bg = blendOver(bg, [62, 130, 255, glow * 0.35]);
  const iconU = (u - 0.5) / 0.62 + 0.5;
  const iconV = (v - 0.34) / (0.62 * (164 / 314)) + 0.5;
  if (iconU >= 0 && iconU <= 1 && iconV >= 0 && iconV <= 1) {
    const px = sampleIconPixel(iconU, iconV);
    bg = blendOver(bg, px);
  }
  return [bg[0], bg[1], bg[2]];
});
fs.writeFileSync(path.join(nsisDir, 'sidebar.bmp'), sidebarBmp);

console.log('[PILLVYN] Generated icons (.png, .ico) and NSIS branding bitmaps.');
