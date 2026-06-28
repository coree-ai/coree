# Changelog

## [0.16.0](https://github.com/coree-ai/coree/compare/v0.15.0...v0.16.0) (2026-06-28)

### Features

* cross-session notification of new high-importance memories ([#64](https://github.com/coree-ai/coree/issues/64)) ([75823ab](https://github.com/coree-ai/coree/commit/75823abf2774e02389bb53cc75958fbcf88bb3bd))
* ephemeral git intelligence — author, co-change graph, ownership ([#63](https://github.com/coree-ai/coree/issues/63)) ([b3abdff](https://github.com/coree-ai/coree/commit/b3abdff4d7def074773b84e4584fd6a6771ecfea))
* index logic version auto-rebuild + reindex CLI ([#70](https://github.com/coree-ai/coree/issues/70)) ([c60a207](https://github.com/coree-ai/coree/commit/c60a20794b6af93cdd74b7480441f1ed4f44877b)), closes [#69](https://github.com/coree-ai/coree/issues/69)
* related-memory neighbours at store time ([#66](https://github.com/coree-ai/coree/issues/66)); git provenance columns ([#67](https://github.com/coree-ai/coree/issues/67)) ([808e5d9](https://github.com/coree-ai/coree/commit/808e5d9cc9afa35851bc21ceb817ffd83fa95f88))
* **release:** replace Renovate pin propagation with a direct workflow ([926aa19](https://github.com/coree-ai/coree/commit/926aa197546cb418be27df1d03ed1d93113bd1b3))
* trim per-prompt inject header + compact result format ([#76](https://github.com/coree-ai/coree/issues/76), [#77](https://github.com/coree-ai/coree/issues/77)) ([94546ee](https://github.com/coree-ai/coree/commit/94546ee0a6dfefcf4a7ffc2931fc13ccba1658e0))

### Bug Fixes

* [#68](https://github.com/coree-ai/coree/issues/68) generic pin/delete messages + [#69](https://github.com/coree-ai/coree/issues/69) per-file git-root resolution ([5890fbd](https://github.com/coree-ai/coree/commit/5890fbd1b131a3c7165e9b921821910d35d9104d))
* **ci:** allow renovate postUpgradeTasks command ([ab4a022](https://github.com/coree-ai/coree/commit/ab4a0220f6d9110afbca18ef7648286dc7209a8c))
* **ci:** pin renovatebot/github-action to v46.1.15 ([681bdad](https://github.com/coree-ai/coree/commit/681bdadc6096ff93303452492d416cb9733f499e))
* **ci:** set RENOVATE_REPOSITORIES so Renovate targets each plugin repo ([dceacd7](https://github.com/coree-ai/coree/commit/dceacd7014890127d2058ad41fbb0a24ec70b645))
* clippy warnings (unnecessary_cast, collapsible_if) from 2026-06-15 batch ([d616e5c](https://github.com/coree-ai/coree/commit/d616e5c8eb0ad96dd546c78bd4188bae2e0b0380))
* make memory on by default, drop "no configuration" session prompt ([7982745](https://github.com/coree-ai/coree/commit/7982745dacff23779219cb055e630791bd7b65bc))
* nested-repo scan with per-repo walks (Option A), bump index logic version ([#71](https://github.com/coree-ai/coree/issues/71)) ([57c245e](https://github.com/coree-ai/coree/commit/57c245e731480410c46ad9a84501740d1f2af00c))
* **release:** update Cargo.lock during release-it bump ([1a38444](https://github.com/coree-ai/coree/commit/1a38444288f011b52993b9f443fb65a5a1ed2f9c))
* serialize embedder model downloads to stop cold-cache lock race ([276b242](https://github.com/coree-ai/coree/commit/276b2426e460f1444a467d785e96af20972fddc6))

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
