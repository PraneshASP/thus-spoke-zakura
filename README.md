# Thus Spoke Zakura

A one-command, local-first Zcash Regtest environment powered by Zakura.

```console
thus-spoke-zakura
```

The launcher creates an isolated Zakura node, lightwalletd, five development
accounts, a hidden mining treasury, faucet and mining controls, and a browser-based wallet/explorer. Host
ports are chosen automatically and bind only to `127.0.0.1`.

## Requirements

Running a published build requires Docker. `docker version` must show both the
client and server sections without `sudo`.

On Linux installations whose socket is owned by `root:docker`, add your user to
the group:

```console
sudo usermod -aG docker "$USER"
```

Then completely log out of the desktop session and log back in. Opening a new
terminal alone does not refresh supplementary groups. Verify the result with:

```console
id
docker version
```

`id` must include `docker`, and `docker version` must reach the server. As an
immediate alternative for the current shell, `newgrp docker` starts a subshell
with the new group. Membership in `docker` is effectively root-level access to
the machine, so only grant it to trusted users.

## Development

Building from source additionally requires Rust 1.98 and Node 24.

```console
cargo test --workspace
cd web && npm ci && npm run build
```

Build the runtime images and install the launcher:

```console
docker build -t ghcr.io/zakura-core/thus-spoke-zakura-app:0.1.0 .
docker build -f docker/lightwalletd.Dockerfile -t ghcr.io/zakura-core/thus-spoke-zakura-lightwalletd:0.1.0 .
cargo install --path crates/tsz-cli
thus-spoke-zakura doctor
thus-spoke-zakura start
```

The dashboard opens automatically. Use `--no-open` in headless or scripted
environments. The current local runtime uses `zakuracore/zakura:1.2.0` and the
project's `0.1.0` app and lightwalletd images. Docker builds retain Cargo
registry, Git, and target caches through BuildKit cache mounts.

## CLI

Every command accepts `--name <instance>`; the default name is `default`.

```text
start [--no-open] [--build|--build-dev]  Run in foreground; optionally rebuild images
status              Show health and endpoints
open                Open the dashboard
endpoints [--json]  Print integration endpoints
logs [service] -f   Follow app, zakura, or lightwalletd logs
stop                Stop while preserving data
reset --force       Delete exactly one instance and its volumes
list                List instances
doctor              Verify Docker connectivity
```

During development, rebuild the app and lightwalletd images and recreate their
containers without deleting instance data:

```console
cargo run -p thus-spoke-zakura -- start --build
```

For a faster edit/build/run loop, compile the workspace code without release
optimizations or LTO while keeping third-party dependencies optimized for
usable cryptographic proving performance:

```console
cargo run -p thus-spoke-zakura -- start --build-dev
```

`start` keeps control of the terminal after the environment becomes ready.
Press Ctrl+C (or send the platform's termination signal) to stop and remove the
selected instance's service containers. Its chain, wallet, seed, and index data
remain available for the next run.

`reset --force` permanently removes the selected instance's chain, wallet,
seed, and index volumes. No command binds services beyond loopback.

## Architecture

```text
Browser ──HTTP/SSE── tsz-server ──JSON-RPC── Zakura (Regtest)
                         │                       │
                         └──────gRPC──── lightwalletd
```

The Rust service uses Zakura's wallet, key, primitive, proof, and SQLite crates.
It scans compact blocks through lightwalletd, reads balances from the scanned
wallet database, constructs and proves real transactions, and broadcasts them
through a hidden sixth account used as the mining and faucet treasury. On first
startup, Account 1 receives exactly 5 ZEC in Orchard from that treasury. Faucet
requests mine mature coinbase funds, shield them through Account 6, and then make the requested transfer. The activity database
is a UI index only and is never a source of wallet balances.

Each faucet request is limited to 5 ZEC. The regtest wallet begins scanning at
block 2 because lightwalletd represents height 0 as an unspecified block ID.
Before shielding, the service retrieves the mature coinbase transaction from
Zakura and stores its full metadata in the wallet; this compensates for the
compact transparent-output response not carrying a transaction index. These
details are internal to the faucet and require no manual setup.

Instances created by older releases keep Account 1 as the miner in their persisted
Zakura configuration. Run `thus-spoke-zakura reset <name> --force` and start the
instance again to migrate it to the hidden treasury layout. Resetting permanently
deletes that instance's local regtest chain and wallet data.

The local chain activates NU6 at height 1 and intentionally remains before
NU6.1. Activating later upgrades at block 1 would require their consensus
lockbox disbursements in that activation block.

The launcher talks to Docker directly and labels every resource with its
instance name. The lightwalletd container currently runs as UID 0 so it can
initialize its root-owned Docker named volume; its gRPC port remains bound only
to loopback on the host.

This software is for Regtest only. Generated keys must never receive real funds.

## Troubleshooting

- `permission denied` while connecting to `/var/run/docker.sock`: the daemon is
  running, but the current process lacks socket access. Confirm that `id` lists
  `docker`; if it does not, log out and back in after running `usermod`.
- `Cannot connect to the Docker daemon`: start Docker Desktop or the system
  Docker service, then rerun `thus-spoke-zakura doctor`.
- Inspect a service with `thus-spoke-zakura logs app`,
  `thus-spoke-zakura logs zakura`, or
  `thus-spoke-zakura logs lightwalletd`. Add `-f` to follow logs.
- `thus-spoke-zakura reset --force` deletes the selected instance's containers,
  chain, wallet, seed, and volumes. This cannot be recovered.

## License

MIT OR Apache-2.0
