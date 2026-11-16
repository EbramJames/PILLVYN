import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const srcDir = path.join(rootDir, 'src');

function collectFiles(dir, extFilter) {
  const results = [];
  if (!fs.existsSync(dir)) return results;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...collectFiles(full, extFilter));
    } else if (!extFilter || full.endsWith(extFilter)) {
      results.push(full);
    }
  }
  return results;
}

const required = [
  'index.html',
  'settings.html',
  'tray.html',
  'styles/tokens.css',
  'styles/island.css',
  'styles/modules.css',
  'styles/settings.css',
  'styles/tray.css',
  'js/main.js',
  'js/emoji.js',
  'js/effects.js',
  'js/tray.js',
  'js/theme.js',
  'assets/fonts/inter-latin-wght-normal.woff2',
  'assets/fonts/inter-latin-ext-wght-normal.woff2',
  'assets/fonts/inter-cyrillic-wght-normal.woff2',
  'assets/fonts/jetbrains-mono-latin-wght-normal.woff2',
];
for (const rel of required) {
  const full = path.join(srcDir, rel);
  if (!fs.existsSync(full)) {
    console.error(`Missing required frontend file: ${rel}`);
    process.exit(1);
  }
}

const jsFiles = collectFiles(srcDir, '.js');
for (const file of jsFiles) {
  execFileSync(process.execPath, ['--check', file], { stdio: 'inherit' });
}

console.log(`[PILLVYN] Frontend syntax check passed (${jsFiles.length} JS modules verified).`);
