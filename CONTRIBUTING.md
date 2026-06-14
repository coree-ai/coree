# Contributing to coree

Thank you for your interest in contributing.

## Contributor License Agreement

Before any pull request can be merged, you must sign the Contributor License Agreement
(CLA). This is a one-time requirement managed automatically by
[CLA Assistant](https://cla-assistant.io).

When you open a pull request, a bot will check whether you have signed the CLA and post
instructions if you have not. Signing takes about 30 seconds via GitHub.

**Why a CLA?** It gives the project a clear, consistent license over all contributions
so the codebase can evolve without legal ambiguity. Your copyright is not affected -
you retain ownership of your contribution.

## How to contribute

1. Open an issue before starting significant work, so we can discuss the approach.
2. Fork the repository and create a branch for your change.
3. Keep changes focused - one concern per pull request.
4. Ensure `cargo test` and `cargo clippy` pass before submitting.
5. Open a pull request against `main`.

## Code style

- Follow standard Rust idioms (`cargo fmt`, `cargo clippy --deny warnings`)
- Explicit types and return types on public functions
- No docstrings or comments on unchanged code
- Error handling via `anyhow` for application code

## Database migrations

One remote database may be shared by several replicas — across projects and even
across machines — that each migrate independently and sync the result. Every new
migration in `src/migrations.rs` MUST be additive and idempotent:

- Create with `CREATE TABLE/INDEX IF NOT EXISTS`.
- Add columns with `ALTER TABLE ... ADD COLUMN` guarded against the
  `"duplicate column name"` error (see `apply_v002`).
- Never emit unconditional seed-data `INSERT`s; use `INSERT OR IGNORE` or an
  idempotent `UPDATE ... WHERE` backfill.
- Avoid destructive renames; prefer add-new + backfill so an older `coree`
  binary sharing the schema keeps working. A shared remote DB implies a shared
  schema version, so schema changes must stay forward-compatible.

Never edit the SQL of a migration that has already shipped — checksums are
validated and existing installs have already applied it. Add a new migration.

## License

By contributing, you agree that your contributions will be licensed under the
[Apache License 2.0](LICENSE).
