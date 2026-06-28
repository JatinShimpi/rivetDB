import express from 'express';
import { createClient } from 'redis';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ---------------------------------------------------------------------------
// RivetDB connection (defaults to the same deployed instance as the whiteboard)
// ---------------------------------------------------------------------------
const redis = createClient({
  socket: {
    host: process.env.RIVETDB_HOST || 'reseau.proxy.rlwy.net',
    port: parseInt(process.env.RIVETDB_PORT || '13189'),
    reconnectStrategy: (retries) => Math.min(retries * 500, 5000)
  },
  password: process.env.RIVETDB_PASSWORD || 'jatin11234321'
});
redis.on('error', (err) => console.error('[RivetDB] error:', err.message));
redis.on('reconnecting', () => console.log('[RivetDB] reconnecting...'));
redis.on('ready', () => console.log('[RivetDB] ready'));

// RivetDB keys used by this demo:
//   tm:live          (HASH)  player -> {speed, raceTime, ts}   <- live state
//   tm:speed:<player>(TS)    timestamp -> speed                <- time-series
//   tm:leaderboard   (ZSET)  player -> bestTimeMs (lower=better)
const K_LIVE = 'tm:live';
const K_BOARD = 'tm:leaderboard';
const STALE_MS = 6000; // a player is "offline" if no telemetry for this long

const app = express();
app.use(express.json());

// allow the dashboard to be hosted anywhere + let the game POST freely
app.use((req, res, next) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Methods', 'GET,POST,OPTIONS');
  res.header('Access-Control-Allow-Headers', 'Content-Type');
  if (req.method === 'OPTIONS') return res.sendStatus(200);
  next();
});

app.use(express.static(join(__dirname, 'public')));

// ---- the Trackmania plugin posts here ~5x/sec --------------------------------
app.post('/telemetry', async (req, res) => {
  try {
    const { player, speed = 0, raceTime = 0, finished = false, finishTime = 0 } = req.body || {};
    if (!player) return res.status(400).json({ error: 'player required' });

    const ts = Date.now();
    await redis.hSet(K_LIVE, player, JSON.stringify({ speed, raceTime, ts }));

    // showcase the Time-Series data type (best-effort; never breaks telemetry)
    try {
      await redis.sendCommand(['TS.ADD', `tm:speed:${player}`, String(ts), String(speed)]);
    } catch (_) { /* TS optional */ }

    // leaderboard: keep each player's BEST (lowest) finish time
    if (finished && finishTime > 0) {
      const cur = await redis.zScore(K_BOARD, player);
      if (cur === null || finishTime < cur) {
        await redis.zAdd(K_BOARD, { score: finishTime, value: player });
        console.log(`[finish] ${player}: ${(finishTime / 1000).toFixed(3)}s`);
      }
    }
    res.json({ ok: true });
  } catch (err) {
    console.error('[telemetry] error:', err.message);
    res.status(500).json({ error: err.message });
  }
});

// ---- the dashboard polls here ------------------------------------------------
app.get('/state', async (req, res) => {
  try {
    const now = Date.now();
    const raw = await redis.hGetAll(K_LIVE);
    const live = Object.entries(raw)
      .map(([player, j]) => { try { return { player, ...JSON.parse(j) }; } catch { return null; } })
      .filter((x) => x && now - x.ts < STALE_MS)
      .sort((a, b) => b.speed - a.speed);

    // ascending by score => fastest (lowest time) first
    const lbRaw = await redis.zRangeWithScores(K_BOARD, 0, -1);
    const leaderboard = lbRaw.map((e) => ({ player: e.value, timeMs: e.score }));

    res.json({ live, leaderboard });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// ---- clear the leaderboard between demo runs ---------------------------------
app.post('/reset', async (req, res) => {
  await redis.del(K_BOARD);
  await redis.del(K_LIVE);
  console.log('[reset] leaderboard + live cleared');
  res.json({ ok: true });
});

app.get('/health', (req, res) => res.send('OK'));

async function main() {
  await redis.connect();
  console.log('[✓] Connected to RivetDB');
  const PORT = process.env.PORT || 8080;
  app.listen(PORT, '0.0.0.0', () => console.log(`[✓] Bridge listening on :${PORT}`));
}

main().catch((e) => { console.error('Fatal:', e); process.exit(1); });
