# Complete stack

Manifests in the repo. Versions are "manifest range → resolved in lockfile".

| Sub-project | Manifest | Lockfile |
|---|---|---|
| Database server | `Cargo.toml` | `Cargo.lock` (tracked, even though `.gitignore` lists it) |
| Whiteboard server | `demo/whiteboard-react/package.json` | `package-lock.json` (npm) |
| Whiteboard client | `demo/whiteboard-react/client/package.json` | `package-lock.json` (npm) |
| Order book / telemetry bridge | `demo/trackmania/package.json` | `package-lock.json` (npm) |
| Python whiteboard (first version) | no manifest; `demo/whiteboard/README.md` says `pip install websockets redis` | none |
| Paper tooling | `paper/package.json`, `paper/report/package.json` | not part of the product; listed under Dev tooling |

## language_runtime: Languages and runtimes

| Name | Version | Used for |
|---|---|---|
| Rust | edition 2021; Docker builder `rust:1.87-slim`; CI on stable and nightly | Database server |
| JavaScript (Node, ES modules) | Node 20 (`node:20-slim` in the whiteboard Dockerfile); `engines.node >= 18` (trackmania) | Demo servers |
| JSX | via Vite React plugin | Whiteboard client |
| Python 3 | not pinned | First whiteboard demo (`demo/whiteboard/server.py`) |
| PowerShell | | Demo and benchmark scripts (`scripts/*.ps1`) |
| LaTeX | Tectonic (`paper/Tectonic.toml`) | Paper and report (not product code) |

## framework: Frameworks

| Name | Version | Used for |
|---|---|---|
| Tokio | 1 → 1.49.0 (`full`) | Async runtime: TCP listener, a task per connection, expiry loop, AOF writer |
| Axum | 0.7 → 0.7.9 | HTTP server for `/metrics`, `/health`, `/info` (`src/metrics.rs`) |
| clap | 4.5 → 4.5.54 (`derive`) | CLI flags (`src/main.rs`) |
| Express | ^4.18.2 / ^4.19.2 → 4.22.2 | Demo HTTP servers |
| Socket.IO | ^4.7.2 → 4.8.3 | Whiteboard real-time events (server) |
| socket.io-client | ^4.7.2 → 4.8.3 | Whiteboard real-time events (browser) |
| React / React DOM | ^18.2.0 → 18.3.1 | Whiteboard client UI |

## ui: Components, styling, animation, icons

| Name | Version | Used for |
|---|---|---|
| HTML canvas + inline CSS | `demo/whiteboard-react/client/src/components/Whiteboard.jsx` | Drawing surface, cursors |
| Static HTML pages | `demo/trackmania/public/{market,trade,index}.html` | Order-book views, telemetry dashboard |

No component library, CSS framework, or icon set.

## state_data_fetching: State management and data fetching

| Name | Version | Used for |
|---|---|---|
| React hooks | React 18.3.1 | Whiteboard client state |
| Socket.IO events | 4.8.3 | Board state sync |
| `fetch` (browser) | | Order-book pages call `/book`, `/order` |

## backend_library: Backend libraries (database server)

| Name | Version | Used for |
|---|---|---|
| dashmap | 6.0 → 6.1.0 | Sharded concurrent map for the keyspace and metrics counters |
| serde | 1.0 → 1.0.228 | Config and schema (de)serialisation |
| serde_json | 1.0 → 1.0.149 | JSON value type, schema definitions |
| jsonpath-rust | 0.7 → 0.7.5 | JSONPath for `JSON.*` commands (`src/commands/json.rs`) |
| sqlparser | 0.39 → 0.39.0 | Parses `QUERY` / `EXPLAIN` SQL (`src/query/executor.rs`) |
| bloomfilter | 1.0 → 1.0.16 | Bloom filter type (`src/storage/bloom.rs`) |
| ordered-float | 4.2 → 4.6.0 | Float scores for sorted sets (`src/storage/zset.rs`) |
| toml | 0.8 → 0.8.23 | Reads `rivetdb.toml` (`src/config.rs`) |
| tracing | 0.1 → 0.1.44 | Structured logging |
| tracing-subscriber | 0.3 → 0.3.22 (`env-filter`) | Log formatting and level filter |
| tower | 0.5 → 0.5.3 | **declared, unused** (no `tower` import in `src/`) |
| bytes | 1.5 → 1.11.0 | **declared, unused** (no `bytes` import in `src/`) |

Backend libraries (demos):

| Name | Version | Used for |
|---|---|---|
| redis (npm) | ^4.6.10 / ^4.6.13 → 4.7.1 | Client to the database (whiteboard, order book), with reconnect strategy |
| pg | ^8.11.3 → 8.22.0 | Archive finished boards to Postgres (`saveBoard`) |
| ws | ^8.18.0 → 8.21.0 | **declared, unused** in `demo/trackmania` (no import in `server.js` or `forwarder.mjs`) |
| Python `websockets`, `redis` | unpinned | First whiteboard server |

No job queue, scheduler, email, payments, or file uploads.

## database_storage: Databases, caches, search, vector stores, object storage

| Name | Version | Used for |
|---|---|---|
| The database itself | 0.1.0 | Primary store for all demos |
| Append-only file on local disk | `appendonly.aof` (`[persistence]` in `rivetdb.toml`) | Write log; replay at startup not wired yet |
| PostgreSQL (Supabase-hosted) | `pg` client | Whiteboard archive tables `boards` and `strokes` (`demo/whiteboard-react/server.js`) |

## ai_ml: AI and ML

None in the product. The commit `e3ee042` has a `Co-Authored-By` trailer for an AI coding assistant. That's dev process, not stack.

## third_party_api: Third-party APIs and SaaS integrations

| Name | Evidence | Used for |
|---|---|---|
| Supabase (Postgres) | `DATABASE_URL`; log lines `→ Supabase` in `server.js` | Whiteboard archive |
| Codecov | `codecov/codecov-action@v3` in CI | Coverage upload |
| Openplanet "Data Sender" plugin (Trackmania) | `demo/trackmania/forwarder.mjs` header comment; `DS_HOST`, `DS_PORT` | Local telemetry source for the bridge |

## messaging_bot_platform: Messaging and bot platforms

None.

## auth: Auth providers

| Name | Version | Used for |
|---|---|---|
| Built-in password auth | `RIVETDB_PASSWORD`, `[security]` in `src/config.rs` | Per-connection `AUTH` |

No external identity provider. The demo apps have no user login; the whiteboard uses a display name and a board code.

## cloud_infra: Infra and cloud

| Name | Evidence | Used for |
|---|---|---|
| Docker (server) | `Dockerfile`: multi-stage `rust:1.87-slim` → `debian:bookworm-slim`, cached dependency layer, exposes 7878 | Database image |
| Docker (whiteboard) | `demo/whiteboard-react/Dockerfile`: builds the Vite client, serves it from Node on 3000 | Whiteboard image |
| Render | `render.yaml`: `tm-bridge`, Node runtime, `plan: free`, `rootDir: demo/trackmania`; forwarder default `BRIDGE_URL` is an `onrender.com` address | Hosts the order book / telemetry bridge |
| Hosted database endpoint | the demo servers' default `RIVETDB_HOST` is a remote hostname (see `redaction-check.md`) | Where the demos' database runs. TODO(owner): which platform hosts the database container, and is it still up? |
| Supabase | as above | Managed Postgres |

No IaC, Kubernetes, CDN, or DNS config.

## iac_deployment: IaC and deployment tooling

`render.yaml` (Render Blueprint) is the only declarative deploy file.

## ci_cd: CI/CD

| Name | Version | Used for |
|---|---|---|
| GitHub Actions | `.github/workflows/ci.yml` | Push/PR: tests on 3 OS × 2 toolchains, fmt, clippy, tarpaulin to Codecov, release build. Details below. |

Workflow details (`ci.yml`):
- Triggers on push to `main`/`develop` and on PRs.
- Unit tests: `cargo test --lib`, on Ubuntu, Windows, macOS × stable, nightly.
- Integration tests against a started server, with failures ignored (`|| true`).
- `rustfmt --check` and `clippy -D warnings`.
- Tarpaulin coverage uploaded to Codecov.
- A release build uploaded as an artifact.
- Actions used: `actions-rs/toolchain@v1`, `actions/cache@v3`, `actions/upload-artifact@v3`.

## observability: Observability

| Name | Version | Used for |
|---|---|---|
| Prometheus exposition endpoint | `src/metrics.rs`, port `metrics_port` (9090) | Per-command counts and time, key count, memory, expired keys |
| `/health`, `/info` (JSON) | `src/metrics.rs` | Liveness and server info |
| In-protocol commands | `STATS`, `HOTKEYS`, `SLOWLOG`, `MEMORY` (`src/commands/admin.rs`) | Live introspection from any client |
| tracing logs | as above | Every command logged at INFO since commit `d87cc35` "for demo visibility" |

No Prometheus or Grafana config in the repo. No error tracking.

## testing: Testing

| Name | Version | Used for |
|---|---|---|
| Rust built-in test harness | | 32 lib tests, 46 command tests in `tests/unit/`, 7 ignored integration tests (need a running server) |
| Custom RESP test client | `tests/integration/test_client.rs` | End-to-end tests over TCP |
| cargo-tarpaulin | CI, and run locally for this kit | Coverage: **12.64% (527 / 4,168 lines)**, run on 2026-10-08 with `cargo tarpaulin --engine llvm` |
| redis-benchmark (in Docker) | `scripts/benchmark.ps1` | Throughput comparison against Redis |

## dev_tooling: Dev tooling

| Name | Version | Used for |
|---|---|---|
| Cargo | | Build, test |
| rustfmt, clippy | CI components | Format and lint checks |
| Vite | ^5.0.0 → 5.4.21 | Whiteboard client dev and build (`client/vite.config.js`) |
| @vitejs/plugin-react | ^4.2.0 → 4.7.0 | JSX transform |
| npm | lockfileVersion 3 | Demo packages |
| Tectonic / LaTeX | `paper/Tectonic.toml` | Builds the paper PDF |

No monorepo tooling or git hooks.

## Card items (featuredOnCard)

These four become `featuredOnCard: true` in `cms-entry.json`:
- Rust
- Tokio
- DashMap
- Redis protocol (RESP)

Declared but unused, so left out of `cms-entry.json`: tower, bytes, ws.

## Stack at a glance

- A single Rust binary on Tokio that speaks the Redis protocol, so any Redis client works with it.
- The keyspace is a sharded DashMap, so commands on different keys run in parallel instead of queuing on one thread.
- JSON with JSONPath, time-series, Bloom filters, typed schemas, and a SQL `QUERY` command are built in. No separate modules.
- Writes go to an append-only file through a background channel, so disk I/O stays out of the request path.
- Prometheus metrics, a slow log, and hot-key tracking are built into the server.
- Demo apps on Node: a Socket.IO whiteboard archived to Postgres, and an order book matched on sorted sets.
