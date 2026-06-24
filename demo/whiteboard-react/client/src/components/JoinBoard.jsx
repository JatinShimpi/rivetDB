import { useState } from 'react'

const s = {
  page: { minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#0f0f0f' },
  card: { background: '#1a1a1a', border: '1px solid #2a2a2a', borderRadius: 16, padding: 40, width: 380, display: 'flex', flexDirection: 'column', gap: 24 },
  title: { fontSize: 28, fontWeight: 700, color: '#fff', textAlign: 'center' },
  subtitle: { fontSize: 13, color: '#666', textAlign: 'center', marginTop: -16 },
  label: { fontSize: 12, color: '#888', marginBottom: 6, display: 'block' },
  input: { width: '100%', background: '#0f0f0f', border: '1px solid #333', borderRadius: 8, padding: '10px 14px', color: '#fff', fontSize: 14, outline: 'none' },
  btn: { width: '100%', padding: '12px', borderRadius: 8, border: 'none', fontWeight: 600, fontSize: 14, cursor: 'pointer' },
  divider: { display: 'flex', alignItems: 'center', gap: 12, color: '#444', fontSize: 12 },
  line: { flex: 1, height: 1, background: '#2a2a2a' },
  error: { color: '#ff6b6b', fontSize: 13, textAlign: 'center' },
}

export default function JoinBoard({ socket, userId, onReady }) {
  const [name, setName] = useState(localStorage.getItem('rivetdb_display_name') || '')
  const [code, setCode] = useState('')
  const [mode, setMode] = useState(null) // null | 'join'
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

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
    socket.emit('create_board', { user_id: userId, display_name: name.trim() })
    socket.once('board_ready', (data) => { setLoading(false); onReady(data) })
  }

  function join() {
    if (!validate()) return
    if (!code.trim()) { setError('Please enter a board code'); return }
    setLoading(true)
    socket.emit('join_board', { board_code: code.trim(), user_id: userId, display_name: name.trim() })
    socket.once('board_ready', (data) => { setLoading(false); onReady(data) })
  }

  return (
    <div style={s.page}>
      <div style={s.card}>
        <div>
          <div style={s.title}>RivetDB Whiteboard</div>
          <div style={s.subtitle}>Powered by RivetDB — real-time, persistent</div>
        </div>

        <div>
          <label style={s.label}>Display Name</label>
          <input style={s.input} placeholder="Enter your name" value={name} onChange={e => saveName(e.target.value)} maxLength={32} />
        </div>

        {mode === 'join' && (
          <div>
            <label style={s.label}>Board Code</label>
            <input style={s.input} placeholder="e.g. A1B2C3" value={code}
              onChange={e => setCode(e.target.value.toUpperCase())} maxLength={8}
              onKeyDown={e => e.key === 'Enter' && join()} autoFocus />
          </div>
        )}

        {error && <div style={s.error}>{error}</div>}

        {mode === null ? (
          <>
            <button style={{ ...s.btn, background: '#4ECDC4', color: '#000' }} onClick={create} disabled={loading}>
              {loading ? 'Creating...' : '+ Create New Board'}
            </button>
            <div style={s.divider}><div style={s.line} /> or <div style={s.line} /></div>
            <button style={{ ...s.btn, background: '#2a2a2a', color: '#fff', border: '1px solid #333' }}
              onClick={() => setMode('join')}>
              Join Existing Board
            </button>
          </>
        ) : (
          <>
            <button style={{ ...s.btn, background: '#4ECDC4', color: '#000' }} onClick={join} disabled={loading}>
              {loading ? 'Joining...' : 'Join Board'}
            </button>
            <button style={{ ...s.btn, background: 'transparent', color: '#666', border: 'none' }}
              onClick={() => { setMode(null); setError('') }}>
              ← Back
            </button>
          </>
        )}
      </div>
    </div>
  )
}
