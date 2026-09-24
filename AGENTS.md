# AGENTS.md

Thus Spoke Zakura runs a local Zcash Regtest environment. The `ths` launcher (`crates/tsz-cli`) starts Zakura, lightwalletd, and an app container that serves `tsz-server` (`crates/tsz-server`) and the React dashboard (`web/`). Human-facing setup is in `CONTRIBUTING.md`; releases are in `RELEASING.md`.

## Commands

Run from the repository root. Web commands need Node 24 (for example, `nvm use 24`).

Rust checks:

```console
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test -p thus-spoke-zakura --features release-distribution
```

Web checks:

```console
npm run lint --prefix web
npm run format:check --prefix web
npm test --prefix web
npm run build --prefix web
```

- Run the Rust checks after changing `crates/`, and the web checks after changing `web/`.
- Run `tests/install.sh` after changing `install.sh`.
- Use `cargo run -p thus-spoke-zakura -- <args>` to run the launcher from source. The package is `thus-spoke-zakura`; `ths` is only the binary name.
- Rebuild images with `cargo run -p thus-spoke-zakura -- build --dev` after changing `crates/tsz-server` or `web/`. `ths` runs prebuilt Docker images and will not pick up source changes otherwise.
- Use `TSZ_DEV_API=<dashboard-url> npm run dev --prefix web` for frontend work against a running instance. Get the URL from `ths endpoints`.
- Expect `cargo test --workspace` to run without Docker. The live `activity_recovery` regression is `#[ignore]`d and needs Docker; read "Activity-recovery integration test" in `README.md` before running or changing it.

## Always

- Keep everything Regtest-only. Bind every host-published service to `127.0.0.1`; container-internal listeners may use `0.0.0.0`.
- Preserve instance-scoped cleanup after normal shutdown, interruption, and partial startup failure. Never remove resources belonging to another instance.
- Use the Zakura wallet crates (`zakura-keys`, `zakura-primitives`, `zakura-client-*`). `crates/tsz-server/Cargo.toml` imports them under `zcash_*` aliases.
- Treat accounts 1 to 5 as user accounts. Keep account 6 (`TREASURY_ACCOUNT_ID` in `crates/tsz-server/src/db.rs`), the mining and faucet treasury, out of user-facing account lists and balances.
- Do not remove or weaken idempotency-key validation, deduplication, or activity recovery. Changes to these flows require regression tests.
- Update the command table in `README.md` when adding or changing `ths` commands or flags.
- Follow the conventions and style of the surrounding code. Reuse existing abstractions, naming, error handling, test patterns, and file organization instead of introducing a new style for the same problem.
- Keep changes focused. Do not reformat, rename, reorganize, or refactor unrelated code.

## Sensitive changes

Only change the following when required by the task. Explain the compatibility or release impact:

- Changing exact-pinned versions (`=x.y.z`) in `crates/tsz-server/Cargo.toml` or the `[patch.crates-io]` git revisions in `Cargo.toml`. The wallet crates are release candidates and must move together.
- Changing image names or tags in `crates/tsz-cli/src/runtime.rs`, `Dockerfile`, or `docker/lightwalletd.Dockerfile`.
- Adding dependencies to either crate or to `web/package.json`.
- Editing `.github/workflows/` or the release process.

## Never

- Connect to mainnet or testnet, or bind a service beyond loopback.
- Add the upstream `zcash_keys` or `zcash_primitives` packages. CI rejects them.
- Edit `web/dist/` or `target/`. They are build output.

## Git

- Branch from `main` and use Conventional Commits, for example `fix(cli): ...`, `feat(web): ...`, or `docs: ...`.
- Fill in `.github/pull_request_template.md` when opening a pull request.
