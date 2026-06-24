import { useState, useEffect } from 'react'

const s = {
  page: { minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#0f0f0f' },
  card: { background: '#1a1a1a', border: '1px solid #2a2a2a', borderRadius: 16, padding: 40, width: 400, display: 'flex', flexDirection: 'column', gap: 20 },
  title: { fontSize: 28, fontWeight: 700, color: '#fff', textAlign: 'center' },
  subtitle: { fontSize: 13, color: '#666', textAlign: 'center', marginTop: -12 },
  label: { fontSize: 12, color: '#888', marginBottom: 6, display: 'block' },
  input: { width: '100%', background: '#0f0f0f', border: '1px solid #333', borderRadius: 8, padding: '10px 14px', color: '#fff', fontSize: 14, outline: 'none', boxSizing: 'border-box' },
  btn: { width: '100%', padding: '12px', borderRadius: 8, border: 'none', fontWeight: 600, fontSize: 14, cursor: 'pointer' },
  divider: { display: 'flex', alignItems: 'center', gap: 12, color: '#444', fontSize: 12 },
  line: { flex: 1, height: 1, background: '#2a2a2a' },
  error: { color: '#ff6b6b', fontSize: 13, textAlign: 'center' },
}

export default function JoinBoard({ socket, userId, onReady }) {
  const [name, setName] = useState(localStorage.getItem('rivetdb_display_name') || '')
  const [boardName, setBoardName] = useState('')
  const [code, setCode] = useState('')
  const [mode, setMode] = useState(null) // null | 'join-list' | 'join-code'
  const [boards, setBoards] = useState([])
  const [selectedBoard, setSelectedBoard] = useState(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    socket.on('boards_list', (list) => {
      setBoards(list)
      setLoading(false)
    })
    socket.on('board_error', (msg) => {
      setError(msg)
      setLoading(false)
    })
    return () => {
      socket.off('boards_list')
      socket.off('board_error')
    }
  }, [socket])

  function saveName(v) {
    setName(v)
    localStorage.setItem('rivetdb_display_name', v)
  }

  function validate() {
    if (!name.trim()) { setError('Please enter a display name'); return false }
    setError('')
    return true
  }

  function create() {
    if (!validate()) return
    setLoading(true)
    socket.emit('create_board', { user_id: userId, display_name: name.trim(), board_name: boardName.trim() || 'Untitled' })
    socket.once('board_ready', (data) => { setLoading(false); onReady(data) })
  }

  function openJoinList() {
    if (!validate()) return
    setLoading(true)
    setMode('join-list')
    socket.emit('list_boards')
  }

  function selectBoard(board) {
    setSelectedBoard(board)
    setCode('')
    setError('')
    setMode('join-code')
  }

  function join() {
    if (!code.trim()) { setError('Please enter the board code'); return }
    setLoading(true)
    socket.emit('join_board', { board_code: code.trim(), user_id: userId, display_name: name.trim() })
    socket.once('board_ready', (data) => { setLoading(false); onReady(data) })
  }

  function back() {
    setMode(null)
    setSelectedBoard(null)
    setError('')
    setCode('')
  }

  return (
    <div style={s.page}>
      <div style={s.card}>
        <div>
          <div style={s.title}>RivetDB Whiteboard</div>
          <div style={s.subtitle}>Powered by RivetDB — real-time, persistent</div>
        </div>

        <div>
          <label style={s.label}>Your Display Name</label>
          <input style={s.input} placeholder="Enter your name" value={name} onChange={e => saveName(e.target.value)} maxLength={32} />
        </div>

        {/* CREATE FLOW */}
        {mode === null && (
          <>
            <div>
              <label style={s.label}>Board Name <span style={{ color: '#555' }}>(optional)</span></label>
              <input style={s.input} placeholder="e.g. Project Brainstorm" value={boardName}
                onChange={e => setBoardName(e.target.value)} maxLength={48}
                onKeyDown={e => e.key === 'Enter' && create()} />
            </div>
            <button style={{ ...s.btn, background: '#4ECDC4', color: '#000' }} onClick={create} disabled={loading}>
              {loading ? 'Creating...' : '+ Create New Board'}
            </button>
            <div style={s.divider}><div style={s.line} /> or <div style={s.line} /></div>
            <button style={{ ...s.btn, background: '#2a2a2a', color: '#fff', border: '1px solid #333' }}
              onClick={openJoinList}>
              Join Existing Board
            </button>
          </>
        )}

        {/* JOIN LIST */}
        {mode === 'join-list' && (
          <>
            <div style={{ fontSize: 13, color: '#888', textAlign: 'center' }}>
              {loading ? 'Loading boards...' : boards.length === 0 ? 'No active boards right now.' : 'Select a board to join'}
            </div>
            {!loading && boards.length > 0 && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8, maxHeight: 280, overflowY: 'auto' }}>
                {boards.map(b => (
                  <button key={b.board_code} onClick={() => selectBoard(b)}
                    style={{ background: '#0f0f0f', border: '1px solid #333', borderRadius: 10, padding: '12px 16px', cursor: 'pointer', textAlign: 'left', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <span style={{ color: '#fff', fontWeight: 600, fontSize: 14 }}>{b.name}</span>
                    <span style={{ color: '#4ECDC4', fontSize: 12 }}>● {b.online} online</span>
                  </button>
                ))}
              </div>
            )}
            <button style={{ ...s.btn, background: 'transparent', color: '#666', border: 'none' }} onClick={back}>← Back</button>
          </>
        )}

        {/* JOIN CODE */}
        {mode === 'join-code' && (
          <>
            <div style={{ background: '#0f0f0f', border: '1px solid #2a2a2a', borderRadius: 10, padding: '12px 16px' }}>
              <div style={{ color: '#fff', fontWeight: 600 }}>{selectedBoard?.name}</div>
              <div style={{ color: '#555', fontSize: 12, marginTop: 2 }}>{selectedBoard?.online} people online</div>
            </div>
            <div>
              <label style={s.label}>Enter Board Code</label>
              <input style={s.input} placeholder="e.g. A1B2C3" value={code}
                onChange={e => setCode(e.target.value.toUpperCase())} maxLength={8}
                onKeyDown={e => e.key === 'Enter' && join()} autoFocus />
            </div>
            {error && <div style={s.error}>{error}</div>}
            <button style={{ ...s.btn, background: '#4ECDC4', color: '#000' }} onClick={join} disabled={loading}>
              {loading ? 'Joining...' : 'Join Board'}
            </button>
            <button style={{ ...s.btn, background: 'transparent', color: '#666', border: 'none' }}
              onClick={() => { setMode('join-list'); setSelectedBoard(null); setError('') }}>
              ← Back to List
            </button>
          </>
        )}

        {error && mode === null && <div style={s.error}>{error}</div>}
      </div>
    </div>
  )
}
