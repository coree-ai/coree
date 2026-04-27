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

// Copy model into the model package when --with-model is passed.
// Pass this flag only when the model version is new and fetch-model.py has run.
// Omitting it skips the copy (model version already published); passing it and
// having dist/model absent is a hard error so real fetch failures are not hidden.
const MODEL_PKG = path.join(NPM_ROOT, 'coree-model-bge-small-en-v1.5');
if (process.argv.includes('--with-model')) {
  const modelSrc = path.join(REPO_ROOT, 'dist', 'model');
  const modelDst = path.join(MODEL_PKG, 'model');
  if (!fs.existsSync(modelSrc)) { console.error(`Missing model: ${modelSrc}`); process.exit(1); }
  fs.cpSync(modelSrc, modelDst, { recursive: true });
  console.log('Bundled model into model package.');
} else {
  console.log('Skipping model (--with-model not passed, version already published).');
}
