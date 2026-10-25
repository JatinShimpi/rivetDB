# Technical depth map

## Strongest technical talking points

- **A database server written from scratch.** A Rust and Tokio server that speaks the Redis wire protocol: a hand-written RESP parser that handles partial and pipelined frames, a Tokio task per connection, and about 100 commands. Stock `redis-cli`, `redis-benchmark`, and the Node and Python Redis clients work unchanged (`src/protocol/frame.rs`, `src/main.rs`).
- **Concurrency built on a sharded map.** The keyspace and metrics live in `DashMap`, so commands on different keys run in parallel. The project's own benchmark reports +42.8% over Redis on non-pipelined `INCR` and roughly +14% on `GET`/`SET`. It also reports the cost: pipelined `SET` is far behind (`paper/chapters/chapter6.tex`, single machine).
- **Write logging off the request path.** The append-only file is fed through a bounded `mpsc` channel to a background writer with a 64 KB buffer and configurable fsync. The durability trade-off is written down in the code (`src/persistence/async_aof.rs`).
- **Five data-model extensions in one binary.** JSON with JSONPath, time-series with retention and labels, Bloom filters, typed schemas with validation, and a SQL `QUERY` over the keyspace, all variants of one value enum (`src/storage/value.rs`).
- **Observability built into the server.** A Prometheus endpoint with 13 metric families, a `/health` endpoint, a slow log, and hot-key tracking, with no external agent (`src/metrics.rs`, `src/commands/admin.rs`).

## engineering_delivery: Engineering and delivery

| Concept | Status | Evidence | How it works here |
|---|---|---|---|
| Technical ownership | Partial | `TESTING.md`, `docs/CONFIGURATION.md`, `docs/phase1.*-complete.md`, `paper/` | Design reasoning is documented in depth in the paper. There's no README at the repo root. |
| Key-person risk (handover-ready) | Partial | Module layout under `src/`, doc comments on most modules | Readable modules with clear names and comments. Gaps: no root README, the root folder has build logs, binaries, zips, and a 500 KB code dump, and server code came in few large commits. |
| Codebase assessment | Partial | `src/{protocol,storage,commands,persistence,schema,query,metrics}` | Clean layering: protocol, router, commands, storage. Debt: AOF replay not wired after the DashMap refactor (`src/main.rs`); namespace commands not routed; LRU reuses the LFU logic (`eviction.rs`); about 13 unused-import warnings; old `mainv1.rs` at the root. |
| Architecture review | Partial | `paper/chapters/chapter3.tex` to `chapter6.tex` | A written architecture with benchmarks and stated limitations. |
| Deployment pipeline | Partial | `Dockerfile`, `demo/whiteboard-react/Dockerfile`, `render.yaml` | Container images and a Render blueprint for one demo. No automated deploy of the database server. |
| CI/CD | Present | `.github/workflows/ci.yml` | Test matrix (3 OS × 2 toolchains), fmt, clippy, coverage, release artifact. Caveats: `cargo test --lib` skips the 46 tests in `tests/unit/`; integration failures are ignored (`\|\| true`); `clippy -D warnings` would fail on the current unused-import warnings; `actions-rs` and `upload-artifact@v3` are deprecated. TODO(owner): is CI currently green? |
| Automated testing | Present | `tests/unit/commands/*`, `#[cfg(test)]` modules, `tests/integration_tests.rs` | 78 tests pass locally (32 lib + 46 unit). 7 integration tests are ignored by default and need a running server. |
| Test coverage | Partial | `cargo tarpaulin --engine llvm` run for this kit | **12.64% (527 / 4,168 lines).** Highest: `schema/validator.rs`, `storage/bloom.rs`, `commands/auth.rs`. Zero coverage: `json.rs`, `hash.rs`, `zset.rs`, `timeseries.rs` commands, the RESP parser, `main.rs`, `metrics.rs`. |
| DevOps practices | Partial | Multi-stage Dockerfiles with dependency caching, TOML config plus CLI overrides, `RIVETDB_PASSWORD` from env | Config and secrets by env for the server. The demos hardcode connection fallbacks (see Security). |

## reliability_operations: Reliability and operations

| Concept | Status | Evidence | How it works here |
|---|---|---|---|
| Monitoring | Partial | `/metrics` Prometheus endpoint | Metrics are exposed. No scrape config or dashboards in the repo. |
| Logging | Present | `tracing` with an env filter and configurable level | Structured logs. Every command and its arguments is logged at INFO (commit `d87cc35`). |
| Observability stack | Partial | `src/metrics.rs`, `SLOWLOG`, `HOTKEYS`, `STATS` | Built-in metrics and introspection. No tracing backend or log shipping. |
| Reliability | Partial | Async AOF, demo reconnect strategy | AOF is written but not replayed at startup, so a restart loses data. The AOF drops entries when its channel is full. |
| Uptime measures | Partial | `/health` on the metrics port; demo reconnect with backoff; Postgres optional | Health endpoint present. No graceful shutdown that flushes the AOF on SIGTERM (a `shutdown()` exists but isn't called). |
| Performance bottlenecks | Partial | `src/storage/eviction.rs`, `src/metrics.rs`, paper chapter 6 | `current_memory_usage` walks every key on each eviction check and each metrics scrape (O(n)). Eviction picks a victim by scanning all access counts. Pipelined writes are slow (paper). |
| Database bottlenecks | Partial | `src/storage/state.rs` | The expiry min-heap sits behind one `Mutex`. `key_access_count` grows per key and is exported per key to Prometheus (high cardinality). `KEYS` and `QUERY` scan the whole map. |

## infrastructure: Infrastructure

| Concept | Status | Evidence | How it works here |
|---|---|---|---|
| Cloud infrastructure | Partial | `render.yaml` (free plan), Supabase Postgres, a remote database host in the demo config | Free-tier hosting for the demos. |
| Docker orchestration | Partial | Two Dockerfiles | Single-container images. No compose file. |

## security_access: Security and access

| Concept | Status | Evidence | How it works here |
|---|---|---|---|
| Authentication | Partial | `src/commands/auth.rs`, `handle_connection` | A single shared password per server, checked per connection. Plain string comparison. No TLS. |
| Permissions | Not present | | |
| Multi-tenancy | Partial | `src/storage/namespace.rs`, `src/commands/namespace.rs` | Named namespaces with memory limits are implemented and unit-tested at the storage layer, but not reachable from the protocol (no route). |
| Security and compliance controls | Partial | listed below | Controls that support compliance work, not a certification. |

Controls that support compliance work:
- Optional password auth (`NOAUTH` until `AUTH`).
- Typed schemas that reject invalid values at write time (`TSET`).
- TTLs and time-series retention for data expiry (`EXPIRE`, `TS.CREATE ... RETENTION`).
- Memory caps with eviction.

Security gaps (worth knowing before a technical audience asks):
- **Passwords in logs.** The `CMD` log line prints every command's arguments before `AUTH` is handled, so the password is written to the log in plain text (`src/main.rs`).
- **Secrets in git.** The demo servers fall back to a hardcoded remote database host and password, and the whiteboard falls back to a full Postgres connection string, all in tracked files (see `redaction-check.md`).
- **Unauthenticated metrics.** The metrics endpoint is open, and it exposes key names through `rivetdb_key_access_count`.
- **Postgres certificate not verified.** `ssl: { rejectUnauthorized: false }` on the whiteboard Postgres pool.

## architecture: Architecture

| Concept | Status | Evidence | How it works here |
|---|---|---|---|
| Monolith vs microservices | Present | `src/main.rs` | A single-process server. The demos are separate small services that talk to it over TCP. |
| API architecture | Present | RESP over TCP; HTTP for metrics; REST and Socket.IO in the demos | A binary-safe text protocol for data, HTTP for ops. |
| Event-driven architecture | Partial | Socket.IO events in the whiteboard; the AOF message channel | Events in the demo layer. The server has no pub/sub. |
| Message queues | Partial | Tokio `mpsc` channel for the AOF (`AofMessage::{Write,Flush,Shutdown}`) | An in-process queue between the request path and the disk writer. |
| Horizontal partitioning | Partial | `DashMap` internal shards | In-memory lock striping inside one process. Not data partitioning across nodes. |
| Migrations | Partial | `CREATE TABLE IF NOT EXISTS` at whiteboard startup | An idempotent bootstrap for the demo's Postgres tables only. |

## forward_look: Forward look (recommendations, not exported to the CMS)

These are recommendations based on the code, not committed plans.

**Technical roadmap (recommendation)**
1. Stop logging command arguments (or redact `AUTH`), remove the hardcoded demo credentials, and rotate them.
2. Wire `load_aof` into startup for the DashMap state and call the AOF `shutdown()` on SIGTERM. Then the persistence story holds end to end.
3. Route the `NAMESPACE` commands, or remove the "multi-tenancy" claim from the materials.
4. Fix CI: run `cargo test --all`, drop `|| true`, clear the warnings so clippy passes, and move off `actions-rs` and `upload-artifact@v3`.
5. Raise coverage on the zero-coverage areas, starting with the RESP parser (fuzzing is already planned in `TESTING.md`) and the JSON, hash, and zset commands.
6. Track memory usage incrementally instead of scanning every key per write and per scrape. Implement real LRU using last-access time.
7. Add a root README and clean the repo root (binaries, zips, logs, code dump).

**Architecture roadmap (recommendation)**
1. Batch pipelined commands: one AOF message per pipeline batch, and fewer shard-lock acquisitions. This is the gap the paper identifies.
2. Add `PUBLISH`/`SUBSCRIBE` and `MULTI`/`EXEC`, so the demos don't need app-side fan-out and serialisation.
3. Add TLS and per-user ACLs before any networked use beyond demos.
4. Add snapshotting (RDB-style) next to AOF for faster restarts.
5. Replication (primary and replica streaming the AOF) as the first step toward running on more than one node.

## Not present

Alerting, uptime monitoring service, AWS / Azure / GCP services, Kubernetes, infrastructure-as-code, data residency controls, permissions, role-based access control (RBAC), SSO, audit logs, enterprise readiness, GraphQL, RPC, webhooks, CQRS, service mesh, distributed systems, eventual consistency, database sharding, strangler fig architecture.
