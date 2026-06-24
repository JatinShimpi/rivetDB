import { useEffect, useRef, useState, useCallback } from 'react'

export default function Whiteboard({ socket, userId, board, onLeave }) {
  const canvasRef = useRef(null)
  const isDrawing = useRef(false)
  const points = useRef([])
  const [users, setUsers] = useState(board.users || [])
  const [cursors, setCursors] = useState({})
  const [copied, setCopied] = useState(false)

  // Draw a stroke on canvas
  const drawStroke = useCallback((ctx, stroke) => {
    if (!stroke.points || stroke.points.length < 2) return
    ctx.beginPath()
    ctx.strokeStyle = stroke.color
    ctx.lineWidth = 3
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'
    ctx.moveTo(stroke.points[0].x, stroke.points[0].y)
    for (let i = 1; i < stroke.points.length; i++) {
      ctx.lineTo(stroke.points[i].x, stroke.points[i].y)
    }
    ctx.stroke()
  }, [])

  // Load existing strokes on mount
  useEffect(() => {
    const canvas = canvasRef.current
    const ctx = canvas.getContext('2d')
    canvas.width = window.innerWidth
    canvas.height = window.innerHeight
    ctx.fillStyle = '#0f0f0f'
    ctx.fillRect(0, 0, canvas.width, canvas.height)

    if (board.strokes) {
      board.strokes.forEach(s => drawStroke(ctx, s))
    }
  }, [])

  // Handle resize
  useEffect(() => {
    function resize() {
      const canvas = canvasRef.current
      const ctx = canvas.getContext('2d')
      const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
      canvas.width = window.innerWidth
      canvas.height = window.innerHeight
      ctx.putImageData(imageData, 0, 0)
    }
    window.addEventListener('resize', resize)
    return () => window.removeEventListener('resize', resize)
  }, [])

  // Socket events
  useEffect(() => {
    const canvas = canvasRef.current
    const ctx = canvas.getContext('2d')

    socket.on('stroke', (stroke) => drawStroke(ctx, stroke))

    socket.on('draw_move', ({ color, from, to }) => {
      ctx.beginPath()
      ctx.strokeStyle = color
      ctx.lineWidth = 3
      ctx.lineCap = 'round'
      ctx.lineJoin = 'round'
      ctx.moveTo(from.x, from.y)
      ctx.lineTo(to.x, to.y)
      ctx.stroke()
    })

    socket.on('board_cleared', () => {
      ctx.fillStyle = '#0f0f0f'
      ctx.fillRect(0, 0, canvas.width, canvas.height)
    })

    socket.on('user_joined', ({ user_id, display_name, color }) => {
      setUsers(prev => [...prev.filter(u => u.user_id !== user_id), { user_id, display_name, color }])
    })

    socket.on('user_left', ({ user_id }) => {
      setUsers(prev => prev.filter(u => u.user_id !== user_id))
      setCursors(prev => { const n = { ...prev }; delete n[user_id]; return n })
    })

    socket.on('cursor_move', ({ user_id, display_name, color, x, y }) => {
      setCursors(prev => ({ ...prev, [user_id]: { display_name, color, x, y } }))
    })

    return () => {
      socket.off('stroke')
      socket.off('draw_move')
      socket.off('board_cleared')
      socket.off('user_joined')
      socket.off('user_left')
      socket.off('cursor_move')
    }
  }, [drawStroke])

  function getPos(e) {
    const canvas = canvasRef.current
    const rect = canvas.getBoundingClientRect()
    const src = e.touches ? e.touches[0] : e
    return { x: src.clientX - rect.left, y: src.clientY - rect.top }
  }

  function onMouseDown(e) {
    isDrawing.current = true
    points.current = [getPos(e)]
  }

  function onMouseMove(e) {
    const pos = getPos(e)
    socket.emit('cursor', pos)

    if (!isDrawing.current) return
    const canvas = canvasRef.current
    const ctx = canvas.getContext('2d')
    points.current.push(pos)

    // Live draw locally + broadcast segment
    const pts = points.current
    if (pts.length >= 2) {
      const from = pts[pts.length - 2]
      const to = pts[pts.length - 1]
      ctx.beginPath()
      ctx.strokeStyle = board.color
      ctx.lineWidth = 3
      ctx.lineCap = 'round'
      ctx.lineJoin = 'round'
      ctx.moveTo(from.x, from.y)
      ctx.lineTo(to.x, to.y)
      ctx.stroke()
      socket.emit('draw_move', { from, to })
    }
  }

  function onMouseUp() {
    if (!isDrawing.current || points.current.length < 2) { isDrawing.current = false; return }
    isDrawing.current = false
    socket.emit('draw', { points: points.current })
    points.current = []
  }

  function copyCode() {
    navigator.clipboard.writeText(board.board_code)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const myUser = users.find(u => u.user_id === userId)
  const displayName = myUser?.display_name || localStorage.getItem('rivetdb_display_name') || 'You'

  return (
    <div style={{ position: 'relative', overflow: 'hidden', width: '100vw', height: '100vh' }}>
      <canvas
        ref={canvasRef}
        style={{ display: 'block', cursor: 'crosshair', touchAction: 'none' }}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
        onTouchStart={e => { e.preventDefault(); onMouseDown(e) }}
        onTouchMove={e => { e.preventDefault(); onMouseMove(e) }}
        onTouchEnd={onMouseUp}
      />

      {/* Other users' cursors */}
      {Object.entries(cursors).map(([uid, { display_name, color, x, y }]) => (
        <div key={uid} style={{ position: 'absolute', left: x, top: y, pointerEvents: 'none', transform: 'translate(8px, 8px)' }}>
          <div style={{ width: 10, height: 10, borderRadius: '50%', background: color, position: 'absolute', left: -14, top: -14, border: '2px solid #fff' }} />
          <div style={{ background: color, color: '#000', fontSize: 11, fontWeight: 600, padding: '2px 6px', borderRadius: 4, whiteSpace: 'nowrap' }}>
            {display_name}
          </div>
        </div>
      ))}

      {/* Top bar */}
      <div style={{ position: 'absolute', top: 16, left: '50%', transform: 'translateX(-50%)', display: 'flex', alignItems: 'center', gap: 12, background: '#1a1a1aee', border: '1px solid #2a2a2a', borderRadius: 12, padding: '8px 16px' }}>
        <span style={{ fontSize: 12, color: '#666' }}>Board</span>
        <span style={{ fontSize: 16, fontWeight: 700, color: '#4ECDC4', letterSpacing: 2 }}>{board.board_code}</span>
        <button onClick={copyCode} style={{ background: copied ? '#4ECDC4' : '#2a2a2a', border: 'none', color: copied ? '#000' : '#fff', padding: '4px 10px', borderRadius: 6, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
          {copied ? 'Copied!' : 'Copy'}
        </button>
      </div>

      {/* Online users */}
      <div style={{ position: 'absolute', top: 16, right: 16, background: '#1a1a1aee', border: '1px solid #2a2a2a', borderRadius: 12, padding: '12px 16px', minWidth: 160 }}>
        <div style={{ fontSize: 11, color: '#666', marginBottom: 8, fontWeight: 600 }}>ONLINE ({users.length})</div>
        {users.map(u => (
          <div key={u.user_id} style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
            <div style={{ width: 8, height: 8, borderRadius: '50%', background: u.color, flexShrink: 0 }} />
            <span style={{ fontSize: 13, color: u.user_id === userId ? '#fff' : '#aaa', fontWeight: u.user_id === userId ? 600 : 400 }}>
              {u.display_name}{u.user_id === userId ? ' (you)' : ''}
            </span>
          </div>
        ))}
      </div>

      {/* Bottom toolbar */}
      <div style={{ position: 'absolute', bottom: 20, left: '50%', transform: 'translateX(-50%)', display: 'flex', gap: 10 }}>
        <div style={{ background: '#1a1a1aee', border: `2px solid ${board.color}`, borderRadius: 8, padding: '6px 14px', fontSize: 12, color: board.color, fontWeight: 600 }}>
          ● {displayName}
        </div>
        <button onClick={() => socket.emit('clear')}
          style={{ background: '#1a1a1aee', border: '1px solid #444', color: '#ff6b6b', padding: '6px 14px', borderRadius: 8, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
          Clear All
        </button>
        <button onClick={onLeave}
          style={{ background: '#1a1a1aee', border: '1px solid #444', color: '#888', padding: '6px 14px', borderRadius: 8, cursor: 'pointer', fontSize: 12 }}>
          Leave
        </button>
      </div>
    </div>
  )
}
