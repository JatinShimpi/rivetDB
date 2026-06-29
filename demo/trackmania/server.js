import express from 'express';
import { createClient } from 'redis';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ---------------------------------------------------------------------------
// RivetDB connection (same deployed instance as the whiteboard)
// ---------------------------------------------------------------------------
const redis = createClient({
  socket: {
    host: process.env.RIVETDB_HOST || 'reseau.proxy.rlwy.net',
    port: parseInt(process.env.RIVETDB_PORT || '13189'),
    reconnectStrategy: (r) => Math.min(r * 500, 5000)
  },
  password: process.env.RIVETDB_PASSWORD || 'jatin11234321'
});
redis.on('error', (e) => console.error('[RivetDB] error:', e.message));
redis.on('ready', () => console.log('[RivetDB] ready'));

// RivetDB keys (the order book LIVES here):
//   ob:bids  ZSET  member=price score=price   (resting buy price levels)
//   ob:asks  ZSET  member=price score=price   (resting sell price levels)
//   ob:bidq  HASH  price -> total qty at that level
//   ob:askq  HASH  price -> total qty at that level
//   ob:trades LIST recent trades (JSON)
//   ob:price  TS   last-trade price over time
//   ob:last   STR  last trade price
//   ob:volume INT  cumulative traded quantity

let lastPrice = 100;
let volume = 0;                   // cumulative traded qty (RivetDB has INCR but not INCRBY)
const chart = [];                 // in-memory sparkline {t, p}
function pushChart(t, p) { chart.push({ t, p }); if (chart.length > 240) chart.shift(); }

// --- serialize order processing so concurrent orders can't corrupt the book ---
let chain = Promise.resolve();
function serialize(fn) { const r = chain.then(fn, fn); chain = r.then(() => {}, () => {}); return r; }

async function loadSide(side) {
  const zkey = side === 'asks' ? 'ob:asks' : 'ob:bids';
  const qkey = side === 'asks' ? 'ob:askq' : 'ob:bidq';
  const rows = await redis.zRangeWithScores(zkey, 0, -1);
  const q = await redis.hGetAll(qkey);
  let levels = rows.map((r) => ({ price: r.score, qty: parseFloat(q[String(r.score)] || '0') })).filter((l) => l.qty > 0);
  levels.sort((a, b) => side === 'asks' ? a.price - b.price : b.price - a.price);
  return { zkey, qkey, levels };
}

// Price-time(ish) matching at price-level granularity.
async function matchOrder(orderSide, price, qty, trader) {
  price = Math.round(price);
  let remaining = Math.max(1, Math.round(qty));
  const opp = orderSide === 'buy' ? 'asks' : 'bids';
  const { zkey, qkey, levels } = await loadSide(opp);
  const trades = [];
  const touched = [];

  for (const lvl of levels) {
    if (remaining <= 0) break;
    const crosses = orderSide === 'buy' ? lvl.price <= price : lvl.price >= price;
    if (!crosses) break;
    const fill = Math.min(remaining, lvl.qty);
    remaining -= fill; lvl.qty -= fill;
    touched.push(lvl);
    trades.push({ price: lvl.price, qty: fill, side: orderSide, trader, ts: Date.now() });
  }

  for (const lvl of touched) {
    if (lvl.qty <= 0) { await redis.zRem(zkey, String(lvl.price)); await redis.hDel(qkey, String(lvl.price)); }
    else await redis.hSet(qkey, String(lvl.price), String(lvl.qty));
  }

  let rested = 0;
  if (remaining > 0) {
    rested = remaining;
    const mk = orderSide === 'buy' ? 'ob:bids' : 'ob:asks';
    const mq = orderSide === 'buy' ? 'ob:bidq' : 'ob:askq';
    await redis.zAdd(mk, { score: price, value: String(price) });
    await redis.hIncrBy(mq, String(price), remaining);
  }

  for (const t of trades) {
    await redis.lPush('ob:trades', JSON.stringify(t));
    lastPrice = t.price; volume += Math.round(t.qty); pushChart(t.ts, t.price);
    try { await redis.sendCommand(['TS.ADD', 'ob:price', String(t.ts), String(t.price)]); } catch (_) {}
  }
  if (trades.length) {
    await redis.lTrim('ob:trades', 0, 99);
    await redis.set('ob:last', String(lastPrice));
    await redis.set('ob:volume', String(volume));
  }

  return { filled: trades.reduce((s, t) => s + t.qty, 0), rested, trades };
}

async function seed() {
  await redis.del('ob:bids', 'ob:asks', 'ob:bidq', 'ob:askq', 'ob:trades', 'ob:volume');
  lastPrice = 100; volume = 0; chart.length = 0;
  for (let i = 1; i <= 6; i++) {
    await redis.zAdd('ob:bids', { score: 100 - i, value: String(100 - i) });
    await redis.hSet('ob:bidq', String(100 - i), String(4 * i + 3));
    await redis.zAdd('ob:asks', { score: 100 + i, value: String(100 + i) });
    await redis.hSet('ob:askq', String(100 + i), String(4 * i + 3));
  }
  await redis.set('ob:last', '100');
  console.log('[book] seeded around 100');
}

// ---------------------------- bot traders ----------------------------------
let botTimer = null;
function startBots() { if (botTimer) return; botTimer = setInterval(() => serialize(botTick).catch(() => {}), 650); }
function stopBots() { if (botTimer) { clearInterval(botTimer); botTimer = null; } }
async function botTick() {
  const mid = lastPrice || 100;
  const trader = 'bot' + (1 + Math.floor(Math.random() * 4));
  // 55% cross (consumes liquidity, makes trades, moves price) keeps the book bounded;
  // 45% small resting liquidity near the mid. Small qtys = a realistic-looking book.
  if (Math.random() < 0.55) {
    const side = Math.random() < 0.5 ? 'buy' : 'sell';
    const price = side === 'buy' ? mid + 1 + Math.floor(Math.random() * 2) : mid - 1 - Math.floor(Math.random() * 2);
    await matchOrder(side, price, 1 + Math.floor(Math.random() * 6), trader);
  } else {
    const side = Math.random() < 0.5 ? 'buy' : 'sell';
    const off = 1 + Math.floor(Math.random() * 4);
    const price = side === 'buy' ? mid - off : mid + off;
    await matchOrder(side, price, 1 + Math.floor(Math.random() * 5), trader);
  }
}

// ------------------------------- HTTP ---------------------------------------
const app = express();
app.use(express.json());
app.use((req, res, next) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Methods', 'GET,POST,OPTIONS');
  res.header('Access-Control-Allow-Headers', 'Content-Type');
  if (req.method === 'OPTIONS') return res.sendStatus(200);
  next();
});
app.use(express.static(join(__dirname, 'public'), { index: false }));
app.get('/', (req, res) => res.sendFile(join(__dirname, 'public', 'market.html')));
app.get('/trade', (req, res) => res.sendFile(join(__dirname, 'public', 'trade.html')));

app.post('/order', async (req, res) => {
  try {
    const { side, price, qty, trader } = req.body || {};
    if (!['buy', 'sell'].includes(side) || !(price > 0) || !(qty > 0))
      return res.status(400).json({ error: 'need side(buy/sell), price>0, qty>0' });
    const result = await serialize(() => matchOrder(side, price, qty, (trader || 'anon').slice(0, 16)));
    res.json(result);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/book', async (req, res) => {
  try {
    const askRows = await redis.zRangeWithScores('ob:asks', 0, -1);
    const bidRows = await redis.zRangeWithScores('ob:bids', 0, -1);
    const askq = await redis.hGetAll('ob:askq');
    const bidq = await redis.hGetAll('ob:bidq');
    const asks = askRows.map((r) => ({ price: r.score, qty: parseFloat(askq[String(r.score)] || '0') }))
      .filter((a) => a.qty > 0).sort((a, b) => a.price - b.price).slice(0, 12);
    const bids = bidRows.map((r) => ({ price: r.score, qty: parseFloat(bidq[String(r.score)] || '0') }))
      .filter((b) => b.qty > 0).sort((a, b) => b.price - a.price).slice(0, 12);
    const trades = (await redis.lRange('ob:trades', 0, 25)).map((t) => { try { return JSON.parse(t); } catch { return null; } }).filter(Boolean);
    const bestBid = bids[0]?.price ?? null;
    const bestAsk = asks[0]?.price ?? null;
    res.json({
      bids, asks, last: lastPrice,
      spread: (bestBid != null && bestAsk != null) ? +(bestAsk - bestBid).toFixed(2) : null,
      bestBid, bestAsk, trades, chart, volume, botsOn: !!botTimer
    });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.post('/bots', (req, res) => { (req.body?.on ? startBots() : stopBots()); res.json({ botsOn: !!botTimer }); });
app.post('/reset', async (req, res) => { await serialize(seed); res.json({ ok: true }); });
app.get('/health', (req, res) => res.send('OK'));

async function main() {
  await redis.connect();
  console.log('[✓] Connected to RivetDB');
  const asks = await redis.zCard('ob:asks');
  if (!asks) await seed();
  else {
    const lp = await redis.get('ob:last'); if (lp) lastPrice = parseFloat(lp);
    const v = await redis.get('ob:volume'); if (v) volume = parseInt(v) || 0;
  }
  // Bots are OFF by default to avoid background load when nobody is presenting.
  // Press "Start market" in the dashboard (or POST /bots {on:true}) during the demo.
  const PORT = process.env.PORT || 8080;
  app.listen(PORT, '0.0.0.0', () => console.log(`[✓] Order book bridge on :${PORT}`));
}
main().catch((e) => { console.error('Fatal:', e); process.exit(1); });
