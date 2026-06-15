#!/usr/bin/env node

// Propagate a new @coree-ai/coree version into a plugin wrapper repo.
//
// Run with the plugin repo as CWD:
//   node propagate-coree-pin.mjs <newCoreeVersion>
//
// Responsibilities (consolidates what Renovate's customManager +
// renovate-post-upgrade.mjs used to do):
//   1. Rewrite the coree dependency pin everywhere it appears.
//   2. On a coree major/minor change, sync the plugin's OWN version to
//      <coreeMajor>.<coreeMinor>.0 across package.json + manifests.
//   3. (Lockfile refresh + commit/tag are done by the workflow, not here.)
//
// Emits GitHub Actions outputs: `changed` and `plugin_version`.

import { execSync } from "node:child_process";
import {
	appendFileSync,
	existsSync,
	readFileSync,
	writeFileSync,
} from "node:fs";

const newCoree = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(newCoree || "")) {
	console.error(
		`Usage: propagate-coree-pin.mjs <newCoreeVersion>  (got: ${newCoree})`,
	);
	process.exit(1);
}

// Pin rewrite rules kept in sync with coree-ai/.github coree-version-check.yml.
// Each rule's replacer takes exactly its own capture groups so group counts
// never collide with the trailing offset/string args of String.replace.
let oldCoree = null;
const record = (cur) => {
	if (oldCoree === null) oldCoree = cur;
	else if (oldCoree !== cur) {
		console.error(
			`ERROR: inconsistent existing coree pins: ${oldCoree} vs ${cur}`,
		);
		process.exit(1);
	}
};
const PIN_RULES = [
	{
		re: /(@coree-ai\/coree@)(\d+\.\d+\.\d+)/g,
		repl: (_m, p1, cur) => {
			record(cur);
			return `${p1}${newCoree}`;
		},
	},
	{
		re: /(COREE_VERSION\s*=\s*")(\d+\.\d+\.\d+)(")/g,
		repl: (_m, p1, cur, p3) => {
			record(cur);
			return `${p1}${newCoree}${p3}`;
		},
	},
];

// Steering docs are canonical, pin-free content owned by coree-ai/.github.
const SKIP = new Set([
	"CLAUDE.md",
	"GEMINI.md",
	"AGENTS.md",
	"opencode.md",
	"package-lock.json",
]);

const trackedFiles = execSync("git ls-files", { encoding: "utf8" })
	.split("\n")
	.filter(Boolean)
	.filter((f) => !SKIP.has(f) && !f.includes("node_modules/"));

let pinChanged = false;

for (const file of trackedFiles) {
	let text;
	try {
		text = readFileSync(file, "utf8");
	} catch {
		continue;
	}
	let updated = text;
	for (const { re, repl } of PIN_RULES) {
		re.lastIndex = 0;
		updated = updated.replace(re, repl);
	}
	if (updated !== text) {
		writeFileSync(file, updated);
		pinChanged = true;
		console.log(`  pin -> ${newCoree}: ${file}`);
	}
}

if (oldCoree === null) {
	console.error("ERROR: no @coree-ai/coree pin found to update.");
	process.exit(1);
}

// Plugin own-version sync: only on coree major/minor change.
const [oMaj, oMin] = oldCoree.split(".").map(Number);
const [nMaj, nMin] = newCoree.split(".").map(Number);
let pluginVersion = readJson("package.json").version;
let versionChanged = false;

if (oMaj !== nMaj || oMin !== nMin) {
	const target = `${nMaj}.${nMin}.0`;
	for (const p of [
		"package.json",
		"plugin.json",
		".claude-plugin/plugin.json",
		".codex-plugin/plugin.json",
		"gemini-extension.json",
	]) {
		if (writeJsonVersion(p, target)) versionChanged = true;
	}
	if (existsSync("extension.toml")) {
		const content = readFileSync("extension.toml", "utf8");
		const updated = content.replace(
			/version\s*=\s*".*"/,
			`version = "${target}"`,
		);
		if (updated !== content) {
			writeFileSync("extension.toml", updated);
			versionChanged = true;
			console.log(`  version -> ${target}: extension.toml`);
		}
	}
	pluginVersion = target;
} else {
	console.log(
		`coree patch bump ${oldCoree} -> ${newCoree}: plugin version unchanged (${pluginVersion})`,
	);
}

const changed = pinChanged || versionChanged;
const out = process.env.GITHUB_OUTPUT;
if (out) {
	appendFileSync(out, `changed=${changed}\n`);
	appendFileSync(out, `plugin_version=${pluginVersion}\n`);
	appendFileSync(out, `old_coree=${oldCoree}\n`);
}
console.log(
	`\nchanged=${changed} plugin_version=${pluginVersion} (coree ${oldCoree} -> ${newCoree})`,
);

function readJson(path) {
	return JSON.parse(readFileSync(path, "utf8"));
}
function writeJsonVersion(path, version) {
	if (!existsSync(path)) return false;
	const pkg = readJson(path);
	if (pkg.version === version) return false;
	pkg.version = version;
	writeFileSync(path, JSON.stringify(pkg, null, 2) + "\n");
	console.log(`  version -> ${version}: ${path}`);
	return true;
}
