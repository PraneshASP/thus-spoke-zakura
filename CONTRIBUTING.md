# Contributing

Thanks for helping improve Thus Spoke Zakura!

## Getting help

For questions about setup or contributing, join the [Zypherpunks Discord](https://discord.gg/2ZV4r2J9n) and ask in the [#dev-help](https://discord.com/channels/1552177825874714667/1552189956963700807) channel.

## Before you start

- Check the [open issues](https://github.com/zcashlabs/thus-spoke-zakura/issues) and pull requests to avoid duplicate work.
- Comment on an issue before you start working on it.
- For larger changes, open an issue first, so the approach can be agreed on.

## Development setup

You need Rust 1.98, Node 24, and Docker.

Install the web dependencies and build development runtime images:

```console
cd web
npm ci
cd ..
cargo run -p thus-spoke-zakura -- build --dev
```

Run the launcher from the checkout:

```console
cargo run -p thus-spoke-zakura
```

After changing the server or web app, rerun `build --dev` before starting again.

## Checks

CI runs the following. Run them locally before opening a pull request:

```console
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test -p thus-spoke-zakura --features release-distribution
npm run lint --prefix web
npm run format:check --prefix web
npm test --prefix web
npm run build --prefix web
```

## Pull requests

- Branch from `main` and keep each pull request focused on one change.
- Use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages, for example `fix(cli): ...` or `feat(web): ...`.
- Add or update tests for behavior changes.
- Fill in the pull request template, and include screenshots for dashboard changes.

## Reporting bugs and requesting features

Use the issue templates. For bugs, include `ths --version`, your platform, and relevant output from `ths status` and `ths logs`.

Report security vulnerabilities privately to the maintainers, not in a public issue.
