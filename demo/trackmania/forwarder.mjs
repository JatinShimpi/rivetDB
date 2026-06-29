// =====================================================================
// RivetTM forwarder  (runs on EACH player's PC)
//
// Reads live telemetry from the signed "Data Sender" Openplanet plugin
// (local TCP, default 127.0.0.1:28765) and forwards it to the deployed
// RivetDB bridge.  No Club Edition / no unsigned plugin / no npm install.
//
// Run:   $env:PLAYER="YourName"; node forwarder.mjs
//   (BRIDGE_URL defaults to the deployed bridge below.)
// =====================================================================

import net from 'net';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SAMPLE_FILE = join(__dirname, 'tm-samples.txt');  // full samples dumped here for debugging

// ---- CONFIG (override with env vars) ---------------------------------
const PLAYER     = process.env.PLAYER      || 'Player1';
const BRIDGE_URL = process.env.BRIDGE_URL  || 'https://tm-bridge.onrender.com';
const WS_HOST    = process.env.DS_HOST     || '127.0.0.1';
const WS_PORT    = parseInt(process.env.DS_PORT || '28765');
const SPEED_SCALE= parseFloat(process.env.SPEED_SCALE || '1');   // set 3.6 if speed looks like m/s
const SEND_MS    = parseInt(process.env.SEND_MS || '200');
// ----------------------------------------------------------------------

let latestSpeed = 0;
let gotData = false;
const seenTypes = new Set();

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
  const type = (data && data.type) ? data.type : 'unknown';
  const src = (data && data.source) ? data.source : '';
  const key = `${type}|${src}`;
  if (!seenTypes.has(key)) {
    seenTypes.add(key);
    console.log(`[forwarder] NEW "${key}" (full sample written to tm-samples.txt)`);
    try { fs.appendFileSync(SAMPLE_FILE, `\n===== ${key} =====\n${JSON.stringify(data, null, 2)}\n`); } catch (e) {}
  }
  const spd = findSpeed(data);
  if (spd != null) { latestSpeed = Math.abs(spd) * SPEED_SCALE; gotData = true; }
}

setInterval(async () => {
  if (!gotData) return;
  try {
    await fetch(`${BRIDGE_URL}/telemetry`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ player: PLAYER, speed: Math.round(latestSpeed) })
    });
  } catch (e) { /* bridge briefly unreachable */ }
}, SEND_MS);

setInterval(() => {
  console.log(`[forwarder] player=${PLAYER}  speed=${Math.round(latestSpeed)}  source=${gotData ? 'live' : 'WAITING for Data Sender...'}`);
}, 2000);

function connectTcp() {
  console.log(`[forwarder] connecting to Data Sender ${WS_HOST}:${WS_PORT} ...`);
  const sock = net.connect(WS_PORT, WS_HOST, () => console.log('[forwarder] connected to Data Sender (tcp)'));
  let buf = '';
  sock.on('data', (chunk) => {
    buf += chunk.toString();
    let i;
    while ((i = buf.indexOf('\n')) >= 0) { handleMessage(buf.slice(0, i)); buf = buf.slice(i + 1); }
  });
  sock.on('error', (e) => console.log('[forwarder] tcp error:', e.message));
  sock.on('close', () => { console.log('[forwarder] closed, retrying in 2s'); setTimeout(connectTcp, 2000); });
}

console.log(`[forwarder] PLAYER=${PLAYER}  ->  BRIDGE=${BRIDGE_URL}`);
connectTcp();
