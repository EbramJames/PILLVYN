import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const targetDir = path.join(rootDir, 'src-tauri', 'target');
const distDir = path.join(rootDir, 'dist');
const cleanAll = process.argv.includes('--all');
const pkg = JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8'));
const currentVersion = pkg.version;

function dirSizeBytes(p) {
  if (!fs.existsSync(p)) return 0;
  const stat = fs.lstatSync(p);
  if (!stat.isDirectory()) return stat.size;
  let total = 0;
  for (const entry of fs.readdirSync(p, { withFileTypes: true })) {
    total += dirSizeBytes(path.join(p, entry.name));
  }
  return total;
}

function removePath(p) {
  if (!fs.existsSync(p)) return 0;
  const bytes = dirSizeBytes(p);
  fs.rmSync(p, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  return bytes;
}

function formatMB(bytes) {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

let reclaimed = 0;

if (cleanAll) {
  reclaimed += removePath(distDir);
  reclaimed += removePath(targetDir);
  console.log(`[PILLVYN] Removed dist/ and src-tauri/target/ (reclaimed ${formatMB(reclaimed)}).`);
} else {
  reclaimed += removePath(path.join(targetDir, 'debug'));
  reclaimed += removePath(path.join(targetDir, 'tmp'));
  const releaseDir = path.join(targetDir, 'release');
  if (fs.existsSync(releaseDir)) {
    for (const sub of ['.fingerprint', 'build', 'deps', 'examples', 'incremental', 'nsis']) {
      reclaimed += removePath(path.join(releaseDir, sub));
    }
    for (const entry of fs.readdirSync(releaseDir, { withFileTypes: true })) {
      if (
        entry.isFile() &&
        (entry.name.endsWith('.pdb') ||
          entry.name.endsWith('.d') ||
          entry.name.endsWith('.lib') ||
          entry.name.endsWith('.rlib') ||
          entry.name.endsWith('.exp'))
      ) {
        reclaimed += removePath(path.join(releaseDir, entry.name));
      }
    }
    const nsisBundleDir = path.join(releaseDir, 'bundle', 'nsis');
    if (fs.existsSync(nsisBundleDir)) {
      const keepPrefix = `PILLVYN_${currentVersion}_`;
      for (const entry of fs.readdirSync(nsisBundleDir, { withFileTypes: true })) {
        if (entry.isFile() && !entry.name.startsWith(keepPrefix)) {
          reclaimed += removePath(path.join(nsisBundleDir, entry.name));
        }
      }
    }
  }
  console.log(
    `[PILLVYN] Pruned Cargo build caches & superseded installers while keeping v${currentVersion} release binaries (reclaimed ${formatMB(reclaimed)}).`
  );
}
