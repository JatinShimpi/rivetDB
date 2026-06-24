import express from 'express';
import { createServer } from 'http';
import { Server } from 'socket.io';
import { createClient } from 'redis';
import pg from 'pg';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { randomBytes } from 'crypto';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const app = express();
const httpServer = createServer(app);
const io = new Server(httpServer, { cors: { origin: '*' } });

app.use(express.static(join(__dirname, 'public')));
app.get('*', (req, res) => res.sendFile(join(__dirname, 'public', 'index.html')));

// RivetDB (Railway)
const redis = createClient({
  socket: { host: process.env.RIVETDB_HOST || 'reseau.proxy.rlwy.net', port: parseInt(process.env.RIVETDB_PORT || '13189') },
  password: process.env.RIVETDB_PASSWORD || 'jatin11234321'
});

// Supabase PostgreSQL
const pool = new pg.Pool({
  connectionString: process.env.DATABASE_URL || 'postgresql://postgres:DqOJOpv7yUIrXLFg@db.cihmvebhxfzzfmyupxjl.supabase.co:5432/postgres',
  ssl: { rejectUnauthorized: false }
});

function generateBoardCode() {
  return randomBytes(3).toString('hex').toUpperCase();
}

function generateColor(userId) {
  const colors = ['#FF6B6B', '#4ECDC4', '#45B7D1', '#96CEB4', '#FFEAA7', '#DDA0DD', '#98FB98', '#FFB347'];
  let hash = 0;
  for (let i = 0; i < userId.length; i++) {
    hash = userId.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

async function saveBoard(boardCode) {
  try {
    const strokes = await redis.lRange(`board:${boardCode}:strokes`, 0, -1);
    if (strokes.length === 0) {
      console.log(`[→] Board ${boardCode} empty, skipping save`);
      return;
    }

    const boardResult = await pool.query(
      `INSERT INTO boards (board_code, ended_at, stroke_count)
       VALUES ($1, NOW(), $2)
       ON CONFLICT (board_code) DO UPDATE SET ended_at = NOW(), stroke_count = $2
       RETURNING id`,
      [boardCode, strokes.length]
    );
    const boardId = boardResult.rows[0].id;

    for (const strokeJson of strokes) {
      const s = JSON.parse(strokeJson);
      await pool.query(
        'INSERT INTO strokes (board_id, user_id, display_name, color, points) VALUES ($1, $2, $3, $4, $5)',
        [boardId, s.user_id, s.display_name, s.color, JSON.stringify(s.points)]
      );
    }

    await redis.del(
      `board:${boardCode}:strokes`,
      `board:${boardCode}:online`,
      `board:${boardCode}:users`,
      `board:${boardCode}:cursors`
    );

    console.log(`[✓] Board ${boardCode} saved (${strokes.length} strokes) → Supabase`);
  } catch (err) {
    console.error(`[✗] Save failed for ${boardCode}:`, err.message);
  }
}

io.on('connection', (socket) => {
  let currentBoard = null;
  let currentUser = null;

  async function joinBoard(boardCode, user_id, display_name) {
    const color = generateColor(user_id);
    currentBoard = boardCode;
    currentUser = { user_id, display_name, color };

    socket.join(boardCode);

    await redis.sAdd(`board:${boardCode}:online`, user_id);
    await redis.hSet(`board:${boardCode}:users`, user_id, JSON.stringify({ display_name, color }));

    const usersHash = await redis.hGetAll(`board:${boardCode}:users`);
    const users = Object.entries(usersHash).map(([id, data]) => ({
      user_id: id,
      ...JSON.parse(data)
    }));

    return { color, users };
  }

  socket.on('create_board', async ({ user_id, display_name }) => {
    const boardCode = generateBoardCode();
    const { color, users } = await joinBoard(boardCode, user_id, display_name);
    socket.emit('board_ready', { board_code: boardCode, color, users, strokes: [] });
    console.log(`[+] ${display_name} created board ${boardCode}`);
  });

  socket.on('join_board', async ({ board_code, user_id, display_name }) => {
    const boardCode = board_code.toUpperCase().trim();
    const { color, users } = await joinBoard(boardCode, user_id, display_name);

    const rawStrokes = await redis.lRange(`board:${boardCode}:strokes`, 0, -1);
    const strokes = rawStrokes.reverse().map(s => JSON.parse(s));

    socket.emit('board_ready', { board_code: boardCode, color, users, strokes });
    socket.to(boardCode).emit('user_joined', { user_id, display_name, color });
    console.log(`[+] ${display_name} joined board ${boardCode}`);
  });

  socket.on('draw_move', ({ from, to }) => {
    if (!currentBoard || !currentUser) return;
    socket.to(currentBoard).emit('draw_move', { color: currentUser.color, from, to });
  });

  socket.on('draw', async ({ points }) => {
    if (!currentBoard || !currentUser) return;
    const stroke = {
      user_id: currentUser.user_id,
      display_name: currentUser.display_name,
      color: currentUser.color,
      points
    };
    await redis.lPush(`board:${currentBoard}:strokes`, JSON.stringify(stroke));
    await redis.lTrim(`board:${currentBoard}:strokes`, 0, 999);
    io.to(currentBoard).emit('stroke', stroke);
  });

  socket.on('cursor', ({ x, y }) => {
    if (!currentBoard || !currentUser) return;
    socket.to(currentBoard).emit('cursor_move', {
      user_id: currentUser.user_id,
      display_name: currentUser.display_name,
      color: currentUser.color,
      x, y
    });
  });

  socket.on('clear', async () => {
    if (!currentBoard) return;
    await redis.del(`board:${currentBoard}:strokes`);
    io.to(currentBoard).emit('board_cleared');
  });

  socket.on('disconnect', async () => {
    if (!currentBoard || !currentUser) return;
    await redis.sRem(`board:${currentBoard}:online`, currentUser.user_id);
    await redis.hDel(`board:${currentBoard}:users`, currentUser.user_id);
    await redis.hDel(`board:${currentBoard}:cursors`, currentUser.user_id);

    socket.to(currentBoard).emit('user_left', { user_id: currentUser.user_id });
    console.log(`[-] ${currentUser.display_name} left board ${currentBoard}`);

    const online = await redis.sCard(`board:${currentBoard}:online`);
    if (online === 0) {
      console.log(`[→] Board ${currentBoard} empty — saving to Supabase...`);
      await saveBoard(currentBoard);
    }
  });
});

async function main() {
  await redis.connect();
  console.log('[✓] Connected to RivetDB');

  try {
    await pool.query(`
      CREATE TABLE IF NOT EXISTS boards (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        board_code VARCHAR(8) UNIQUE NOT NULL,
        created_at TIMESTAMPTZ DEFAULT NOW(),
        ended_at TIMESTAMPTZ,
        stroke_count INTEGER DEFAULT 0
      );
      CREATE TABLE IF NOT EXISTS strokes (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        board_id UUID REFERENCES boards(id) ON DELETE CASCADE,
        user_id VARCHAR(64) NOT NULL,
        display_name VARCHAR(64) NOT NULL,
        color VARCHAR(16) NOT NULL,
        points JSONB NOT NULL,
        created_at TIMESTAMPTZ DEFAULT NOW()
      );
    `);
    console.log('[✓] Supabase tables ready');
  } catch (err) {
    console.error('[!] Supabase unavailable (persistence disabled):', err.message);
  }

  const PORT = process.env.PORT || 3000;
  httpServer.listen(PORT, '0.0.0.0', () => {
    console.log(`[✓] Server on port ${PORT}`);
  });
}

main().catch(console.error);
