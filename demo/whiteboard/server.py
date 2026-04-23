"""
RivetDB Collaborative Whiteboard - WebSocket Server
Connects browsers to RivetDB for real-time cursor and drawing sync.

Requirements: pip install websockets redis
Usage: python server.py
"""

import asyncio
import json
import websockets
import redis
from datetime import datetime

# RivetDB connection (Redis-compatible)
r = redis.Redis(host='localhost', port=7878, decode_responses=True)

# Connected clients
clients = {}

async def broadcast(message, exclude=None):
    """Send message to all clients except sender"""
    for user_id, ws in list(clients.items()):  # Use list() to avoid iteration error
        if user_id != exclude:
            try:
                await ws.send(message)
            except:
                pass

async def handle_client(websocket):
    """Handle a single client connection"""
    user_id = f"user_{len(clients) + 1}"
    colors = ["#FF6B6B", "#4ECDC4", "#45B7D1", "#96CEB4", "#FFEAA7"]
    color = colors[len(clients) % len(colors)]

    
    clients[user_id] = websocket
    print(f"[+] {user_id} connected ({len(clients)} online)")
    
    # Store user in RivetDB
    r.sadd("whiteboard:online", user_id)
    r.hset(f"whiteboard:user:{user_id}", mapping={
        "color": color,
        "joined": datetime.now().isoformat()
    })
    
    # Send user their ID and color
    await websocket.send(json.dumps({
        "type": "init",
        "user_id": user_id,
        "color": color,
        "online": list(clients.keys())
    }))
    
    # Notify others
    await broadcast(json.dumps({
        "type": "user_joined",
        "user_id": user_id,
        "color": color
    }), exclude=user_id)
    
    # Load existing drawings from RivetDB
    strokes = r.lrange("whiteboard:strokes", 0, -1)
    for stroke in strokes:
        await websocket.send(json.dumps({
            "type": "stroke",
            "data": json.loads(stroke)
        }))
    
    try:
        async for message in websocket:
            data = json.loads(message)
            
            if data["type"] == "cursor":
                # Store cursor position in RivetDB
                r.hset("whiteboard:cursors", user_id, 
                       f"{data['x']},{data['y']}")
                
                # Broadcast to others
                await broadcast(json.dumps({
                    "type": "cursor",
                    "user_id": user_id,
                    "x": data["x"],
                    "y": data["y"],
                    "color": color
                }), exclude=user_id)
                
            elif data["type"] == "draw":
                # Store stroke in RivetDB
                stroke_data = {
                    "user_id": user_id,
                    "color": color,
                    "points": data["points"]
                }
                r.lpush("whiteboard:strokes", json.dumps(stroke_data))
                r.ltrim("whiteboard:strokes", 0, 999)  # Keep last 1000
                
                # Broadcast to all
                await broadcast(json.dumps({
                    "type": "stroke",
                    "data": stroke_data
                }))
                
            elif data["type"] == "clear":
                # Clear whiteboard in RivetDB
                r.delete("whiteboard:strokes")
                await broadcast(json.dumps({"type": "clear"}))
                
    except websockets.exceptions.ConnectionClosed:
        pass
    finally:
        # Cleanup
        del clients[user_id]
        r.srem("whiteboard:online", user_id)
        r.hdel("whiteboard:cursors", user_id)
        print(f"[-] {user_id} disconnected ({len(clients)} online)")
        
        await broadcast(json.dumps({
            "type": "user_left",
            "user_id": user_id
        }))

async def main():
    print("=" * 50)
    print("RivetDB Collaborative Whiteboard Server")
    print("=" * 50)
    
    # Test RivetDB connection
    try:
        r.ping()
        print("[✓] Connected to RivetDB on port 7878")
    except:
        print("[✗] Cannot connect to RivetDB! Start it first:")
        print("    cargo run --release")
        return
    
    # Clear old data
    r.delete("whiteboard:online", "whiteboard:cursors", "whiteboard:strokes")
    
    print("[✓] Starting WebSocket server on ws://0.0.0.0:8765")
    print("\nOn OTHER PCs, open whiteboard.html and it will connect!")
    print("Make sure all PCs are on the same network.")
    print("Press Ctrl+C to stop\n")
    
    async with websockets.serve(handle_client, "0.0.0.0", 8765):
        await asyncio.Future()  # Run forever

if __name__ == "__main__":
    asyncio.run(main())
