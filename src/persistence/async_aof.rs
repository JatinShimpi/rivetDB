//! Async AOF Writer using Tokio mpsc channel
//!
//! This provides non-blocking AOF logging by using a channel to decouple
//! command execution from disk writes. Commands are sent to a background
//! task that handles the actual I/O.
//!
//! This dramatically improves pipelined write performance by eliminating
//! the synchronous disk write from the critical path.

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tracing::{debug, error, info};

use crate::commands::ParsedCommand;
use super::AofFsyncPolicy;

/// Message types for the AOF channel
pub enum AofMessage {
    /// Log a command (RESP-formatted string)
    Write(String),
    /// Force fsync
    Flush,
    /// Shutdown the writer
    Shutdown,
}

/// Async AOF writer that sends commands through a channel
/// 
/// This is the "hot path" struct that connection handlers interact with.
/// It only sends messages to a channel - no I/O blocking!
#[derive(Clone)]
pub struct AsyncAofWriter {
    sender: Sender<AofMessage>,
}

impl AsyncAofWriter {
    /// Create a new async AOF writer
    /// 
    /// Spawns a background Tokio task that consumes from the channel
    /// and writes to disk asynchronously.
    pub fn new(
        path: &str, 
        fsync_policy: AofFsyncPolicy,
        buffer_size: usize,
    ) -> io::Result<Self> {
        // Create channel with bounded capacity for backpressure
        let (tx, rx) = mpsc::channel(buffer_size);
        
        // Open file for writing
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        
        // Use 64KB buffer for efficient disk writes
        let writer = BufWriter::with_capacity(64 * 1024, file);
        
        let path_owned = path.to_string();
        
        // Spawn background writer task
        tokio::spawn(async move {
            Self::writer_loop(rx, writer, fsync_policy, path_owned).await;
        });
        
        info!(path = %path, buffer_size = buffer_size, "Async AOF writer initialized");
        
        Ok(AsyncAofWriter { sender: tx })
    }
    
    /// Log a command asynchronously (non-blocking!)
    /// 
    /// This is the key optimization - we just send to a channel,
    /// no disk I/O happens in the calling task.
    pub fn log_command(&self, cmd: &ParsedCommand) {
        let resp = Self::format_as_resp(cmd);
        
        // Use try_send for non-blocking send
        // If channel is full, we drop the message (lose durability for speed)
        // For production, you might want to handle backpressure differently
        if let Err(e) = self.sender.try_send(AofMessage::Write(resp)) {
            debug!(error = %e, "AOF channel full, command not logged");
        }
    }
    
    /// Force a sync (for testing or graceful shutdown)
    pub async fn flush(&self) {
        let _ = self.sender.send(AofMessage::Flush).await;
    }
    
    /// Shutdown the writer gracefully
    pub async fn shutdown(&self) {
        let _ = self.sender.send(AofMessage::Shutdown).await;
    }
    
    /// Format command as RESP array
    fn format_as_resp(cmd: &ParsedCommand) -> String {
        let total_elements = 1 + cmd.args.len();
        let mut resp = format!("*{}\r\n", total_elements);
        
        // Command name
        resp.push_str(&format!("${}\r\n{}\r\n", cmd.name.len(), cmd.name));
        
        // Arguments
        for arg in &cmd.args {
            resp.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
        }
        
        resp
    }
    
    /// Background writer loop - runs in a separate Tokio task
    async fn writer_loop(
        mut rx: Receiver<AofMessage>,
        mut writer: BufWriter<File>,
        fsync_policy: AofFsyncPolicy,
        path: String,
    ) {
        let mut pending_writes = 0u64;
        let mut total_writes = 0u64;
        
        // For everysec policy, spawn a ticker to force periodic fsync
        let fsync_interval = if fsync_policy == AofFsyncPolicy::Everysec {
            Some(tokio::time::interval(std::time::Duration::from_secs(1)))
        } else {
            None
        };
        
        // Pin the interval so we can use it in select!
        tokio::pin!(fsync_interval);
        
        loop {
            tokio::select! {
                // Handle incoming messages
                msg = rx.recv() => {
                    match msg {
                        Some(AofMessage::Write(data)) => {
                            if let Err(e) = writer.write_all(data.as_bytes()) {
                                error!(error = %e, "Failed to write to AOF");
                            } else {
                                pending_writes += 1;
                                total_writes += 1;
                                
                                // Batch flush based on policy
                                match fsync_policy {
                                    AofFsyncPolicy::Always => {
                                        let _ = writer.flush();
                                        if let Err(e) = writer.get_ref().sync_all() {
                                            error!(error = %e, "AOF fsync failed");
                                        }
                                        pending_writes = 0;
                                    }
                                    AofFsyncPolicy::Everysec | AofFsyncPolicy::No => {
                                        // Flush buffer every 1000 writes for efficiency
                                        if pending_writes >= 1000 {
                                            let _ = writer.flush();
                                            pending_writes = 0;
                                        }
                                    }
                                }
                            }
                        }
                        Some(AofMessage::Flush) => {
                            let _ = writer.flush();
                            if let Err(e) = writer.get_ref().sync_all() {
                                error!(error = %e, "AOF fsync failed");
                            }
                            pending_writes = 0;
                            debug!(total_writes = total_writes, "AOF flushed");
                        }
                        Some(AofMessage::Shutdown) => {
                            let _ = writer.flush();
                            let _ = writer.get_ref().sync_all();
                            info!(path = %path, total_writes = total_writes, "AOF writer shutdown");
                            break;
                        }
                        None => {
                            // Channel closed
                            let _ = writer.flush();
                            let _ = writer.get_ref().sync_all();
                            info!(path = %path, "AOF channel closed");
                            break;
                        }
                    }
                }
                
                // Periodic fsync for everysec policy
                _ = async {
                    if let Some(ref mut interval) = *fsync_interval {
                        interval.tick().await
                    } else {
                        // Never resolves if no interval
                        std::future::pending::<tokio::time::Instant>().await
                    }
                } => {
                    if pending_writes > 0 {
                        let _ = writer.flush();
                        if let Err(e) = writer.get_ref().sync_all() {
                            error!(error = %e, "Periodic AOF fsync failed");
                        }
                        debug!(pending = pending_writes, "Periodic AOF fsync");
                        pending_writes = 0;
                    }
                }
            }
        }
    }
}

/// Shared async AOF writer type
pub type SharedAsyncAofWriter = Arc<Option<AsyncAofWriter>>;

/// Create a new shared async AOF writer
pub fn create_async_aof_writer(
    path: &str,
    fsync_policy: AofFsyncPolicy,
    channel_size: usize,
) -> io::Result<SharedAsyncAofWriter> {
    let writer = AsyncAofWriter::new(path, fsync_policy, channel_size)?;
    Ok(Arc::new(Some(writer)))
}
