#!/usr/bin/env node
/**
 * Usage: node scripts/bump-version.mjs <version> [model-version]
 *
 * Bumps the coree version across all files. If model-version is omitted,
 * the model package version is left unchanged.
 *
 * Example: node scripts/bump-version.mjs 0.9.7 1.0.3
 */
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(fileURLToPath(import.meta.url), '../..');

const [newVersion, newModelVersion] = process.argv.slice(2);

if (!newVersion) {
  console.error('Usage: node scripts/bump-version.mjs <version> [model-version]');
  process.exit(1);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function writeJson(file, data) {
  fs.writeFileSync(file, JSON.stringify(data, null, 2) + '\n');
  console.log(`  updated ${path.relative(REPO_ROOT, file)}`);
}

function replaceInFile(file, from, to) {
  const content = fs.readFileSync(file, 'utf8');
  const updated = content.replaceAll(from, to);
  if (content === updated) {
    console.warn(`  warning: no replacement made in ${path.relative(REPO_ROOT, file)}`);
  } else {
    fs.writeFileSync(file, updated);
    console.log(`  updated ${path.relative(REPO_ROOT, file)}`);
  }
}

// Read current versions from the main package as the source of truth.
const mainPkgPath = path.join(REPO_ROOT, 'npm/@coree-ai/coree/package.json');
const mainPkg = readJson(mainPkgPath);
const currentVersion = mainPkg.version;

const modelPkgPath = path.join(REPO_ROOT, 'npm/@coree-ai/coree-model-bge-small-en-v1.5/package.json');
const modelPkg = readJson(modelPkgPath);
const currentModelVersion = modelPkg.version;
const modelVersion = newModelVersion ?? currentModelVersion;

console.log(`coree: ${currentVersion} -> ${newVersion}`);
if (newModelVersion) console.log(`model: ${currentModelVersion} -> ${modelVersion}`);
console.log('');

// Cargo.toml (plain text replacement -- only the package version line).
replaceInFile(
  path.join(REPO_ROOT, 'Cargo.toml'),
  `version = "${currentVersion}"`,
  `version = "${newVersion}"`,
);

// Platform packages.
for (const pkg of ['coree-linux-x64', 'coree-linux-arm64', 'coree-darwin-arm64', 'coree-win32-x64']) {
  const pkgPath = path.join(REPO_ROOT, 'npm/@coree-ai', pkg, 'package.json');
  const manifest = readJson(pkgPath);
  manifest.version = newVersion;
  writeJson(pkgPath, manifest);
}

// Model package.
if (newModelVersion) {
  modelPkg.version = modelVersion;
  writeJson(modelPkgPath, modelPkg);
}

// Main package: version, optionalDependencies, model dependency.
mainPkg.version = newVersion;
for (const key of Object.keys(mainPkg.optionalDependencies)) {
  mainPkg.optionalDependencies[key] = newVersion;
}
mainPkg.dependencies['@coree-ai/coree-model-bge-small-en-v1.5'] = modelVersion;
writeJson(mainPkgPath, mainPkg);

// npm-bundled plugin.json (no suffix -- matches npm version).
const npmPluginPath = path.join(REPO_ROOT, 'npm/@coree-ai/coree/.claude-plugin/plugin.json');
const npmPlugin = readJson(npmPluginPath);
npmPlugin.version = newVersion;
writeJson(npmPluginPath, npmPlugin);

// Gemini extension (suffix -1).
const geminiPath = path.join(REPO_ROOT, 'agents/gemini/gemini-extension.json');
const gemini = readJson(geminiPath);
gemini.version = `${newVersion}-1`;
writeJson(geminiPath, gemini);

console.log(`\nDone. Commit with: git add -u && git commit -m "chore: bump version to ${newVersion}"`);
