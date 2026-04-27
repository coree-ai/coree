import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(fileURLToPath(import.meta.url), '../..');
const NPM_ROOT = path.join(REPO_ROOT, 'npm', '@coree-ai');
const MAIN_PKG = path.join(NPM_ROOT, 'coree');
const { version } = JSON.parse(fs.readFileSync(path.join(MAIN_PKG, 'package.json'), 'utf8'));

const PLATFORMS = [
  { pkg: 'coree-linux-x64',    artifact: 'coree-linux-x86_64',   ext: '' },
  { pkg: 'coree-linux-arm64',  artifact: 'coree-linux-aarch64',  ext: '' },
  { pkg: 'coree-darwin-arm64', artifact: 'coree-macos-aarch64',  ext: '' },
  { pkg: 'coree-win32-x64',    artifact: 'coree-windows-x86_64', ext: '.exe' },
];

for (const { pkg, artifact, ext } of PLATFORMS) {
  const pkgDir = path.join(NPM_ROOT, pkg);
  const manifest = JSON.parse(fs.readFileSync(path.join(pkgDir, 'package.json'), 'utf8'));
  manifest.version = version;
  fs.writeFileSync(path.join(pkgDir, 'package.json'), JSON.stringify(manifest, null, 2) + '\n');

  const src = path.join(REPO_ROOT, 'dist', `${artifact}${ext}`);
  const dst = path.join(pkgDir, `coree${ext}`);
  if (!fs.existsSync(src)) { console.error(`Missing artifact: ${src}`); process.exit(1); }
  console.log(`Copy ${src} -> ${dst}`);
  fs.copyFileSync(src, dst);
  if (ext === '') fs.chmodSync(dst, 0o755);
}

// Update optionalDependencies versions in main package.
const mainManifest = JSON.parse(fs.readFileSync(path.join(MAIN_PKG, 'package.json'), 'utf8'));
for (const key of Object.keys(mainManifest.optionalDependencies)) {
  mainManifest.optionalDependencies[key] = version;
}
fs.writeFileSync(path.join(MAIN_PKG, 'package.json'), JSON.stringify(mainManifest, null, 2) + '\n');

// Update plugin.json version to match (lives in agents/claude, not the npm package).
const pluginJsonPath = path.join(REPO_ROOT, 'agents', 'claude', '.claude-plugin', 'plugin.json');
const pluginManifest = JSON.parse(fs.readFileSync(pluginJsonPath, 'utf8'));
pluginManifest.version = version;
fs.writeFileSync(pluginJsonPath, JSON.stringify(pluginManifest, null, 2) + '\n');
