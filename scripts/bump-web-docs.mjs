#!/usr/bin/env node
/**
 * Usage: node scripts/bump-web-docs.mjs <version>
 *
 * Updates the coree version string in web docs and config.
 * Called by release-it as an after:bump hook.
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(fileURLToPath(import.meta.url), "../..");

const [newVersion] = process.argv.slice(2);
if (!newVersion) {
	console.error("Usage: node scripts/bump-web-docs.mjs <version>");
	process.exit(1);
}

function replaceInFile(file, from, to) {
	const content = fs.readFileSync(file, "utf8");
	const updated = content.replaceAll(from, to);
	if (content === updated) {
		console.warn(
			`  warning: no replacement made in ${path.relative(REPO_ROOT, file)}`,
		);
	} else {
		fs.writeFileSync(file, updated);
		console.log(`  updated ${path.relative(REPO_ROOT, file)}`);
	}
}

const configPath = path.join(REPO_ROOT, "web", "config.toml");
const currentVersion = fs
	.readFileSync(configPath, "utf8")
	.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (!currentVersion) {
	console.error("Could not detect current version from web/config.toml");
	process.exit(1);
}

console.log(`web docs: ${currentVersion} -> ${newVersion}\n`);

// Sync optionalDependencies in the main npm package (bumper only updates the version field).
const mainPkgPath = path.join(REPO_ROOT, "npm/@coree-ai/coree/package.json");
const mainPkg = JSON.parse(fs.readFileSync(mainPkgPath, "utf8"));
for (const key of Object.keys(mainPkg.optionalDependencies)) {
	mainPkg.optionalDependencies[key] = newVersion;
}
fs.writeFileSync(mainPkgPath, JSON.stringify(mainPkg, null, 2) + "\n");
console.log(`  updated npm/@coree-ai/coree/package.json optionalDependencies`);

replaceInFile(configPath, currentVersion, newVersion);

const installDir = path.join(
	REPO_ROOT,
	"web",
	"content",
	"docs",
	"installation",
);
for (const file of fs.readdirSync(installDir)) {
	replaceInFile(path.join(installDir, file), currentVersion, newVersion);
}
