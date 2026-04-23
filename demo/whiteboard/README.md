# RivetDB Collaborative Whiteboard Demo

A real-time collaborative whiteboard demonstrating RivetDB's capabilities.

## Quick Start (Single Laptop)

### 1. Install Python dependencies
```bash
pip install websockets redis
```

### 2. Start RivetDB
```bash
cd d:\dev\rivetDb
cargo run --release
```

### 3. Start WebSocket server
```bash
cd d:\dev\rivetDb\demo\whiteboard
python server.py
```

### 4. Open whiteboard
Open `whiteboard.html` in **multiple browser tabs**.

---

## Network Demo (Multiple Laptops)

### On Server Laptop

1. **Find your IP:**
```powershell
ipconfig
# Look for: IPv4 Address (e.g., 10.226.65.93)
```

2. **Disable firewall temporarily (run as Administrator):**
```powershell
netsh advfirewall set allprofiles state off
```

3. **Start RivetDB and server.py** (same as above)

### On Client Laptops

1. Copy `whiteboard.html` to the laptop
2. Open in browser
3. Enter the server IP when prompted (e.g., `10.226.65.93`)

### After Demo - Re-enable Firewall!
```powershell
netsh advfirewall set allprofiles state on
```

---

## Features Demonstrated

| Feature | RivetDB Command | Purpose |
|---------|-----------------|---------|
| Online users | `SADD whiteboard:online user_1` | Track who's connected |
| Cursor positions | `HSET whiteboard:cursors user_1 "100,200"` | Real-time cursor sync |
| Drawing strokes | `LPUSH whiteboard:strokes '{...}'` | Persistent drawings |
| User profiles | `HSET whiteboard:user:1 color #FF6B6B` | User metadata |

## Files

- `server.py` - Python WebSocket server (connects to RivetDB)
- `whiteboard.html` - Browser client (canvas + WebSocket)
