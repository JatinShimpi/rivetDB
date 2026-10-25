# Demo prep

## Can it still run?

Yes, locally. The database server and both demos run on one machine with no outside services. One catch matters more than anything else: **the demo servers fall back to hosted services when env vars are empty or unset.**
- Unset `RIVETDB_HOST`/`RIVETDB_PORT`/`RIVETDB_PASSWORD`: the demo connects to the hosted database endpoint.
- Unset `DATABASE_URL`: the whiteboard writes to the hosted Supabase Postgres.
- Because the code uses `||`, an empty string also falls back.

Always set all of them explicitly (below).

TODO(owner): Is the hosted database endpoint still running, and on which platform? Is the Render bridge still deployed?

What exists if something can't run:
- The paper's benchmark tables and figures (`paper/chapters/chapter6.tex`).
- `paper/Picture1.png`, `paper/Picture2.png`.
- `important documents/website.46af363f.mp4`. TODO(owner): what is this video? Check it before using it.

## Environment variables (names only)

Database server:
- `RIVETDB_PASSWORD` (optional; enables `AUTH`)
- `RUST_LOG` (optional; read by the tracing env filter)
- Config file `rivetdb.toml` (`[server]`, `[memory]`, `[persistence]`, `[logging]`, `[security]`)

Whiteboard server (`demo/whiteboard-react/server.js`):
- `RIVETDB_HOST`, `RIVETDB_PORT`, `RIVETDB_PASSWORD`, `DATABASE_URL`, `PORT` (default 3000)

Order book (`demo/trackmania/server.js`):
- `RIVETDB_HOST`, `RIVETDB_PORT`, `RIVETDB_PASSWORD`, `PORT` (default 8080)

Telemetry forwarder (`demo/trackmania/forwarder.mjs`, not used in the demo):
- `PLAYER`, `BRIDGE_URL`, `DS_HOST`, `DS_PORT`, `SPEED_SCALE`, `SEND_MS`

## Run it locally (PowerShell)

1. Database server, AOF off, no password:
   ```powershell
   cargo run --release -- --port 7878 --aof false
   ```
   Metrics: `http://localhost:9090/metrics`.
2. Client in a second terminal:
   ```powershell
   docker run -it --rm redis redis-cli -h host.docker.internal -p 7878
   ```
3. Order book:
   ```powershell
   cd demo/trackmania
   npm install
   $env:RIVETDB_HOST="127.0.0.1"; $env:RIVETDB_PORT="7878"; $env:RIVETDB_PASSWORD="x"; npm start
   ```
   Open `http://localhost:8080` and `http://localhost:8080/trade`. `RIVETDB_PASSWORD` must be non-empty so it doesn't fall back. With no password set on the server, any `AUTH` is accepted (`src/commands/auth.rs`).
4. Whiteboard (builds the client into `public/`, as the Dockerfile does):
   ```powershell
   cd demo/whiteboard-react
   npm install
   npm --prefix client install
   npm --prefix client run build
   Copy-Item -Recurse client/dist public
   $env:RIVETDB_HOST="127.0.0.1"; $env:RIVETDB_PORT="7878"; $env:RIVETDB_PASSWORD="x"; $env:DATABASE_URL="postgresql://demo:demo@127.0.0.1:5432/whiteboard"; npm start
   ```
   Open `http://localhost:3000` in two windows.
   - `DATABASE_URL` points at a local Postgres. If none is running, the server logs "Supabase unavailable (persistence disabled)" and carries on. That's fine for the demo, and it guarantees nothing goes to the hosted database.
   - These build steps write `node_modules/`, `client/dist/`, and `public/` inside the demo folder. All three are gitignored. If you'd rather not touch the folder, use the Dockerfile instead (`docker build -t whiteboard demo/whiteboard-react`).

## Before recording

**Integrations to disable or stub**
- Payments, email, SMS, analytics: none exist.
- Hosted database and Supabase: override every connection env var as above.
- Codecov: CI only, not part of the recording.
- Order-book bots: off by default. Start them on camera with the Start button.

**Screen hygiene**
- Run the server without `RIVETDB_PASSWORD`. Every command and its arguments is logged at INFO, so an `AUTH` password would appear in the terminal.
- If the server terminal is on screen, start it with `--log-level warn`.
- Don't open `server.js` from either demo in an editor on camera (see `redaction-check.md`).
- Keep the repo root listing off screen (binaries, zips, `important documents/`, `paper/report/`).

## Seed data per journey

No seed script exists for the database. The order book seeds itself (`seed()` in `demo/trackmania/server.js`: bids 99 to 90, asks 101 to 110, last price 100).

Proposed seed for Journeys A and B (paste into `redis-cli` before recording; dummy data only):
```
SET user:1001 "Asha Rao"
SET user:1002 "Vikram Shah"
HSET user:1001:profile city Pune plan pro
ZADD leaderboard 4200 asha 3900 vikram 4550 meera
LPUSH queue:jobs resize-42 resize-43 resize-44
JSON.SET order:77 $ '{"item":"keyboard","qty":2,"price":2499}'
TS.CREATE temp:room1 RETENTION 86400000 LABELS room 1
TS.ADD temp:room1 * 22.5
BF.RESERVE seen:emails 1000 0.01
BF.ADD seen:emails asha@example.com
```

| Journey | Data needed |
|---|---|
| A, drop-in Redis | Seed above, or type it live |
| B, extended types | `order:77`, `temp:room1`, `seen:emails` |
| C, whiteboard | Two browser windows, names "Asha" and "Vikram", board "Sprint planning" |
| D, order book | Built-in `seed()`, plus `POST /reset` before the take |


## Screenshots for the CMS

Use a local server with no password, and the seed commands above.

| # | Screen | Platform | Caption (140 characters or less) | Use |
|---|---|---|---|---|
| S1 | Split view: whiteboard in two windows with the same stroke, terminal showing `LRANGE` of that stroke | desktop | A live whiteboard on the database: strokes broadcast to every window and stored as list entries. | **Thumbnail** (16:9) and **hero** |
| S2 | `redis-cli` session: `SET`, `GET`, `ZADD`, `ZREVRANGE` | terminal | The stock redis-cli talks to the server unchanged. Strings, TTLs and sorted sets work as in Redis. | Screenshot |
| S3 | `redis-cli` session: `JSON.SET`/`JSON.GET`, `TS.ADD`/`TS.RANGE`, `QUERY ... FROM keys` | terminal | JSON paths, time-series and a SQL query over the keyspace, built into the server. | Screenshot |
| S4 | Order-book market page with bots running | desktop | An order book matched on sorted sets and hashes, with every trade written to a time series. | Screenshot |
| S5 | `http://localhost:9090/metrics` in the browser | desktop | Built-in Prometheus metrics: per-command counts and time, memory, keys by type, expirations. | Screenshot |

Keep the browser address bar out of S4 and S5 if it shows anything other than localhost.

Thumbnail: `rivetdb-case-study/thumbnail/thumbnail.png` (1600×900) is ready and used for `thumbnail` and `seo.metaImage`. It's a designed graphic of the real architecture, not a screenshot, so it needs no recreation label. To edit it, change `thumbnail.html` and re-render with headless Edge at a 1600×900 window size. The S1 screenshot remains the hero image.
