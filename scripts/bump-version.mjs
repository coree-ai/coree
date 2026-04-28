#!/usr/bin/env node
/**
 * Usage: node scripts/bump-version.mjs <version>
 *
 * Bumps the coree version across all files.
 *
 * Example: node scripts/bump-version.mjs 0.10.1
 */
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(fileURLToPath(import.meta.url), '../..');

const [newVersion] = process.argv.slice(2);

if (!newVersion) {
  console.error('Usage: node scripts/bump-version.mjs <version>');
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

const mainPkgPath = path.join(REPO_ROOT, 'npm/@coree-ai/coree/package.json');
const mainPkg = readJson(mainPkgPath);
const currentVersion = mainPkg.version;

console.log(`coree: ${currentVersion} -> ${newVersion}\n`);

// Cargo.toml
replaceInFile(
  path.join(REPO_ROOT, 'Cargo.toml'),
  `version = "${currentVersion}"`,
  `version = "${newVersion}"`,
);

// Platform packages
for (const pkg of ['coree-linux-x64', 'coree-linux-arm64', 'coree-darwin-arm64', 'coree-win32-x64']) {
  const pkgPath = path.join(REPO_ROOT, 'npm/@coree-ai', pkg, 'package.json');
  const manifest = readJson(pkgPath);
  manifest.version = newVersion;
  writeJson(pkgPath, manifest);
}

// Main package: version and optionalDependencies
mainPkg.version = newVersion;
for (const key of Object.keys(mainPkg.optionalDependencies)) {
  mainPkg.optionalDependencies[key] = newVersion;
}
writeJson(mainPkgPath, mainPkg);

// agents/claude plugin.json (suffix -1)
const claudePluginPath = path.join(REPO_ROOT, 'agents/claude/.claude-plugin/plugin.json');
const claudePlugin = readJson(claudePluginPath);
claudePlugin.version = `${newVersion}-1`;
writeJson(claudePluginPath, claudePlugin);

// agents/claude .mcp.json and hooks
replaceInFile(path.join(REPO_ROOT, 'agents/claude/.mcp.json'), currentVersion, newVersion);
replaceInFile(path.join(REPO_ROOT, 'agents/claude/hooks/hooks.json'), currentVersion, newVersion);

// agents/gemini
const geminiPath = path.join(REPO_ROOT, 'agents/gemini/gemini-extension.json');
const gemini = readJson(geminiPath);
gemini.version = `${newVersion}-1`;
writeJson(geminiPath, gemini);
replaceInFile(path.join(REPO_ROOT, 'agents/gemini/hooks/hooks.json'), currentVersion, newVersion);

console.log(`\nDone. Commit with: git add -u && git commit -m "chore: bump version to ${newVersion}"`);
