use clap::Parser;
use std::cmp::Reverse;
use std::io::{self, Cursor};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

// Import from our modules
use rivetdb::commands::process_command;
use rivetdb::commands::auth::{auth, is_auth_exempt};
use rivetdb::commands::json::{
    json_set, json_get, json_mget, json_del, json_type, 
    json_arrappend, json_arrlen, json_objkeys, json_objlen, json_numincrby,
};
use rivetdb::commands::schema::{
    create_schema_registry, schema_cmd, tget, tkeys, tset, tvalidate, SchemaRegistry,
};
use rivetdb::config::SecurityConfig;
use rivetdb::metrics::start_metrics_server;
use rivetdb::persistence::{
    create_async_aof_writer, is_write_command, AofFsyncPolicy,
    AsyncAofWriter, SharedAsyncAofWriter,
};
use rivetdb::protocol::{frame_to_command, parse_frame};
use rivetdb::storage::SlowLogEntry;
use rivetdb::{Config, EvictionPolicy, ParsedCommand, RespReply, ServerState, SharedState};

/// RivetDB - A high-performance in-memory database written in Rust
#[derive(Parser, Debug)]
#[command(name = "rivetdb")]
#[command(version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "rivetdb.toml")]
    config: String,

    /// Server host to bind to
    #[arg(long)]
    host: Option<String>,

    /// Server port to bind to
    #[arg(short, long)]
    port: Option<u16>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long)]
    log_level: Option<String>,

    /// Enable AOF persistence
    #[arg(long)]
    aof: Option<bool>,
}

#[tokio::main]
async fn main() {
    // Parse CLI arguments
    let args = Args::parse();

    // Load configuration
    let mut config = Config::load_or_default(&args.config);

    // Override config with CLI arguments
    if let Some(host) = args.host {
        config.server.host = host;
    }
    if let Some(port) = args.port {
        config.server.port = port;
    }
    if let Some(level) = args.log_level {
        config.logging.level = level;
    }
    if let Some(aof_enabled) = args.aof {
        config.persistence.aof_enabled = aof_enabled;
    }

    // Override security config from environment variables
    if let Ok(password) = std::env::var("RIVETDB_PASSWORD") {
        if !password.is_empty() {
            config.security.password = Some(password);
            config.security.require_auth = true;
        }
    }

    // Initialize tracing/logging
    init_logging(&config.logging.level);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting RivetDB (async mode with type-safe schemas)"
    );
    info!(
        host = %config.server.host,
        port = config.server.port,
        "Server configuration"
    );
    info!(
        max_memory = %config.memory.max_memory,
        eviction_policy = %config.memory.eviction_policy,
        "Memory configuration"
    );
    info!(
        aof_enabled = config.persistence.aof_enabled,
        aof_fsync = %config.persistence.aof_fsync,
        aof_file = %config.persistence.aof_filename,
        "Persistence configuration"
    );

    // Parse eviction policy
    let eviction_policy = match config.memory.eviction_policy.as_str() {
        "allkeys-lfu" => EvictionPolicy::AllKeysLFU,
        "allkeys-lru" => EvictionPolicy::AllKeysLRU,
        "noeviction" => EvictionPolicy::NoEviction,
        _ => {
            warn!(
                policy = %config.memory.eviction_policy,
                "Unknown eviction policy, defaulting to allkeys-lfu"
            );
            EvictionPolicy::AllKeysLFU
        }
    };

    // Initialize server state with DashMap (lock-free concurrent HashMap!)
    let state: SharedState = Arc::new(ServerState::new(
        config.max_memory_bytes(),
        eviction_policy,
    ));

    // Initialize schema registry
    let schema_registry: SchemaRegistry = create_schema_registry();
    info!("Schema registry initialized (type-safe keys enabled)");

    // TODO: Load AOF - needs refactoring for DashMap approach
    // AOF loading would need to be updated to work with the new ServerState
    if config.persistence.aof_enabled {
        info!("AOF persistence enabled (loading not yet available with DashMap)");
    }

    info!(
        max_memory_bytes = state.max_memory,
        keys = state.key_count(),
        mode = "DashMap (lock-free)",
        "Server state initialized"
    );

    // Initialize ASYNC AOF writer if enabled (non-blocking for better throughput!)
    let aof_writer: SharedAsyncAofWriter = if config.persistence.aof_enabled {
        let fsync_policy = AofFsyncPolicy::from(config.persistence.aof_fsync.as_str());
        // Channel size of 10,000 allows high burst throughput
        match create_async_aof_writer(&config.persistence.aof_filename, fsync_policy, 10_000) {
            Ok(writer) => {
                info!(
                    file = %config.persistence.aof_filename,
                    channel_size = 10_000,
                    "Async AOF writer initialized (non-blocking)"
                );
                writer
            }
            Err(e) => {
                error!(error = %e, "Failed to create async AOF writer");
                Arc::new(None)
            }
        }
    } else {
        Arc::new(None)
    };

    // Bind to address using async Tokio
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", bind_addr, e));

    info!(addr = %bind_addr, "Server listening (async)");

    // Start Prometheus metrics server
    let metrics_state = Arc::clone(&state);
    let metrics_port = config.server.metrics_port;
    tokio::spawn(async move {
        start_metrics_server(metrics_state, metrics_port).await;
    });
    info!(port = metrics_port, "Prometheus metrics server started at /metrics");

    // Start background expiry task
    let expiry_state = Arc::clone(&state);
    tokio::spawn(async move {
        run_expiry_loop(expiry_state).await;
    });
    info!("Expiry background task started");

    // Track active connections
    let connection_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    // Clone security config for sharing across connections
    let security_config = Arc::new(config.security.clone());

    // Accept connections (async loop)
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let count =
                    connection_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                info!(client = %addr, connections = count, "New connection");

                let state_clone = Arc::clone(&state);
                let aof_clone = Arc::clone(&aof_writer);
                let schema_clone = Arc::clone(&schema_registry);
                let conn_count = Arc::clone(&connection_count);
                let security_clone = Arc::clone(&security_config);

                // Spawn async task (not OS thread!)
                // This is the key benefit: thousands of concurrent connections with minimal memory
                tokio::spawn(async move {
                    handle_connection(stream, state_clone, aof_clone, schema_clone, security_clone).await;
                    let remaining =
                        conn_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) - 1;
                    debug!(connections = remaining, "Connection closed");
                });
            }
            Err(e) => {
                error!(error = %e, "Failed to accept connection");
            }
        }
    }
}

/// Async connection handler with schema support and authentication
async fn handle_connection(
    mut stream: TcpStream,
    state: SharedState,
    aof: SharedAsyncAofWriter,
    schema: SchemaRegistry,
    security: Arc<SecurityConfig>,
) {
    let peer_addr = stream.peer_addr().ok();

    debug!(client = ?peer_addr, "Connection handler started (async)");

    // Track authentication state for this connection
    // If auth is not required, start as authenticated
    let mut is_authenticated = !security.require_auth;

    let mut buf = vec![0u8; 4096];
    let mut read_buf = Vec::new();

    loop {
        // Read data from socket (async!)
        let n = match stream.read(&mut buf).await {
            Ok(0) => {
                info!(client = ?peer_addr, "Client disconnected");
                return;
            }
            Ok(n) => n,
            Err(e) => {
                if e.kind() != io::ErrorKind::ConnectionReset {
                    error!(client = ?peer_addr, error = %e, "Error reading from socket");
                }
                return;
            }
        };

        // Append to read buffer
        read_buf.extend_from_slice(&buf[..n]);

        // Try to parse frames from buffer
        loop {
            let mut cursor = Cursor::new(&read_buf);

            // Try to parse a frame
            match parse_frame(&mut cursor) {
                Ok(frame) => {
                    let consumed = cursor.position() as usize;

                    // Convert frame -> ParsedCommand
                    let cmd = match frame_to_command(frame) {
                        Ok(c) => c,
                        Err(msg) => {
                            warn!(client = ?peer_addr, error = %msg, "Invalid command");
                            let reply = RespReply::Error(msg);
                            if stream.write_all(&reply.to_bytes()).await.is_err() {
                                return;
                            }
                            read_buf.drain(..consumed);
                            continue;
                        }
                    };

                    debug!(
                        client = ?peer_addr,
                        command = %cmd.name,
                        args = ?cmd.args,
                        "Processing command"
                    );

                    // Check authentication before processing command
                    let cmd_name_upper = cmd.name.to_uppercase();
                    
                    // Handle AUTH command specially
                    if cmd_name_upper == "AUTH" {
                        let reply = auth(&cmd, &security.password);
                        // If AUTH succeeded, mark connection as authenticated
                        if matches!(reply, RespReply::Simple(ref s) if s == "OK") {
                            is_authenticated = true;
                            info!(client = ?peer_addr, "Client authenticated successfully");
                        } else {
                            warn!(client = ?peer_addr, "Authentication failed");
                        }
                        if stream.write_all(&reply.to_bytes()).await.is_err() {
                            return;
                        }
                        read_buf.drain(..consumed);
                        continue;
                    }

                    // Check if command requires authentication
                    if !is_authenticated && !is_auth_exempt(&cmd_name_upper) {
                        let reply = RespReply::Error("NOAUTH Authentication required. Use AUTH <password>".into());
                        if stream.write_all(&reply.to_bytes()).await.is_err() {
                            return;
                        }
                        read_buf.drain(..consumed);
                        continue;
                    }

                    // Execute command with timing
                    let cmd_name = cmd.name.clone();
                    let is_write = is_write_command(&cmd_name);

                    let start = Instant::now();

                    // Route command - check for schema commands first
                    let reply = route_command(cmd.clone(), &state, &schema);

                    let duration = start.elapsed();

                    // Log to AOF if write command succeeded (NON-BLOCKING!)
                    // This is the key optimization - we just send to a channel,
                    // the actual disk write happens in a background task
                    if is_write && !matches!(reply, RespReply::Error(_)) {
                        if let Some(ref writer) = *aof {
                            writer.log_command(&cmd);
                        }
                    }

                    // Log slow commands
                    let duration_ns = duration.as_nanos();
                    if duration_ns > 1_000_000 {
                        warn!(
                            command = %cmd_name,
                            duration_ms = duration.as_millis(),
                            "Slow command detected"
                        );
                    }

                    // Update observability metrics (lock-free with DashMap!)
                    state.inc_command_count(&cmd_name);
                    state.add_command_time(&cmd_name, duration_ns);

                    if duration_ns > 1_000_000 {
                        state.add_slowlog(SlowLogEntry {
                            command: cmd_name,
                            duration_ns,
                            timestamp: Instant::now(),
                        });
                    }

                    // Send response (async!)
                    let bytes = reply.to_bytes();
                    if stream.write_all(&bytes).await.is_err() {
                        error!(client = ?peer_addr, "Failed to write response to client");
                        return;
                    }

                    // Remove consumed bytes from buffer
                    read_buf.drain(..consumed);
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    // Need more data
                    break;
                }
                Err(e) => {
                    error!(client = ?peer_addr, error = %e, "Error parsing RESP frame");
                    let reply = RespReply::Error("protocol error".into());
                    let _ = stream.write_all(&reply.to_bytes()).await;
                    return;
                }
            }
        }
    }
}

/// Route command to appropriate handler
fn route_command(cmd: ParsedCommand, state: &SharedState, schema: &SchemaRegistry) -> RespReply {
    match cmd.name.to_uppercase().as_str() {
        // Schema commands (unique to RivetDB!)
        "SCHEMA" => schema_cmd(cmd, schema),
        "TSET" => tset(cmd, state, schema),
        "TGET" => tget(cmd, state, schema),
        "TVALIDATE" => tvalidate(cmd, schema),
        "TKEYS" => tkeys(cmd, state),

        // JSON commands (native, unlike Redis!)
        "JSON.SET" => json_set(cmd, state),
        "JSON.GET" => json_get(cmd, state),
        "JSON.MGET" => json_mget(cmd, state),
        "JSON.DEL" => json_del(cmd, state),
        "JSON.TYPE" => json_type(cmd, state),
        "JSON.ARRAPPEND" => json_arrappend(cmd, state),
        "JSON.ARRLEN" => json_arrlen(cmd, state),
        "JSON.OBJKEYS" => json_objkeys(cmd, state),
        "JSON.OBJLEN" => json_objlen(cmd, state),
        "JSON.NUMINCRBY" => json_numincrby(cmd, state),

        // All other commands go to standard processor
        _ => process_command(cmd, state),
    }
}

/// Async expiry loop - runs as a background task (DashMap version)
async fn run_expiry_loop(state: SharedState) {
    use std::time::Duration;
    use tokio::time::interval;

    let mut ticker = interval(Duration::from_millis(100));

    loop {
        ticker.tick().await;

        let now = Instant::now();

        // Check and expire keys from the expiry heap
        // Need to lock just the expiry heap (BinaryHeap needs mutex)
        loop {
            let next_key = if let Ok(mut expiries) = state.expiries.lock() {
                match expiries.peek() {
                    Some(Reverse((expires_at, _))) if *expires_at <= now => {
                        let entry = expiries.pop().unwrap();
                        Some(entry.0.1) // Get the key
                    }
                    _ => None,
                }
            } else {
                None
            };

            match next_key {
                Some(key) => {
                    // Remove from DashMap (lock-free!)
                    if state.check_and_expire(&key) {
                        debug!(key = %key, "Key expired");
                    }
                }
                None => break,
            }
        }
    }
}

/// Initialize tracing subscriber with specified log level
fn init_logging(level: &str) {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}
