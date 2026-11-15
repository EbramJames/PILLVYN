import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const srcDir = path.join(rootDir, 'src');
const distDir = path.join(rootDir, 'dist');

execFileSync(process.execPath, [path.join(__dirname, 'check-frontend.mjs')], { stdio: 'inherit' });

fs.rmSync(distDir, { recursive: true, force: true });
fs.cpSync(srcDir, distDir, { recursive: true });

console.log(`[PILLVYN] Built frontend assets into ${distDir}`);
