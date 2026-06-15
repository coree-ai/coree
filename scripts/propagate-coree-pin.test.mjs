import assert from "node:assert/strict";
import { execSync, spawnSync } from "node:child_process";
import {
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, sep } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(import.meta.url).replace(/\.test\.mjs$/, ".mjs");

let tmp;

before(() => {
	tmp = mkdtempSync(join(tmpdir(), "coree-prop-"));
	if (sep === "\\") {
		try {
			execSync("git config core.autocrlf false", { cwd: tmp, stdio: "pipe" });
		} catch {}
	}
});

after(() => {
	try {
		rmSync(tmp, { recursive: true, force: true });
	} catch {}
});

function freshRepo(name) {
	const dir = mkdtempSync(join(tmp, `repo-${name || "anon"}-`));
	execSync("git init -b main", { cwd: dir, encoding: "utf8", stdio: "pipe" });
	return dir;
}

function git(cwd, cmd) {
	return execSync(`git ${cmd}`, {
		cwd,
		encoding: "utf8",
		stdio: "pipe",
	}).trim();
}

function run(dir, args, env = {}) {
	return spawnSync(process.execPath, [script, ...args], {
		cwd: dir,
		env: { ...process.env, ...env },
		encoding: "utf8",
		stdio: ["pipe", "pipe", "pipe"],
		timeout: 10_000,
	});
}

function ok(dir, args, env) {
	const r = run(dir, args, env);
	if (r.status !== 0)
		throw new Error(
			`expected exit 0, got ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`,
		);
	return r;
}

function fail(dir, args, env) {
	const r = run(dir, args, env);
	if (r.status === 0)
		throw new Error(
			`expected non-zero exit, got 0\nstdout: ${r.stdout}\nstderr: ${r.stderr}`,
		);
	return r;
}

describe("propagate-coree-pin", () => {
	describe("argument validation", () => {
		it("errors without a version argument", () => {
			const r = fail(freshRepo("v0"), []);
			assert.match(r.stderr, /Usage:/);
		});

		it("errors with invalid version format", () => {
			const r = fail(freshRepo("v1"), ["abc"]);
			assert.match(r.stderr, /Usage:/);
		});

		it("errors when no tracked files contain a coree pin", () => {
			const dir = freshRepo("v2");
			writeFileSync(
				join(dir, "package.json"),
				'{"name":"test","version":"0.1.0"}',
			);
			writeFileSync(join(dir, "readme.md"), "# hello");
			git(dir, "add package.json readme.md");
			const r = fail(dir, ["0.15.0"]);
			assert.match(r.stderr, /no @coree-ai\/coree pin found/);
		});
	});

	describe("pin rewriting", () => {
		it("rewrites @coree-ai/coree@ semver pin in npx/hook commands", () => {
			const dir = freshRepo("pr1");
			writeFileSync(
				join(dir, "package.json"),
				'{"name":"test","version":"0.1.0"}',
			);
			writeFileSync(
				join(dir, "hook-wrapper.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject\n",
			);
			git(dir, "add package.json hook-wrapper.js");
			ok(dir, ["0.15.0"]);
			assert.match(
				readFileSync(join(dir, "hook-wrapper.js"), "utf8"),
				/@coree-ai\/coree@0\.15\.0/,
			);
		});

		it('rewrites COREE_VERSION = "..." shell/make pin', () => {
			const dir = freshRepo("pr2");
			writeFileSync(
				join(dir, "package.json"),
				'{"name":"test","version":"0.1.0"}',
			);
			writeFileSync(join(dir, "Makefile"), 'COREE_VERSION = "0.14.0"\n');
			git(dir, "add package.json Makefile");
			ok(dir, ["0.15.0"]);
			assert.match(
				readFileSync(join(dir, "Makefile"), "utf8"),
				/COREE_VERSION\s*=\s*"0\.15\.0"/,
			);
		});

		it("errors on inconsistent pins across different files", () => {
			const dir = freshRepo("pr3");
			writeFileSync(
				join(dir, "package.json"),
				'{"name":"test","version":"0.1.0"}',
			);
			writeFileSync(
				join(dir, "hook-a.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			writeFileSync(
				join(dir, "hook-b.js"),
				"npx --yes @coree-ai/coree@0.13.0 inject",
			);
			git(dir, "add package.json hook-a.js hook-b.js");
			const r = fail(dir, ["0.15.0"]);
			assert.match(r.stderr, /inconsistent/);
		});

		it("skips excluded files (CLAUDE.md, AGENTS.md, etc.)", () => {
			const dir = freshRepo("pr4");
			writeFileSync(
				join(dir, "package.json"),
				'{"name":"test","version":"0.1.0"}',
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			writeFileSync(
				join(dir, "CLAUDE.md"),
				"npx --yes @coree-ai/coree@0.13.0\n",
			);
			git(dir, "add package.json hook.js CLAUDE.md");
			ok(dir, ["0.15.0"]);
			assert.match(
				readFileSync(join(dir, "hook.js"), "utf8"),
				/coree@0\.15\.0/,
			);
			assert.match(
				readFileSync(join(dir, "CLAUDE.md"), "utf8"),
				/coree@0\.13\.0/,
			);
		});

		it("reports no change when pin already matches", () => {
			const dir = freshRepo("pr5");
			writeFileSync(
				join(dir, "package.json"),
				'{"name":"test","version":"0.1.0"}',
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.15.0 inject",
			);
			git(dir, "add package.json hook.js");
			const r = ok(dir, ["0.15.0"]);
			assert.match(r.stdout, /patch bump|plugin version unchanged/);
		});
	});

	describe("plugin version sync", () => {
		it("syncs plugin version to X.Y.0 on major bump", () => {
			const dir = freshRepo("vs1");
			writeFileSync(
				join(dir, "package.json"),
				JSON.stringify({ name: "test", version: "0.14.2" }),
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			git(dir, "add package.json hook.js");
			ok(dir, ["1.0.0"]);
			const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
			assert.strictEqual(pkg.version, "1.0.0");
		});

		it("syncs plugin version to X.Y.0 on minor bump", () => {
			const dir = freshRepo("vs2");
			writeFileSync(
				join(dir, "package.json"),
				JSON.stringify({ name: "test", version: "0.14.0" }),
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			git(dir, "add package.json hook.js");
			ok(dir, ["0.15.0"]);
			const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
			assert.strictEqual(pkg.version, "0.15.0");
		});

		it("keeps plugin version on patch bump", () => {
			const dir = freshRepo("vs3");
			writeFileSync(
				join(dir, "package.json"),
				JSON.stringify({ name: "test", version: "0.14.2" }),
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			git(dir, "add package.json hook.js");
			const r = ok(dir, ["0.14.3"]);
			assert.match(r.stdout, /plugin version unchanged/);
			const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
			assert.strictEqual(pkg.version, "0.14.2");
		});

		it("syncs .claude-plugin/plugin.json version on major bump", () => {
			const dir = freshRepo("vs4");
			mkdirSync(join(dir, ".claude-plugin"));
			writeFileSync(
				join(dir, "package.json"),
				JSON.stringify({ name: "test", version: "0.14.2" }),
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			writeFileSync(
				join(dir, ".claude-plugin/plugin.json"),
				JSON.stringify({ version: "0.14.2" }),
			);
			git(dir, "add package.json hook.js .claude-plugin/plugin.json");
			ok(dir, ["1.0.0"]);
			const plug = JSON.parse(
				readFileSync(join(dir, ".claude-plugin/plugin.json"), "utf8"),
			);
			assert.strictEqual(plug.version, "1.0.0");
		});
	});

	describe("GITHUB_OUTPUT", () => {
		it("writes changed and plugin_version to GITHUB_OUTPUT on major bump", () => {
			const dir = freshRepo("gh1");
			writeFileSync(
				join(dir, "package.json"),
				JSON.stringify({ name: "test", version: "0.14.2" }),
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			git(dir, "add package.json hook.js");
			const outFile = join(dir, "_out");
			ok(dir, ["1.0.0"], { GITHUB_OUTPUT: outFile });
			const out = readFileSync(outFile, "utf8");
			assert.match(out, /changed=true/);
			assert.match(out, /plugin_version=1\.0\.0/);
		});

		it("writes changed=true on pin-only change (patch, no version sync)", () => {
			const dir = freshRepo("gh2");
			writeFileSync(
				join(dir, "package.json"),
				JSON.stringify({ name: "test", version: "0.14.2" }),
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			git(dir, "add package.json hook.js");
			const outFile = join(dir, "_out");
			ok(dir, ["0.14.1"], { GITHUB_OUTPUT: outFile });
			const out = readFileSync(outFile, "utf8");
			assert.match(out, /changed=true/);
		});

		it("writes changed=false when nothing changes", () => {
			const dir = freshRepo("gh3");
			writeFileSync(
				join(dir, "package.json"),
				JSON.stringify({ name: "test", version: "0.14.2" }),
			);
			writeFileSync(
				join(dir, "hook.js"),
				"npx --yes @coree-ai/coree@0.14.0 inject",
			);
			git(dir, "add package.json hook.js");
			const outFile = join(dir, "_out");
			ok(dir, ["0.14.0"], { GITHUB_OUTPUT: outFile });
			const out = readFileSync(outFile, "utf8");
			assert.match(out, /changed=false/);
		});
	});
});
