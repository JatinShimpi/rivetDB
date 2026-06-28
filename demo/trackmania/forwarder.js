// =====================================================================
// RivetTM forwarder  (runs on EACH player's PC)
//
// Reads live telemetry from the signed "Data Sender" Openplanet plugin
// (local WebSocket, default 127.0.0.1:28765) and forwards speed to the
// deployed RivetDB bridge.  No Club Edition / no unsigned plugin needed.
//
// Run:   node forwarder.js
// Config below can be overridden with environment variables.
// =====================================================================

import net from 'net';
// 'ws' is only needed for the (optional) WebSocket transport; loaded lazily so
// the default TCP mode needs ZERO npm install on a friend's PC (just Node).

// ---- CONFIG ----------------------------------------------------------
const PLAYER     = process.env.PLAYER      || 'Player1';                 // <- set your name on each PC
const BRIDGE_URL = process.env.BRIDGE_URL  || 'http://localhost:8080';   // <- deployed bridge base URL (no trailing slash)
const WS_HOST    = process.env.DS_HOST     || '127.0.0.1';
const WS_PORT    = parseInt(process.env.DS_PORT || '28765');
const TRANSPORT  = (process.env.TRANSPORT  || 'tcp').toLowerCase();       // 'tcp' (Data Sender is raw TCP) or 'ws'
const SPEED_SCALE= parseFloat(process.env.SPEED_SCALE || '1');           // set 3.6 if speed looks like m/s
const SEND_MS    = parseInt(process.env.SEND_MS || '200');               // how often we POST to the bridge
// ----------------------------------------------------------------------

let latestSpeed = 0;
let gotData = false;
const seenTypes = new Set();

// Recursively hunt for a numeric speed field anywhere in the JSON snapshot.
function findSpeed(obj) {
  if (obj == null) return null;
  if (Array.isArray(obj)) {
    for (const v of obj) { const r = findSpeed(v); if (r != null) return r; }
    return null;
  }
  if (typeof obj === 'object') {
    for (const [k, v] of Object.entries(obj)) {
      if (typeof v === 'number' && /^(spd|speed|displayspeed)$/i.test(k)) return v;
    }
    for (const v of Object.values(obj)) { const r = findSpeed(v); if (r != null) return r; }
  }
  return null;
}

function handleMessage(text) {
  let data;
  try { data = JSON.parse(text); } catch { return; }
  // Log the first sample of each distinct type+source (so we capture
  // vehicle telemetry AND the lap/checkpoint feed formats separately).
  const type = (data && data.type) ? data.type : 'unknown';
  const src = (data && data.source) ? data.source : '';
  const key = `${type}|${src}`;
  if (!seenTypes.has(key)) {
    seenTypes.add(key);
    console.log(`[forwarder] NEW "${key}":`, JSON.stringify(data).slice(0, 900));
  }
  const spd = findSpeed(data);
  if (spd != null) {
    latestSpeed = Math.abs(spd) * SPEED_SCALE;
    gotData = true;
  }
}

// POST the latest speed to the bridge on a fixed cadence.
setInterval(async () => {
  if (!gotData) return;
  try {
    await fetch(`${BRIDGE_URL}/telemetry`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ player: PLAYER, speed: Math.round(latestSpeed) })
    });
  } catch (e) { /* bridge briefly unreachable; ignore */ }
}, SEND_MS);

// status heartbeat
setInterval(() => {
  console.log(`[forwarder] player=${PLAYER}  speed=${Math.round(latestSpeed)}  source=${gotData ? 'live' : 'WAITING for Data Sender...'}`);
}, 2000);

// ---- connect to Data Sender -----------------------------------------
async function connectWs() {
  const { default: WebSocket } = await import('ws');
  const url = `ws://${WS_HOST}:${WS_PORT}`;
  console.log(`[forwarder] connecting (ws) to ${url} ...`);
  const ws = new WebSocket(url);
  ws.on('open', () => console.log('[forwarder] connected to Data Sender (ws)'));
  ws.on('message', (buf) => handleMessage(buf.toString()));
  ws.on('error', (e) => console.log('[forwarder] ws error:', e.message));
  ws.on('close', () => { console.log('[forwarder] ws closed, retrying in 2s'); setTimeout(connectWs, 2000); });
}

function connectTcp() {
  console.log(`[forwarder] connecting (tcp) to ${WS_HOST}:${WS_PORT} ...`);
  const sock = net.connect(WS_PORT, WS_HOST, () => console.log('[forwarder] connected to Data Sender (tcp)'));
  let buf = '';
  sock.on('data', (chunk) => {
    buf += chunk.toString();
    let i;
    while ((i = buf.indexOf('\n')) >= 0) { handleMessage(buf.slice(0, i)); buf = buf.slice(i + 1); }
  });
  sock.on('error', (e) => console.log('[forwarder] tcp error:', e.message));
  sock.on('close', () => { console.log('[forwarder] tcp closed, retrying in 2s'); setTimeout(connectTcp, 2000); });
}

console.log(`[forwarder] PLAYER=${PLAYER}  ->  BRIDGE=${BRIDGE_URL}`);
if (TRANSPORT === 'tcp') connectTcp(); else connectWs();
