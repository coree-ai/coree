# Changelog

## [0.15.0](https://github.com/coree-ai/coree/compare/v0.14.1...v0.15.0) (2026-06-14)

### Features

* add CI pin guard script and workflow template ([#53](https://github.com/coree-ai/coree/issues/53)) ([83ba71b](https://github.com/coree-ai/coree/commit/83ba71b3ffbbdee9ec7b5fdc3d254971f5a53d63))
* add postUpgradeTasks support to Renovate ([#53](https://github.com/coree-ai/coree/issues/53)) ([e276f5b](https://github.com/coree-ai/coree/commit/e276f5bbb866a53a65a2e63c40d5973c76f69393))
* always infer project_id, remove serve_no_config inert-mode gate ([e8e7f60](https://github.com/coree-ai/coree/commit/e8e7f60472337db12b03d15797d17e63adc7c03f)), closes [#49](https://github.com/coree-ai/coree/issues/49)
* **config:** disallow remote/shared storage for the code index ([d4677a3](https://github.com/coree-ai/coree/commit/d4677a37cead2349dfa4a85d9c39e678716a6d19)), closes [#60](https://github.com/coree-ai/coree/issues/60)
* Renovate-based pin propagation replaces bump-dependents ([#53](https://github.com/coree-ai/coree/issues/53)) ([931428a](https://github.com/coree-ai/coree/commit/931428a0f9ea88c71b08d3020d465ca5722b8a00))

### Bug Fixes

* **ci:** CI / packaging hygiene — 2-H1, 2-M4, 2-M5 + LOWs ([b66869d](https://github.com/coree-ai/coree/commit/b66869dbaa26ff69f7d912a2da137ddd57d26b9f)), closes [#48](https://github.com/coree-ai/coree/issues/48)
* guard against empty/literal ${VAR} env values treated as set ([e8221a4](https://github.com/coree-ai/coree/commit/e8221a436db5e5eba941661660386ff2bbb432c8)), closes [#36](https://github.com/coree-ai/coree/issues/36)
* **indexer:** explicit cascading deletes for index DB dependent tables ([4e18b95](https://github.com/coree-ai/coree/commit/4e18b95feafb3bfca6970d05947b494944996e7b)), closes [#34](https://github.com/coree-ai/coree/issues/34)
* **launcher:** resolve symlinks in PWD heuristic to avoid forking project identity ([da476e0](https://github.com/coree-ai/coree/commit/da476e036062e90915f7ceffd735352f0af84723)), closes [#38](https://github.com/coree-ai/coree/issues/38)
* **migrations:** conflict-free bookkeeping for shared remote DBs ([84aef6d](https://github.com/coree-ai/coree/commit/84aef6d1a23194c027bfc1448aca78442b866430)), closes [#59](https://github.com/coree-ai/coree/issues/59)
* prevent UTF-8 panic in print_within_budget on mid-codepoint budget ([5e49188](https://github.com/coree-ai/coree/commit/5e4918885906e856b7f358156f495b3d53e4012e)), closes [#35](https://github.com/coree-ai/coree/issues/35)
* remote sync push + serve_state guards against DB conflicts ([143f434](https://github.com/coree-ai/coree/commit/143f4344eafb612f8a0f2ac81feaf6937f7b6333)), closes [#32](https://github.com/coree-ai/coree/issues/32)
* remove redundant closure in retrieve token mapping ([6185bea](https://github.com/coree-ai/coree/commit/6185bea92c08295f436aed5bcc6f37723c42b5e1))
* **search:** cosine gate no longer kills keyword fallback when zero current-model vectors exist ([2928c1a](https://github.com/coree-ai/coree/commit/2928c1ae822191968ca80b94f5d6162672e37d6f)), closes [#33](https://github.com/coree-ai/coree/issues/33)
* **serve:** secondary processes proxy tool calls to the primary ([9efb094](https://github.com/coree-ai/coree/commit/9efb094e26e1d36cf9b2413831e270f1382261aa)), closes [#58](https://github.com/coree-ai/coree/issues/58)
* soft-delete semantics: active-only reads, topic_key resurrection, evict cascade (closes [#31](https://github.com/coree-ai/coree/issues/31)) ([431da67](https://github.com/coree-ai/coree/commit/431da670e8ae863cf749540856e007f721ac93fc))
* **store:** sanitize facts + tags fields, non-silent redaction notice ([0e664e3](https://github.com/coree-ai/coree/commit/0e664e3b614c764cafe21b884bd2b4b713fcfa75)), closes [#41](https://github.com/coree-ai/coree/issues/41)
* use app-id instead of client-id in create-github-app-token ([99222fb](https://github.com/coree-ai/coree/commit/99222fb4e31bf26617458ce33444f4560a8f53f2))

## [0.14.1](https://github.com/coree-ai/coree/compare/v0.14.0...v0.14.1) (2026-05-15)

### Bug Fixes

* trigger npm-dist after Release workflow, not on push ([d48a7db](https://github.com/coree-ai/coree/commit/d48a7db1aa826562747a0bcff1d2981a8fbfc65d))

## [0.14.0](https://github.com/coree-ai/coree/compare/v0.13.0...v0.14.0) (2026-05-15)

### Features

* replace bump-version.mjs with release-it automation (issue [#30](https://github.com/coree-ai/coree/issues/30)) ([decd695](https://github.com/coree-ai/coree/commit/decd695ce38f3e72d5f729d25c9ede63af28a25c))

### Bug Fixes

* sync optionalDependencies in after:bump hook ([584d904](https://github.com/coree-ai/coree/commit/584d9040451099392b7bff0ece3467fb4a795f9c))
