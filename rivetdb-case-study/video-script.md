# Video script (about 4 minutes)

Format: talking points, not a word-for-word script. This is infrastructure, so the "product demo" is a terminal session first, then two apps built on top of it.

**Record on:** desktop, 1440 px wide.
- Left: Windows Terminal at a large font (18 pt or more) running `redis-cli` in Docker.
- Right: Chrome for the whiteboard and order book.
- Use a local server with no password and AOF off (see `demo-prep.md`), so no credentials appear on screen.

## 1. Hook (0:15)

- "A Redis-compatible database, written from scratch in Rust, with JSON, time-series, Bloom filters, schemas, and SQL built in. Your existing Redis client just works."
- On screen: `redis-cli` connected to port 7878, running `SET`, then `QUERY "SELECT COUNT(*) FROM keys"`.

## 2. Context (0:30)

- Type: a database server. It speaks the Redis protocol over TCP.
- Users: developers who already use Redis clients.
- Core constraint: stay wire-compatible with Redis, so `redis-cli`, `redis-benchmark`, and client libraries work unchanged, while adding features Redis needs separate modules for.
- Second constraint: use all CPU cores. A sharded map lets commands on different keys run at the same time.

## 3. Product demo (1:30)

**Journey A: drop-in Redis (0:20)**, terminal:
```
SET user:1001 "Asha Rao"
GET user:1001
EXPIRE user:1001 30
TTL user:1001
ZADD leaderboard 4200 asha 3900 vikram 4550 meera
ZREVRANGE leaderboard 0 2
```
- Point out: this is the stock `redis-cli`, not a custom client.

**Journey B: what Redis needs modules for (0:30)**, terminal:
```
JSON.SET order:77 $ '{"item":"keyboard","qty":2,"price":2499}'
JSON.GET order:77 $.qty
TS.ADD temp:room1 * 22.5
TS.ADD temp:room1 * 23.1
TS.RANGE temp:room1 - +
BF.ADD seen:emails asha@example.com
BF.EXISTS seen:emails vikram@example.com
QUERY "SELECT key, type FROM keys WHERE key LIKE 'temp:%'"
```
- Optional, if time allows: `SCHEMA DEFINE` a small schema, then a `TSET` that fails validation.
- Point out: one binary and one keyspace. `QUERY` can list everything by type.
- Dry-run these first. `scripts/demo.ps1` only covers the Redis-style sections (strings to admin), not JSON, TS, BF, or QUERY.

**Journey C: collaborative whiteboard (0:25)**, two browser windows side by side:
1. Window 1: enter the name `Asha`, create the board "Sprint planning", note the 6-character code.
2. Window 2: enter the name `Vikram`, enter the code, join.
3. Draw in window 1. The stroke appears live in window 2, along with the cursor.
4. Back in the terminal: `LRANGE board:<CODE>:strokes 0 0` shows the stored stroke.
- Point out: the board state lives in the database. Socket.IO only carries the broadcast.

**Journey D: order book (0:15)**
1. Open `/` (market view) and `/trade`.
2. Place `buy 102 x 5`. A fill appears.
3. Press **Start** on the bots and watch the book move.
4. Terminal: `ZRANGE ob:bids 0 -1` and `TS.RANGE ob:price - +`.
- Point out: price levels are sorted sets, quantities are hashes, every trade is also a time-series point.

## 4. Under the hood (1:15)

1. `diagrams/architecture.png` (0:25). Reveal in order: Clients → TCP listener (task per connection) → RESP parser and AUTH → Router → DashMap keyspace → background tasks (expiry, eviction, AOF writer) → metrics on 9090.
   - Stack at a glance: Rust and Tokio, DashMap, Axum for metrics, sqlparser, jsonpath-rust, bloomfilter, Node demos.
2. `diagrams/flow-command.png` (0:25). Follow one `SET` from bytes to reply. Pause on "other shards free" and on `try_send` to the AOF writer.
   - Decision 1: a sharded map instead of one thread, with the benchmark result and its trade-off (below).
   - Decision 2: disk writes go through a channel, so a slow disk never blocks a client.
3. Benchmark slide (0:15), from `paper/chapters/chapter6.tex`. Label it "Single machine, redis-benchmark, as reported in the project paper":
   - Non-pipelined `INCR`: 14,741 vs 10,322 req/s (+42.8%).
   - Non-pipelined `GET`: +14.3%.
   - Show the honest flip side too: pipelined `SET` is far behind Redis.
4. `diagrams/flow-whiteboard.png` (0:10). How a real-time app sits on top.

## 5. Results (0:20)

- On screen: the benchmark table above, "78 automated tests passing", and the `/metrics` page in a browser.
- Don't show the coverage figure as a strength (12.64%).
- TODO(owner): any users, deployments, or results beyond the paper.

## 6. Close (0:10)

- "Built by the ByteHint team."
- CTA: TODO(owner): pick one, for example "Need systems-level engineering in Rust? Talk to us." plus the ByteHint contact link.

## Short cut (90 to 120 sec)

Keep these moments:
1. Hook: stock `redis-cli` hitting the server, `SET`/`GET` (0:10).
2. `JSON.SET`/`JSON.GET` and `TS.ADD`/`TS.RANGE` back to back (0:15).
3. `QUERY "SELECT key, type FROM keys ..."` (0:10).
4. Whiteboard: a stroke appearing in the second window (0:15).
5. Architecture diagram, one line on the sharded map and one on the async AOF (0:25).
6. Benchmark slide with the INCR number and the pipelining caveat (0:15).
7. "Built by the ByteHint team" + CTA (0:10).

Total: about 100 seconds.

Don't claim on camera:
- Persistence across restarts (AOF replay isn't wired).
- Multi-tenancy (namespace commands aren't routed).
- The Trackmania telemetry demo (its endpoint was replaced).

## CMS video slots

| Slot | Content | Target length |
|---|---|---|
| `videoFull` | Segments 1 to 6 above | about 240 s |
| `videoShort` | The "Short cut" list above | about 100 s |

## Transcript draft: videoFull

DRAFT. Correct this after recording; it's built from the talking points, not from the recording.

This is a Redis-compatible database, written from scratch in Rust, with JSON, time-series, Bloom filters, schemas, and SQL built in. Your existing Redis client just works.

It's a database server that speaks the Redis protocol over TCP. It's for developers who already use Redis clients. Two constraints shaped it: stay wire-compatible, so redis-cli, redis-benchmark, and client libraries work unchanged, and use every CPU core, so commands on different keys run at the same time.

This is the stock redis-cli. I set a key, read it, give it a 30 second expiry, and check the TTL. I add three players to a sorted-set leaderboard and read the top of it back. Nothing custom on the client side.

Now the parts Redis needs modules for. I store a JSON order and read one field back with a path. I add two temperature readings to a time series and read the range. I add an email to a Bloom filter and check another. Then a SQL query lists every key that starts with temp, with its type. One binary, one keyspace.

On top of it sits a collaborative whiteboard. Two browser windows join the same board with a six-character code. A stroke drawn in one appears live in the other, and in the terminal the stored stroke is right there in a list.

And an order book. Price levels are sorted sets, quantities are hashes, and every trade is also written to a time series. I place a buy order, it fills, and when the bots start, the book moves.

Under the hood: Tokio runs a task per connection. A hand-written parser reads the Redis protocol, including partial and batched requests. The keyspace is a sharded DashMap, so commands on different keys don't wait on each other. Writes go to an append-only file through a background channel, so a slow disk never blocks a client. The project's own benchmark, run on a single machine with redis-benchmark, reports non-pipelined INCR at about 42 percent faster than Redis. It's also honest about the trade-off: heavily pipelined writes are far slower, because each command takes a lock and sends a log message.

Built by the ByteHint team.

## Transcript draft: videoShort

DRAFT. Correct this after recording.

A Redis-compatible database written from scratch in Rust. The stock redis-cli works against it. On top of the usual commands, it has JSON with paths, time-series, Bloom filters, and a SQL query over the whole keyspace, built in. A live whiteboard runs on it, with strokes appearing in a second window as they're drawn. Inside, a sharded map lets commands on different keys run in parallel, and disk writes happen in the background. The project's own single-machine benchmark puts non-pipelined INCR about 42 percent ahead of Redis, and it reports where Redis still wins. Built by the ByteHint team.
