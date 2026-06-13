#!/usr/bin/env node
const { readFileSync, writeFileSync, existsSync } = require('node:fs');
const { execSync } = require('node:child_process');

const [oldVersion, newVersion] = process.argv.slice(2);
if (!oldVersion || !newVersion) {
  console.error('Usage: renovate-post-upgrade.mjs <oldVersion> <newVersion>');
  process.exit(1);
}

const [oldMajor, oldMinor] = oldVersion.split('.').map(Number);
const [newMajor, newMinor] = newVersion.split('.').map(Number);

if (oldMajor === newMajor && oldMinor === newMinor) {
  console.log(`Patch bump ${oldVersion} -> ${newVersion}, no version sync needed`);
} else {
  const newPluginVersion = `${newMajor}.${newMinor}.0`;
  console.log(`Major/minor bump: syncing plugin version to ${newPluginVersion}`);

  updateJson('package.json', newPluginVersion);

  const manifestPaths = [
    'plugin.json',
    '.claude-plugin/plugin.json',
    '.codex-plugin/plugin.json',
    'gemini-extension.json',
  ];
  for (const p of manifestPaths) {
    updateJson(p, newPluginVersion);
  }

  if (existsSync('extension.toml')) {
    let content = readFileSync('extension.toml', 'utf8');
    const updated = content.replace(/version\s*=\s*".*"/, `version = "${newPluginVersion}"`);
    if (updated !== content) {
      writeFileSync('extension.toml', updated);
      console.log(`  updated extension.toml`);
    }
  }
}

console.log('Running npm install...');
execSync('npm install', { stdio: 'inherit' });

function updateJson(path, version) {
  if (!existsSync(path)) return;
  const pkg = JSON.parse(readFileSync(path, 'utf8'));
  if (pkg.version === version) {
    console.log(`  ${path} already ${version}`);
    return;
  }
  pkg.version = version;
  writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
  console.log(`  updated ${path}: ${version}`);
}
