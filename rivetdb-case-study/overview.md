# Overview

## Project type and platforms

**Type:** a database server (backend infrastructure). It's an in-memory key-value store written in Rust that speaks the Redis wire protocol (RESP over TCP). Three demo apps sit on top of it: a real-time collaborative whiteboard (web app), a live order-book simulator (web app), and an earlier Trackmania telemetry bridge.

Evidence:
- Server binary and library crate `rivetdb` (`Cargo.toml`, `src/main.rs`, `src/lib.rs`).
- RESP parser and reply encoder (`src/protocol/frame.rs`, `src/protocol/reply.rs`).
- Async TCP listener on port 7878 (`src/main.rs`, `rivetdb.toml`).
- Prometheus metrics server on port 9090 (`src/metrics.rs`).
- Any standard Redis client works against it:
  - the demo scripts use `redis-cli` and `redis-benchmark` in Docker (`scripts/demo.ps1`, `scripts/benchmark.ps1`);
  - the demos use the Node `redis` client (`demo/*/package.json`);
  - the older whiteboard uses Python `redis` (`demo/whiteboard/server.py`).
- Demo apps:
  - `demo/whiteboard-react/`: React + Socket.IO + Node, with Postgres archiving.
  - `demo/trackmania/`: Express order-book simulator plus a telemetry forwarder.
  - `demo/whiteboard/`: Python + plain HTML, the first version.

**Platforms:** a cross-platform Rust binary (CI matrix targets Linux, Windows, macOS in `.github/workflows/ci.yml`), a Linux container (`Dockerfile`), and browser front ends for the demos.

**Context:** the repo also holds an academic paper and report about the system (`paper/`). TODO(owner): Was this a college project? If so, confirm it can be presented as ByteHint work (see `open-questions.md`).

**CMS projectType:** `api_backend`. It's a backend service: a TCP database server. `other` was the alternative. The demo apps are mentioned above and in the diagrams.

**Origin:** `engineering_team_project`. There's no client, and the paper credits student authors. TODO(owner): Confirm it can be shown as ByteHint work at all. This blocks publishing.

## One-line summary

A Redis-compatible database server in Rust that runs commands on different keys in parallel and builds in JSON, time-series, Bloom filters, schemas, and SQL queries.

(165 characters. This is the CMS `summary` and hero text.)

## The problem

**Summary.** Redis is the default in-memory store, but it runs commands on one thread, and JSON, time-series, and probabilistic types need separate modules. The project set out to build a wire-compatible alternative in Rust that uses every core and ships those types built in (`paper/chapters/chapter1.tex`, `chapter6.tex`, code comments in `src/commands/mod.rs`).

**Challenges**
1. **Single-threaded execution.** Independent writes from many clients queue behind one thread, even when they touch different keys.
2. **Features spread across modules.** JSON, time-series, and Bloom filters each need a separate Redis module to install and operate.
3. **No type checking on stored values.** Bad data reaches the store unchecked; the paper's answer is "type-safe data contracts" (the schema registry).
4. **Staying wire-compatible.** Existing clients, `redis-cli`, and `redis-benchmark` must work unchanged, so the server has to speak the Redis protocol exactly, including pipelined and partial frames.
5. **Durability without slowing writes.** Logging every write to disk inside the request path would cap throughput at disk speed.

**If it's not solved:** teams keep paying for single-thread limits and run several modules to get the data types they need.

TODO(owner): Was this driven by a real workload or user, or built as a research and learning project?

## The solution

**As a developer experiences it**
- Start one binary: `rivetdb --port 7878`. Point any Redis client at it.
- Use familiar commands: `SET`, `GET`, `LPUSH`, `ZADD`, `HSET`, `EXPIRE`.
- Use the added commands in the same connection:
  - `JSON.SET`, `TS.ADD`, `BF.ADD`;
  - `SCHEMA` and `TSET` for validated values;
  - `QUERY "SELECT key, type FROM keys WHERE key LIKE 'user:%'"`.
- Scrape `/metrics` with Prometheus, or call `STATS`, `HOTKEYS`, `SLOWLOG`, `MEMORY` from the client.
- Set `RIVETDB_PASSWORD` to require `AUTH`.

**Technically**
- Each TCP connection is a Tokio task, not an OS thread (`tokio::spawn` in `src/main.rs`).
- Incoming bytes are buffered and parsed into RESP frames, then routed by command name.
- All data lives in one `DashMap<String, ValueObject>`. DashMap is a sharded concurrent hash map, so commands on different keys don't wait on a single global lock (`src/storage/state.rs`).
- Successful write commands go to an append-only file through a bounded Tokio channel. A background task does the disk I/O, so writers never block on disk (`src/persistence/async_aof.rs`).
- TTLs sit in a min-heap. A background task wakes every 100 ms to remove expired keys (`run_expiry_loop` in `src/main.rs`).
- When memory goes over `max_memory`, the key with the lowest access count is evicted (`src/storage/eviction.rs`).

## Core features

| Feature | Where it lives |
|---|---|
| RESP2 wire protocol (simple, error, integer, bulk, array; pipelined frames) | `src/protocol/frame.rs`, `src/protocol/reply.rs`, buffer loop in `src/main.rs` |
| Strings, lists, sets, sorted sets, hashes, plus generic and admin commands (103 command names routed in total across `process_command` and `route_command`) | `src/commands/{string,list,set,zset,hash,basic,generic}.rs`, router `src/commands/mod.rs` |
| TTL and expiry: `EXPIRE`, `TTL`, background sweeper | `src/commands/expiry.rs`, `run_expiry_loop` in `src/main.rs` |
| Native JSON documents with JSONPath (`JSON.SET`, `JSON.GET`, `JSON.ARRAPPEND`, `JSON.NUMINCRBY`, and more) | `src/commands/json.rs` (`jsonpath-rust`) |
| Time-series with retention and labels (`TS.CREATE`, `TS.ADD`, `TS.RANGE`, `TS.MRANGE`, and more) | `src/storage/timeseries.rs`, `src/commands/timeseries.rs` |
| Bloom filters (`BF.RESERVE`, `BF.ADD`, `BF.EXISTS`, and more) | `src/storage/bloom.rs`, `src/commands/bloom.rs` (`bloomfilter`) |
| Typed schemas: define a schema, then `TSET` validates and `TGET` reads typed values | `src/schema/mod.rs`, `src/schema/validator.rs`, `src/commands/schema.rs` |
| SQL-style queries over the keyspace (`QUERY`, `EXPLAIN`; `SELECT`, `COUNT(*)`, `DELETE ... FROM keys WHERE ...`) | `src/query/executor.rs` (`sqlparser`), `src/commands/query.rs` |
| AOF persistence (always / everysec / no fsync policies), async writer, rewrite and load functions | `src/persistence/async_aof.rs`, `src/persistence/aof.rs` |
| Memory limit with LFU eviction | `src/storage/eviction.rs`, `[memory]` in `rivetdb.toml` |
| Password auth (`AUTH`, `NOAUTH` error, `RIVETDB_PASSWORD`) | `src/commands/auth.rs`, `handle_connection` in `src/main.rs` |
| Observability: Prometheus `/metrics`, `/health`, `/info`; `STATS`, `HOTKEYS`, `SLOWLOG` (commands over 1 ms) | `src/metrics.rs`, `src/commands/admin.rs`, `src/storage/state.rs` |
| Config: TOML file plus CLI flags (`--config`, `--host`, `--port`, `--log-level`, `--aof`) | `src/config.rs`, `Args` in `src/main.rs`, `docs/CONFIGURATION.md` |
| Named namespaces with per-namespace memory limits | `src/storage/namespace.rs`, `src/commands/namespace.rs`. **Implemented but not routed:** no `NAMESPACE` arm in `process_command` or `route_command`, so clients can't reach it. |

## Key user journeys

1. **Use it as a drop-in Redis**
   - Start: `cargo run --release` (binds 127.0.0.1:7878), or the Docker image (`CMD ./rivetdb --host 0.0.0.0 --port 7878`).
   - Connect: `docker run -it --rm redis redis-cli -h host.docker.internal -p 7878` (`important documents/imp commands.txt`).
   - Commands: `SET user:1 Asha`, `GET user:1`, `EXPIRE user:1 30`, `TTL user:1`, `ZADD board 120 asha`.
2. **Use the features Redis doesn't have built in**
   - `JSON.SET`, `JSON.GET` with a path.
   - `TS.ADD temp:room1 * 22.5`, `TS.RANGE`.
   - `BF.ADD seen user:1`, `BF.EXISTS`.
   - `SCHEMA DEFINE` a schema, then `TSET` (rejected when invalid), `TGET`.
   - `QUERY "SELECT COUNT(*) FROM keys WHERE type = 'zset'"`.
   - `scripts/demo.ps1` walks through these in sections.
3. **Collaborative whiteboard demo** (`demo/whiteboard-react/`)
   - Browser: the join screen (`JoinBoard.jsx`): create a named board or enter a 6-character code, then the drawing canvas (`Whiteboard.jsx`).
   - Socket.IO events: `create_board`, `join_board`, `draw_move` (live stroke segments), `draw` (finished stroke, stored with `LPUSH`), `cursor`, `clear`.
   - When the last user leaves, the board's strokes are archived to Postgres and its keys are deleted (`saveBoard` in `server.js`).
4. **Order-book simulator** (`demo/trackmania/server.js`)
   - Pages: `/` (market view, `market.html`), `/trade` (order entry, `trade.html`).
   - API: `POST /order {side, price, qty}`, `GET /book`, `POST /reset`, plus bot start and stop controls.
   - Bids and asks are sorted sets, quantities are hashes, trades are a capped list, and every fill is written with `TS.ADD ob:price`.

Note on the Trackmania telemetry demo: `demo/trackmania/forwarder.mjs` still posts to `/telemetry`, but that route only exists in commit `e3ee042`. The current `server.js` is the order-book simulator, and `public/index.html` (the telemetry dashboard) is no longer served. TODO(owner): keep the telemetry demo (restore it from `e3ee042`) or drop it?

## Key technical decisions

1. **Sharded concurrent map instead of a single-threaded event loop**
   - Chosen: `DashMap`, plus a Tokio task per connection, so commands on different keys run in parallel (`src/storage/state.rs`).
   - Alternative: the Redis model, one thread and no locks.
   - Why it fits, and the cost: the paper's own benchmark (`paper/chapters/chapter6.tex`) reports the database ahead of Redis on non-pipelined `INCR` (+42.8%) and `GET`/`SET` (about +14%). It's behind on pipelined `SET` (-93.9%), which the paper puts down to per-command locking and per-command channel sends. Those numbers come from one machine with `redis-benchmark`, and the raw runs aren't in the repo.
2. **AOF writes through a channel instead of inline disk writes**
   - Chosen: an `mpsc` channel with 10,000 slots feeding a background writer with a 64 KB buffer (`src/persistence/async_aof.rs`).
   - Alternative: write and fsync inside the command path.
   - Trade-off, stated in the code: `try_send` drops the command if the channel is full ("lose durability for speed").
3. **Speak RESP instead of a new protocol**
   - Chosen: a RESP2 parser, so existing Redis clients, `redis-cli`, and `redis-benchmark` work unchanged (`src/protocol/frame.rs`).
   - Alternative: a custom protocol and client library.
   - Why: every demo uses a stock Redis client (`redis` npm package, Python `redis`), and benchmarking uses Redis's own tool.
4. **Extra data types built into the server instead of plug-in modules**
   - Chosen: JSON, time-series, Bloom filters, and schemas are variants of one `ValueObject` enum (`src/storage/value.rs`).
   - Alternative: a module system.
   - Why: one binary, one memory estimate per value (`estimate_value_size`), one keyspace that the `QUERY` command can scan by type.
5. **Real-time fan-out in the app server, not the database**
   - Chosen: the whiteboard broadcasts with Socket.IO and stores state in the database (lists, hashes, sets). The order book serialises orders in Node (`serialize()` in `demo/trackmania/server.js`).
   - Why: the database has no `PUBLISH`/`SUBSCRIBE` or transactions (none in `src/commands/mod.rs`), so the demos handle broadcasting and ordering themselves.

## Delivery phases

From git history:
1. **Core server** (2026-01-13, `33d7801`). RESP server with command handling, memory management, and eviction.
2. **Feature-complete database and paper** (2026-04-23 to 2026-06-24). JSON, time-series, Bloom, schemas, SQL query, metrics, Dockerfile, password auth, and the paper sources.
3. **Collaborative whiteboard demo** (2026-06-25). React and Socket.IO board on the database, with Postgres archiving, reconnect handling, and code-gated joins.
4. **Telemetry bridge and order book** (2026-06-28 to 2026-06-30). Trackmania telemetry bridge (later replaced), then a live order-book simulator matched on sorted sets.

## Hardest parts

- **Persistence after the move to DashMap.** The server writes the AOF, and `load_aof` and `rewrite_aof` exist (`src/persistence/aof.rs`). But `main.rs` still has `// TODO: Load AOF - needs refactoring for DashMap approach`, so a restart starts empty.
- **Throughput under pipelining.** `paper/chapters/chapter5.tex` describes three optimisation passes: AOF fsync contention, pre-sizing the map, and async AOF. The pipelined `SET` result is still the weakest number in the benchmark.
- **Real-time demo resilience.** The commit history (2026-06-25) shows the work: Redis client auto-reconnect with backoff, making Postgres optional so the server starts without it, streaming partial strokes, and code-gated board joins. More recent work went into keeping the order book's bot-driven depth bounded (`0da7f83`, `c84093b`).
- The git history is short (16 commits) and the core server came in a few large commits, so churn isn't a useful signal for the server itself.

## Edge cases and failure handling (only what exists)

**Protocol and connections**
- Partial frames wait for more bytes (`UnexpectedEof`).
- Malformed frames get `protocol error` and the connection closes.
- Non-array commands get an error reply and the stream continues (`src/main.rs`).
- Negative or invalid bulk and array lengths, and a missing CRLF, are rejected (`src/protocol/frame.rs`).
- Unknown commands return `unknown command`. Type mismatches return errors (unit tests cover these in `tests/unit/commands/`).

**Auth**
- Commands before `AUTH` get `NOAUTH`. `PING`, `QUIT`, `COMMAND` are exempt (`src/commands/auth.rs`).

**Memory, persistence, expiry**
- Eviction protects the key just written. `noeviction` skips eviction (`src/storage/eviction.rs`).
- If the AOF writer fails to open, the server logs an error and keeps running without persistence. When its channel is full, commands are dropped from the AOF (`src/persistence/async_aof.rs`).
- Expired keys are removed by the sweeper. `is_expired` is checked on read (`src/commands/expiry.rs`).
- Commands slower than 1 ms are logged with a warning and kept in a 128-entry slow log (`src/main.rs`, `src/storage/state.rs`).

**Config**
- Unknown eviction policies fall back to `allkeys-lfu` with a warning. A missing config file falls back to defaults (`Config::load_or_default`).

**Demos**
- The Redis client reconnects with capped backoff (500 ms steps, max 5 s).
- Postgres being unreachable is not fatal.
- Order input is validated (`side`, `price > 0`, `qty > 0`, 400 otherwise).
- Concurrent orders are serialised so the book can't be corrupted (`demo/trackmania/server.js`).
