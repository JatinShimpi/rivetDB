import { useState, useEffect } from 'react'
import { io } from 'socket.io-client'
import JoinBoard from './components/JoinBoard.jsx'
import Whiteboard from './components/Whiteboard.jsx'

function getOrCreateUserId() {
  let id = localStorage.getItem('rivetdb_user_id')
  if (!id) {
    id = 'u_' + Math.random().toString(36).slice(2, 10)
    localStorage.setItem('rivetdb_user_id', id)
  }
  return id
}

const socket = io({ autoConnect: false })

export default function App() {
  const [board, setBoard] = useState(null)
  const userId = getOrCreateUserId()

  useEffect(() => {
    socket.connect()
    return () => socket.disconnect()
  }, [])

  function handleReady(boardData) {
    setBoard(boardData)
  }

  if (!board) {
    return <JoinBoard socket={socket} userId={userId} onReady={handleReady} />
  }

  return <Whiteboard socket={socket} userId={userId} board={board} onLeave={() => setBoard(null)} />
}
