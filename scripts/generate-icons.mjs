import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const sourceLogoPath = path.join(rootDir, 'docs', 'branding', 'pillvyn-logo.png');
const iconsDir = path.join(rootDir, 'src-tauri', 'icons');
const nsisDir = path.join(rootDir, 'src-tauri', 'nsis');
const docsIconPath = path.join(rootDir, 'docs', 'icon.png');
const webIconPath = path.join(rootDir, 'docs', 'website', 'assets', 'icon.png');
const srcAssetsDir = path.join(rootDir, 'src', 'assets');

for (const dir of [iconsDir, nsisDir, path.dirname(webIconPath), srcAssetsDir]) {
  fs.mkdirSync(dir, { recursive: true });
}

function clamp(v, min = 0, max = 1) {
  return v < min ? min : v > max ? max : v;
}

function paeth(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}

function decodePNG(filePath) {
  const buf = fs.readFileSync(filePath);
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (!buf.subarray(0, 8).equals(sig)) {
    throw new Error(`Invalid PNG signature in ${filePath}`);
  }

  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  const idatChunks = [];
  let offset = 8;

  while (offset < buf.length) {
    const len = buf.readUInt32BE(offset);
    const type = buf.subarray(offset + 4, offset + 8).toString('ascii');
    const dataStart = offset + 8;
    const dataEnd = dataStart + len;
    if (type === 'IHDR') {
      width = buf.readUInt32BE(dataStart);
      height = buf.readUInt32BE(dataStart + 4);
      bitDepth = buf[dataStart + 8];
      colorType = buf[dataStart + 9];
      const interlace = buf[dataStart + 12];
      if (bitDepth !== 8 || (colorType !== 2 && colorType !== 6) || interlace !== 0) {
        throw new Error(`Unsupported PNG format in ${filePath} (depth=${bitDepth}, type=${colorType}, interlace=${interlace})`);
      }
    } else if (type === 'IDAT') {
      idatChunks.push(buf.subarray(dataStart, dataEnd));
    } else if (type === 'IEND') {
      break;
    }
    offset = dataEnd + 4;
  }

  const raw = zlib.inflateSync(Buffer.concat(idatChunks));
  const bpp = colorType === 6 ? 4 : 3;
  const stride = width * bpp;
  const unfilt = Buffer.alloc(height * stride);

  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const srcRow = y * (stride + 1) + 1;
    const dstRow = y * stride;
    const prevRow = (y - 1) * stride;

    for (let x = 0; x < stride; x++) {
      const val = raw[srcRow + x];
      const a = x >= bpp ? unfilt[dstRow + x - bpp] : 0;
      const b = y > 0 ? unfilt[prevRow + x] : 0;
      const c = y > 0 && x >= bpp ? unfilt[prevRow + x - bpp] : 0;

      let recon = val;
      if (filter === 1) recon = (val + a) & 0xff;
      else if (filter === 2) recon = (val + b) & 0xff;
      else if (filter === 3) recon = (val + ((a + b) >> 1)) & 0xff;
      else if (filter === 4) recon = (val + paeth(a, b, c)) & 0xff;
      unfilt[dstRow + x] = recon;
    }
  }

  const rgba = Buffer.alloc(width * height * 4);
  for (let i = 0, p = 0; i < width * height; i++, p += bpp) {
    rgba[i * 4] = unfilt[p];
    rgba[i * 4 + 1] = unfilt[p + 1];
    rgba[i * 4 + 2] = unfilt[p + 2];
    rgba[i * 4 + 3] = bpp === 4 ? unfilt[p + 3] : 255;
  }
  return { width, height, rgba };
}

function extractCleanSquircleIcon(src) {
  const { width: w, height: h, rgba } = src;
  const alphaMask = new Uint8Array(w * h);
  alphaMask.fill(255);

  // Check if outer corners already carry transparency
  const cornerAlpha = rgba[3];
  if (cornerAlpha === 255) {
    // Exterior flood-fill from image borders across near-black pixels (<= 14 luminance)
    const visited = new Uint8Array(w * h);
    const queue = new Int32Array(w * h);
    let head = 0;
    let tail = 0;

    const tryEnqueue = (x, y) => {
      const idx = y * w + x;
      if (visited[idx]) return;
      const p = idx * 4;
      const lum = Math.max(rgba[p], rgba[p + 1], rgba[p + 2]);
      if (lum <= 14) {
        visited[idx] = 1;
        alphaMask[idx] = 0;
        queue[tail++] = idx;
      }
    };

    for (let x = 0; x < w; x++) {
      tryEnqueue(x, 0);
      tryEnqueue(x, h - 1);
    }
    for (let y = 0; y < h; y++) {
      tryEnqueue(0, y);
      tryEnqueue(w - 1, y);
    }

    while (head < tail) {
      const idx = queue[head++];
      const x = idx % w;
      const y = (idx / w) | 0;
      if (x > 0) tryEnqueue(x - 1, y);
      if (x + 1 < w) tryEnqueue(x + 1, y);
      if (y > 0) tryEnqueue(x, y - 1);
      if (y + 1 < h) tryEnqueue(x, y + 1);
    }

    // Softly feather 2px perimeter ring for clean edge anti-aliasing over any desktop theme
    const feathered = new Uint8Array(alphaMask);
    for (let y = 1; y < h - 1; y++) {
      for (let x = 1; x < w - 1; x++) {
        const idx = y * w + x;
        if (alphaMask[idx] === 0) continue;
        let zeroNeighbors = 0;
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (alphaMask[(y + dy) * w + (x + dx)] === 0) zeroNeighbors++;
          }
        }
        if (zeroNeighbors > 0) {
          const coverage = 1 - zeroNeighbors / 10;
          const p = idx * 4;
          const lum = Math.max(rgba[p], rgba[p + 1], rgba[p + 2]);
          feathered[idx] = Math.round(clamp(coverage * Math.min(1, lum / 22), 0.12, 1) * 255);
        }
      }
    }
    for (let i = 0; i < w * h; i++) {
      rgba[i * 4 + 3] = feathered[i];
    }
  }

  // Locate tight square bounding box of the visible squircle icon
  let minX = w;
  let minY = h;
  let maxX = 0;
  let maxY = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (rgba[(y * w + x) * 4 + 3] > 20) {
        if (x < minX) minX = x;
        if (y < minY) minY = y;
        if (x > maxX) maxX = x;
        if (y > maxY) maxY = y;
      }
    }
  }

  const bw = maxX - minX + 1;
  const bh = maxY - minY + 1;
  const side = Math.round(Math.max(bw, bh) * 1.05); // slight margin for clean Windows shell padding
  const cx = (minX + maxX) * 0.5;
  const cy = (minY + maxY) * 0.5;
  const sx0 = Math.round(cx - side * 0.5);
  const sy0 = Math.round(cy - side * 0.5);

  const cropped = Buffer.alloc(side * side * 4);
  for (let y = 0; y < side; y++) {
    const sy = sy0 + y;
    if (sy < 0 || sy >= h) continue;
    for (let x = 0; x < side; x++) {
      const sx = sx0 + x;
      if (sx < 0 || sx >= w) continue;
      const srcIdx = (sy * w + sx) * 4;
      const dstIdx = (y * side + x) * 4;
      cropped[dstIdx] = rgba[srcIdx];
      cropped[dstIdx + 1] = rgba[srcIdx + 1];
      cropped[dstIdx + 2] = rgba[srcIdx + 2];
      cropped[dstIdx + 3] = rgba[srcIdx + 3];
    }
  }
  return { width: side, height: side, rgba: cropped };
}

function resizePremultipliedBox(src, targetSize) {
  const { width: sw, height: sh, rgba: srcBuf } = src;
  const out = Buffer.alloc(targetSize * targetSize * 4);
  const scaleX = sw / targetSize;
  const scaleY = sh / targetSize;

  for (let dy = 0; dy < targetSize; dy++) {
    const sy0 = dy * scaleY;
    const sy1 = (dy + 1) * scaleY;
    const iy0 = Math.max(0, Math.floor(sy0));
    const iy1 = Math.min(sh - 1, Math.floor(sy1));

    for (let dx = 0; dx < targetSize; dx++) {
      const sx0 = dx * scaleX;
      const sx1 = (dx + 1) * scaleX;
      const ix0 = Math.max(0, Math.floor(sx0));
      const ix1 = Math.min(sw - 1, Math.floor(sx1));

      let pr = 0;
      let pg = 0;
      let pb = 0;
      let pa = 0;
      let weightSum = 0;

      for (let sy = iy0; sy <= iy1; sy++) {
        const wy = Math.min(sy + 1, sy1) - Math.max(sy, sy0);
        if (wy <= 0) continue;
        for (let sx = ix0; sx <= ix1; sx++) {
          const wx = Math.min(sx + 1, sx1) - Math.max(sx, sx0);
          if (wx <= 0) continue;
          const w = wx * wy;
          const idx = (sy * sw + sx) * 4;
          const a = srcBuf[idx + 3] / 255;
          pr += srcBuf[idx] * a * w;
          pg += srcBuf[idx + 1] * a * w;
          pb += srcBuf[idx + 2] * a * w;
          pa += a * w;
          weightSum += w;
        }
      }

      const dstIdx = (dy * targetSize + dx) * 4;
      if (weightSum > 0 && pa > 1e-6) {
        const avgA = pa / weightSum;
        out[dstIdx] = Math.round(clamp(pr / pa, 0, 255));
        out[dstIdx + 1] = Math.round(clamp(pg / pa, 0, 255));
        out[dstIdx + 2] = Math.round(clamp(pb / pa, 0, 255));
        out[dstIdx + 3] = Math.round(clamp(avgA * 255, 0, 255));
      }
    }
  }
  return out;
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
  ihdr[8] = 8;
  ihdr[9] = 6;

  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0;
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
  const count = entries.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(count, 4);

  const dir = Buffer.alloc(count * 16);
  let offset = 6 + count * 16;
  const payloads = [];

  for (let i = 0; i < count; i++) {
    const { size, png } = entries[i];
    const base = i * 16;
    dir[base] = size >= 256 ? 0 : size;
    dir[base + 1] = size >= 256 ? 0 : size;
    dir.writeUInt16LE(1, base + 4);
    dir.writeUInt16LE(32, base + 6);
    dir.writeUInt32LE(png.length, base + 8);
    dir.writeUInt32LE(offset, base + 12);
    offset += png.length;
    payloads.push(png);
  }

  return Buffer.concat([header, dir, ...payloads]);
}

function compositeOverRGB(dst, srcRgba, srcSize, dx, dy) {
  if (dx < 0 || dy < 0 || dx >= srcSize || dy >= srcSize) return dst;
  const idx = (dy * srcSize + dx) * 4;
  const a = srcRgba[idx + 3] / 255;
  if (a <= 0.001) return dst;
  return [
    srcRgba[idx] * a + dst[0] * (1 - a),
    srcRgba[idx + 1] * a + dst[1] * (1 - a),
    srcRgba[idx + 2] * a + dst[2] * (1 - a),
  ];
}

function encodeBMP24(width, height, pixelFn) {
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
    const bmpY = height - 1 - y;
    for (let x = 0; x < width; x++) {
      const [r, g, b] = pixelFn(x, y);
      const idx = 54 + bmpY * rowSize + x * 3;
      buf[idx] = Math.round(clamp(b, 0, 255));
      buf[idx + 1] = Math.round(clamp(g, 0, 255));
      buf[idx + 2] = Math.round(clamp(r, 0, 255));
    }
  }
  return buf;
}

if (!fs.existsSync(sourceLogoPath)) {
  throw new Error(`Missing primary branding image at ${sourceLogoPath}`);
}

const decoded = decodePNG(sourceLogoPath);
const master = extractCleanSquircleIcon(decoded);

const pngSizes = [16, 24, 32, 48, 64, 128, 256, 512];
const rgbaBySize = new Map();
const pngBySize = new Map();
for (const size of pngSizes) {
  const rgba = resizePremultipliedBox(master, size);
  rgbaBySize.set(size, rgba);
  pngBySize.set(size, encodePNG(size, size, rgba));
}

fs.writeFileSync(path.join(iconsDir, '32x32.png'), pngBySize.get(32));
fs.writeFileSync(path.join(iconsDir, '128x128.png'), pngBySize.get(128));
fs.writeFileSync(path.join(iconsDir, '128x128@2x.png'), pngBySize.get(256));
fs.writeFileSync(path.join(iconsDir, 'icon.png'), pngBySize.get(512));
fs.writeFileSync(docsIconPath, pngBySize.get(256));
fs.writeFileSync(webIconPath, pngBySize.get(256));
fs.writeFileSync(path.join(srcAssetsDir, 'icon.png'), pngBySize.get(128));

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

// NSIS header banner (150x57) — clean dark obsidian surface with monochrome pillvyn-logo
const headerIconSize = 44;
const headerIconRgba = resizePremultipliedBox(master, headerIconSize);
const headerBmp = encodeBMP24(150, 57, (x, y) => {
  const t = (x / 150) * 0.65 + (1 - y / 57) * 0.35;
  let col = [11 + t * 9, 13 + t * 10, 18 + t * 12];
  if (y === 56) col = [34, 38, 48];
  const ix = x - (150 - headerIconSize - 10);
  const iy = y - Math.floor((57 - headerIconSize) / 2);
  return compositeOverRGB(col, headerIconRgba, headerIconSize, ix, iy);
});
fs.writeFileSync(path.join(nsisDir, 'header.bmp'), headerBmp);

// NSIS wizard welcome/finish sidebar (164x314)
const sidebarIconSize = 104;
const sidebarIconRgba = resizePremultipliedBox(master, sidebarIconSize);
const sidebarBmp = encodeBMP24(164, 314, (x, y) => {
  const u = x / 164;
  const v = y / 314;
  const glow = Math.exp(-((u - 0.5) ** 2 * 5.0 + (v - 0.33) ** 2 * 8.0));
  let col = [
    9 + (1 - v) * 8 + glow * 16,
    11 + (1 - v) * 9 + glow * 18,
    16 + (1 - v) * 12 + glow * 24,
  ];
  const ix = x - Math.floor((164 - sidebarIconSize) / 2);
  const iy = y - 48;
  return compositeOverRGB(col, sidebarIconRgba, sidebarIconSize, ix, iy);
});
fs.writeFileSync(path.join(nsisDir, 'sidebar.bmp'), sidebarBmp);

console.log('[PILLVYN] Generated all v1.1.0 icons, tray PNGs, ICO, website assets, and NSIS branding from pillvyn-logo.png.');
