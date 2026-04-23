use std::collections::{HashMap, HashSet, LinkedList};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

type ExpiryHeap = BinaryHeap<Reverse<(Instant, String)>>;

use std::collections::VecDeque;

struct SlowLogEntry {
    command: String,
    duration_ns: u128,
    timestamp: Instant,
}

#[derive(Debug, PartialEq)]
enum EvictionPolicy {
    NoEviction,
    AllKeysLFU,
    AllKeysLRU,
}

struct ServerState {
    db: HashMap<String, ValueObject>,
    expiries: ExpiryHeap,
    expired_count: u64,

    // 🔍 Observability
    command_count: HashMap<String, u64>,
    command_time_ns: HashMap<String, u128>,
    slowlog: VecDeque<SlowLogEntry>,
    key_access_count: HashMap<String, u64>,

    max_memory: usize, // bytes
    eviction_policy: EvictionPolicy,
}

type SharedState = Arc<Mutex<ServerState>>;

pub enum ValueObject {
    String(String),
    List(LinkedList<String>),
    Set(HashSet<String>),
}

// type Db = Arc<Mutex<HashMap<String, ValueObject>>>;

/// RESP frame representation (subset of RESP)
// For now we only really use Array + Bulk for commands.
#[derive(Debug)]
enum RespFrame {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(Option<Vec<u8>>),         // None = Null bulk string
    Array(Option<Vec<RespFrame>>), // None = Null array
}

/// Parsed command: what the executer will see
struct ParsedCommand {
    name: String,
    args: Vec<String>,
}

enum RespReply {
    Simple(String),        // +OK
    Error(String),         // -ERR msg
    Integer(i64),          // :1
    Bulk(Option<String>),  // $-1 or bulk
    Array(Vec<RespReply>), // *N ...
}

impl EvictionPolicy {
    fn as_str(&self) -> &'static str {
        match self {
            EvictionPolicy::NoEviction => "noeviction",
            EvictionPolicy::AllKeysLFU => "allkeys-lfu",
            EvictionPolicy::AllKeysLRU => "allkeys-lru",
        }
    }
}

impl RespReply {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            RespReply::Simple(s) => format!("+{}\r\n", s).into_bytes(),

            RespReply::Error(e) => format!("-ERR {}\r\n", e).into_bytes(),

            RespReply::Integer(i) => format!(":{}\r\n", i).into_bytes(),

            RespReply::Bulk(None) => b"$-1\r\n".to_vec(),

            RespReply::Bulk(Some(s)) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),

            RespReply::Array(items) => {
                let mut out = format!("*{}\r\n", items.len()).into_bytes();
                for item in items {
                    out.extend(item.to_bytes());
                }
                out
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").expect("failed to bind to address");
    println!("server listening on 127.0.0.1:7878");

    let state = Arc::new(Mutex::new(ServerState {
        db: HashMap::new(),
        expiries: BinaryHeap::new(),
        expired_count: 0,

        command_count: HashMap::new(),
        command_time_ns: HashMap::new(),
        slowlog: VecDeque::with_capacity(128),
        key_access_count: HashMap::new(),

        max_memory: 64 * 1024 * 1024, // 64 MB default
        eviction_policy: EvictionPolicy::AllKeysLFU,
    }));

    start_expiry_thread(Arc::clone(&state));

    // let db: Db = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("new connection: {}", stream.peer_addr().unwrap());

                let state_clone = Arc::clone(&state);
                thread::spawn(move || {
                    handle_connection(stream, state_clone);
                });
            }
            Err(e) => {
                eprintln!("failed to establish a connection: {}", e);
            }
        }
    }
}

/// Read a single CRLF-terminated line (without the trailing CRLF)
fn read_crlf_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "EOF while reading line",
        ));
    }

    // Strip trailing \r\n or \n
    if line.ends_with("\r\n") {
        line.truncate(line.len() - 2);
    } else if line.ends_with('\n') {
        line.truncate(line.len() - 1);
    }

    Ok(line)
}

fn resp_parse_err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Parse a single RESP frame from the stream
fn parse_frame(reader: &mut impl BufRead) -> io::Result<RespFrame> {
    let mut prefix = [0u8; 1];
    reader.read_exact(&mut prefix)?;

    match prefix[0] {
        b'+' => {
            // Simple String
            let line = read_crlf_line(reader)?;
            Ok(RespFrame::Simple(line))
        }
        b'-' => {
            // Error
            let line = read_crlf_line(reader)?;
            Ok(RespFrame::Error(line))
        }
        b':' => {
            // Integer
            let line = read_crlf_line(reader)?;
            let val: i64 = line
                .parse()
                .map_err(|_| resp_parse_err("invalid integer"))?;
            Ok(RespFrame::Integer(val))
        }
        b'$' => {
            // Bulk string
            let line = read_crlf_line(reader)?;
            let len: isize = line
                .parse()
                .map_err(|_| resp_parse_err("invalid bulk string length"))?;

            if len == -1 {
                return Ok(RespFrame::Bulk(None)); // Null bulk string
            }

            if len < 0 {
                return Err(resp_parse_err("negative bulk string length"));
            }

            let len = len as usize;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;

            // Read and discard trailing CRLF
            let mut crlf = [0u8; 2];
            reader.read_exact(&mut crlf)?;
            if &crlf != b"\r\n" {
                return Err(resp_parse_err("bulk string missing CRLF"));
            }

            Ok(RespFrame::Bulk(Some(buf)))
        }
        b'*' => {
            // Array
            let line = read_crlf_line(reader)?;
            let len: isize = line
                .parse()
                .map_err(|_| resp_parse_err("invalid array length"))?;

            if len == -1 {
                return Ok(RespFrame::Array(None)); // Null array
            }

            if len < 0 {
                return Err(resp_parse_err("negative array length"));
            }

            let len = len as usize;
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                let frame = parse_frame(reader)?;
                items.push(frame);
            }
            Ok(RespFrame::Array(Some(items)))
        }
        other => Err(resp_parse_err(&format!(
            "unknown RESP prefix: {}",
            other as char
        ))),
    }
}

/// Convert a Bulk or Simple frame into a UTF-8 String
fn frame_to_string(frame: RespFrame) -> Result<String, String> {
    match frame {
        RespFrame::Bulk(Some(bytes)) => {
            String::from_utf8(bytes).map_err(|_| "ERR invalid UTF-8 in bulk string".to_string())
        }
        RespFrame::Simple(s) => Ok(s),
        _ => Err("ERR expected bulk or simple string".to_string()),
    }
}

/// Convert a RESP frame (Array of bulk strings) into ParsedCommand
fn frame_to_command(frame: RespFrame) -> Result<ParsedCommand, String> {
    match frame {
        RespFrame::Array(Some(elems)) if !elems.is_empty() => {
            let mut iter = elems.into_iter();

            // First element: command name
            let name_frame = iter.next().unwrap();
            let name = frame_to_string(name_frame)?.to_uppercase();

            // Rest: args
            let mut args = Vec::new();
            for f in iter {
                args.push(frame_to_string(f)?);
            }

            Ok(ParsedCommand { name, args })
        }
        _ => Err("ERR expected array of bulk strings as command".to_string()),
    }
}

fn current_memory_usage(state: &ServerState) -> usize {
    state.db.values().map(estimate_value_size).sum()
}

fn evict_if_needed(state: &mut ServerState, protected: Option<&str>) {
    if state.eviction_policy == EvictionPolicy::NoEviction {
        return;
    }

    let mut used = current_memory_usage(state);

    while used > state.max_memory && !state.db.is_empty() {
        let victim = match state.eviction_policy {
            EvictionPolicy::AllKeysLFU | EvictionPolicy::AllKeysLRU => state
                .key_access_count
                .iter()
                .filter(|(k, _)| Some(k.as_str()) != protected)
                .min_by_key(|(_, count)| *count)
                .map(|(k, _)| k.clone()),

            EvictionPolicy::NoEviction => None,
        };

        let Some(key) = victim else { break };

        if let Some(val) = state.db.remove(&key) {
            used -= estimate_value_size(&val);
        }

        state.key_access_count.remove(&key);
    }
}

fn handle_connection(stream: TcpStream, state: SharedState) {
    let mut reader = BufReader::new(stream.try_clone().expect("failed to clone stream"));
    let mut response_stream = stream;

    loop {
        // 1. Parse a RESP frame
        let frame = match parse_frame(&mut reader) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                println!("client disconnected.");
                return;
            }
            Err(e) => {
                eprintln!("Error reading RESP frame: {}", e);
                let reply = RespReply::Error("protocol error".into());
                let _ = response_stream.write_all(&reply.to_bytes());
                return;
            }
        };

        // 2. Convert frame -> ParsedCommand
        let cmd = match frame_to_command(frame) {
            Ok(c) => c,
            Err(msg) => {
                let reply = RespReply::Error(msg);
                let _ = response_stream.write_all(&reply.to_bytes());
                continue;
            }
        };

        println!("Received command: {} {:?}", cmd.name, cmd.args);

        // 3. Execute command (IMPORTANT FIX HERE)
        // 3. Execute command with timing (NO LOCK HELD HERE)
        // Take command name BEFORE moving cmd
        let cmd_name = cmd.name.clone();

        // Execute command (cmd is moved here)
        let start = Instant::now();
        let reply = std::panic::catch_unwind(|| process_command(cmd, &state));
        let duration = start.elapsed().as_nanos();

        let reply = match reply {
            Ok(r) => r,
            Err(_) => RespReply::Error("internal error".into()),
        };

        // Update observability metrics
        {
            let mut guard = state.lock().unwrap();

            *guard.command_count.entry(cmd_name.clone()).or_insert(0) += 1;
            *guard.command_time_ns.entry(cmd_name.clone()).or_insert(0) += duration;

            // Slow log (threshold: 1 ms)
            if duration > 1_000_000 {
                if guard.slowlog.len() == 128 {
                    guard.slowlog.pop_front();
                }
                guard.slowlog.push_back(SlowLogEntry {
                    command: cmd_name,
                    duration_ns: duration,
                    timestamp: Instant::now(),
                });
            }
        }

        // 5. Send response
        let bytes = reply.to_bytes();
        if response_stream.write_all(&bytes).is_err() {
            eprintln!("Failed to write to client.");
            return;
        }
    }
}
fn start_expiry_thread(state: SharedState) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(100));

            let mut guard = state.lock().unwrap();
            let now = Instant::now();

            loop {
                // Step 1: copy the next expiry (ends immutable borrow immediately)
                let next = match guard.expiries.peek() {
                    Some(Reverse((t, key))) => (*t, key.clone()),
                    None => break,
                };

                // Step 2: stop if not yet expired
                if next.0 > now {
                    break;
                }

                // Step 3: now we can mutate safely
                guard.expiries.pop();
                if guard.db.remove(&next.1).is_some() {
                    guard.key_access_count.remove(&next.1);
                    guard.expired_count += 1;
                }
            }
        }
    });
}

fn is_expired(state: &mut ServerState, key: &str) -> bool {
    let now = Instant::now();
    let expired = state
        .expiries
        .iter()
        .any(|Reverse((t, k))| k == key && *t <= now);

    if expired {
        state.db.remove(key);
        state.key_access_count.remove(key);
        state.expired_count += 1;
    }

    expired
}

fn estimate_value_size(v: &ValueObject) -> usize {
    match v {
        ValueObject::String(s) => s.len(),
        ValueObject::List(l) => l.iter().map(|s| s.len()).sum(),
        ValueObject::Set(s) => s.iter().map(|s| s.len()).sum(),
    }
}

fn process_command(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    match cmd.name.as_str() {
        "PING" => {
            if cmd.args.is_empty() {
                RespReply::Simple("PONG".into())
            } else {
                RespReply::Simple(cmd.args[0].clone())
            }
        }

        "SET" => {
            if cmd.args.len() < 2 {
                return RespReply::Error("SET requires key and value".into());
            }

            let key = &cmd.args[0];
            let value = &cmd.args[1];

            let mut guard = state.lock().unwrap();

            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            // then modify db
            guard
                .db
                .insert(key.clone(), ValueObject::String(value.clone()));

            evict_if_needed(&mut guard, Some(key));

            RespReply::Simple("OK".into())
        }

        "GET" => {
            if cmd.args.len() < 1 {
                return RespReply::Error("GET requires key".into());
            }

            let key = &cmd.args[0];
            let mut guard = state.lock().unwrap();

            if is_expired(&mut guard, key) {
                return RespReply::Bulk(None);
            }

            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            match guard.db.get(key) {
                Some(ValueObject::String(v)) => RespReply::Bulk(Some(v.clone())),
                Some(_) => {
                    RespReply::Error("WRONGTYPE Operation against wrong kind of value".into())
                }
                None => RespReply::Bulk(None),
            }
        }

        "DEL" => {
            if cmd.args.is_empty() {
                return RespReply::Error("DEL requires at least one key".into());
            }

            let mut guard = state.lock().unwrap();
            let mut removed = 0;

            for key in &cmd.args {
                if guard.db.remove(key).is_some() {
                    guard.key_access_count.remove(key); // 🔥 ADD THIS
                    removed += 1;
                }
            }

            RespReply::Integer(removed)
        }

        "EXISTS" => {
            let mut guard = state.lock().unwrap();
            let mut count = 0;

            for key in &cmd.args {
                if !is_expired(&mut guard, key) && guard.db.contains_key(key) {
                    *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;
                    count += 1;
                }
            }

            RespReply::Integer(count)
        }

        "INCR" => {
            if cmd.args.len() != 1 {
                return RespReply::Error("INCR requires one key".into());
            }

            let key = &cmd.args[0];
            let mut guard = state.lock().unwrap();

            if is_expired(&mut guard, key) {
                guard
                    .db
                    .insert(key.clone(), ValueObject::String("0".into()));
            }

            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            let value = guard
                .db
                .entry(key.clone())
                .or_insert_with(|| ValueObject::String("0".into()));

            match value {
                ValueObject::String(s) => {
                    let num = match s.parse::<i64>() {
                        Ok(n) => n,
                        Err(_) => return RespReply::Error("value is not an integer".into()),
                    };

                    let new_val = num + 1;
                    *s = new_val.to_string();
                    RespReply::Integer(new_val)
                }
                _ => RespReply::Error("WRONGTYPE".into()),
            }
        }

        "DECR" => {
            if cmd.args.len() != 1 {
                return RespReply::Error("DECR requires one key".into());
            }

            let key = &cmd.args[0];
            let mut guard = state.lock().unwrap();

            if is_expired(&mut guard, key) {
                guard
                    .db
                    .insert(key.clone(), ValueObject::String("0".into()));
            }

            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            let value = guard
                .db
                .entry(key.clone())
                .or_insert_with(|| ValueObject::String("0".into()));

            match value {
                ValueObject::String(s) => {
                    let num = match s.parse::<i64>() {
                        Ok(n) => n,
                        Err(_) => return RespReply::Error("value is not an integer".into()),
                    };

                    let new_val = num - 1;
                    *s = new_val.to_string();
                    RespReply::Integer(new_val)
                }
                _ => RespReply::Error("WRONGTYPE".into()),
            }
        }

        "LLEN" => {
            if cmd.args.len() != 1 {
                return RespReply::Error("LLEN requires one key".into());
            }

            let mut guard = state.lock().unwrap();

            if is_expired(&mut guard, &cmd.args[0]) {
                return RespReply::Integer(0);
            }

            let key = &cmd.args[0];
            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            match guard.db.get(&cmd.args[0]) {
                Some(ValueObject::List(list)) => RespReply::Integer(list.len() as i64),
                None => RespReply::Integer(0),
                _ => RespReply::Error("WRONGTYPE".into()),
            }
        }

        "LRANGE" => {
            if cmd.args.len() != 3 {
                return RespReply::Error("LRANGE requires key start stop".into());
            }

            let key = &cmd.args[0];
            let start: isize = cmd.args[1].parse().unwrap_or(0);
            let stop: isize = cmd.args[2].parse().unwrap_or(-1);

            let mut guard = state.lock().unwrap();
            if is_expired(&mut guard, key) {
                return RespReply::Array(vec![]);
            }

            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            let list = match guard.db.get(key) {
                Some(ValueObject::List(l)) => l,
                None => return RespReply::Array(vec![]),
                _ => return RespReply::Error("WRONGTYPE".into()),
            };

            let len = list.len() as isize;
            let s = if start < 0 { len + start } else { start }.max(0);
            let e = if stop < 0 { len + stop } else { stop }.min(len - 1);

            let mut result = Vec::new();

            for (i, val) in list.iter().enumerate() {
                let i = i as isize;
                if i >= s && i <= e {
                    result.push(RespReply::Bulk(Some(val.clone())));
                }
            }

            RespReply::Array(result)
        }

        "LPUSH" => {
            if cmd.args.len() < 2 {
                return RespReply::Error("LPUSH requires key and value".into());
            }

            let key = &cmd.args[0];
            let value = &cmd.args[1];

            let mut guard = state.lock().unwrap();

            // count access
            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            // 👇 limit borrow scope
            let new_len = {
                let entry = guard
                    .db
                    .entry(key.clone())
                    .or_insert_with(|| ValueObject::List(LinkedList::new()));

                match entry {
                    ValueObject::List(list) => {
                        list.push_front(value.clone());
                        list.len() as i64
                    }
                    _ => {
                        return RespReply::Error(
                            "WRONGTYPE Operation against wrong kind of value".into(),
                        );
                    }
                }
            }; // <-- entry borrow ENDS HERE

            // now it's safe
            evict_if_needed(&mut guard, Some(key));

            RespReply::Integer(new_len)
        }

        "SADD" => {
            if cmd.args.len() < 2 {
                return RespReply::Error("SADD requires key and members".into());
            }

            let key = &cmd.args[0];
            let members = &cmd.args[1..];

            let mut guard = state.lock().unwrap();

            if is_expired(&mut guard, key) {
                guard.db.remove(key);
            }

            // ✅ count write access
            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            // 👇 limit borrow scope
            let added = {
                let entry = guard
                    .db
                    .entry(key.clone())
                    .or_insert_with(|| ValueObject::Set(HashSet::new()));

                match entry {
                    ValueObject::Set(s) => {
                        let mut added = 0;
                        for m in members {
                            if s.insert(m.clone()) {
                                added += 1;
                            }
                        }
                        added
                    }
                    _ => {
                        return RespReply::Error("WRONGTYPE".into());
                    }
                }
            }; // <-- set borrow ENDS HERE

            // now eviction is safe
            evict_if_needed(&mut guard, Some(key));

            RespReply::Integer(added)
        }

        "SREM" => {
            if cmd.args.len() < 2 {
                return RespReply::Error("SREM requires key and members".into());
            }

            let key = &cmd.args[0];
            let members = &cmd.args[1..];

            let mut guard = state.lock().unwrap();

            if is_expired(&mut guard, key) {
                return RespReply::Integer(0);
            }

            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            match guard.db.get_mut(key) {
                Some(ValueObject::Set(s)) => {
                    let mut removed = 0;
                    for m in members {
                        if s.remove(m) {
                            removed += 1;
                        }
                    }
                    RespReply::Integer(removed)
                }
                None => RespReply::Integer(0),
                _ => RespReply::Error("WRONGTYPE".into()),
            }
        }

        "SMEMBERS" => {
            if cmd.args.len() != 1 {
                return RespReply::Error("SMEMBERS requires one key".into());
            }

            let key = &cmd.args[0];
            let mut guard = state.lock().unwrap();

            if is_expired(&mut guard, key) {
                return RespReply::Array(vec![]);
            }

            *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;

            match guard.db.get(key) {
                Some(ValueObject::Set(s)) => {
                    let members = s.iter().map(|v| RespReply::Bulk(Some(v.clone()))).collect();
                    RespReply::Array(members)
                }
                None => RespReply::Array(vec![]),
                _ => RespReply::Error("WRONGTYPE".into()),
            }
        }

        "EXPIRE" => {
            if cmd.args.len() != 2 {
                return RespReply::Error("EXPIRE requires key and seconds".into());
            }

            let key = &cmd.args[0];
            let seconds: u64 = match cmd.args[1].parse() {
                Ok(s) => s,
                Err(_) => return RespReply::Error("invalid expire time".into()),
            };

            let mut state = state.lock().unwrap();

            if !state.db.contains_key(key) {
                return RespReply::Integer(0);
            }

            let expire_at = Instant::now() + Duration::from_secs(seconds);
            state.expiries.push(Reverse((expire_at, key.clone())));

            RespReply::Integer(1)
        }

        "TTL" => {
            if cmd.args.len() != 1 {
                return RespReply::Error("TTL requires one key".into());
            }

            let key = &cmd.args[0];
            let mut guard = state.lock().unwrap();

            // Key does not exist
            if !guard.db.contains_key(key) {
                return RespReply::Integer(-2);
            }

            let now = Instant::now();
            let mut ttl = None;

            for Reverse((t, k)) in guard.expiries.iter() {
                if k == key {
                    if *t <= now {
                        // expired
                        guard.db.remove(key);
                        guard.expired_count += 1;
                        return RespReply::Integer(-2);
                    }
                    ttl = Some(t.saturating_duration_since(now).as_secs() as i64);
                    break;
                }
            }

            match ttl {
                Some(v) => RespReply::Integer(v),
                None => RespReply::Integer(-1), // exists but no expiry
            }
        }

        "STATS" => {
            let guard = state.lock().unwrap();

            let mut arr = Vec::new();
            arr.push(RespReply::Bulk(Some("expired_keys".into())));
            arr.push(RespReply::Integer(guard.expired_count as i64));

            for (cmd, count) in &guard.command_count {
                let total_time = guard.command_time_ns.get(cmd).unwrap_or(&0);
                let avg = if *count > 0 {
                    total_time / (*count as u128)
                } else {
                    0
                };

                arr.push(RespReply::Bulk(Some(format!("cmd:{}:count", cmd))));
                arr.push(RespReply::Integer(*count as i64));

                arr.push(RespReply::Bulk(Some(format!("cmd:{}:avg_ns", cmd))));
                arr.push(RespReply::Integer(avg as i64));
            }

            RespReply::Array(arr)
        }

        "HOTKEYS" => {
            let guard = state.lock().unwrap();

            let mut keys: Vec<_> = guard.key_access_count.iter().collect();
            keys.sort_by(|a, b| b.1.cmp(a.1));

            let mut result = Vec::new();
            for (k, v) in keys.into_iter().take(5) {
                result.push(RespReply::Bulk(Some(k.clone())));
                result.push(RespReply::Integer(*v as i64));
            }

            RespReply::Array(result)
        }

        "MEMORY" => {
            if cmd.args.len() != 1 {
                return RespReply::Error("MEMORY requires key".into());
            }

            let key = &cmd.args[0];
            let mut guard = state.lock().unwrap();

            // Respect TTL
            if is_expired(&mut guard, key) {
                return RespReply::Integer(0);
            }

            // Count access only if key exists
            if guard.db.contains_key(key) {
                *guard.key_access_count.entry(key.clone()).or_insert(0) += 1;
            }

            match guard.db.get(key) {
                Some(v) => RespReply::Integer(estimate_value_size(v) as i64),
                None => RespReply::Integer(0),
            }
        }

        "SLOWLOG" => {
            let guard = state.lock().unwrap();
            let mut result = Vec::new();

            for entry in guard.slowlog.iter().rev() {
                result.push(RespReply::Bulk(Some(entry.command.clone())));
                result.push(RespReply::Integer(entry.duration_ns as i64));
            }

            RespReply::Array(result)
        }

        "CONFIG" => {
            if cmd.args.len() < 2 {
                return RespReply::Error("CONFIG GET|SET".into());
            }

            match cmd.args[0].as_str() {
                "GET" => {
                    let guard = state.lock().unwrap();
                    match cmd.args[1].as_str() {
                        "maxmemory" => RespReply::Integer(guard.max_memory as i64),
                        "eviction" => RespReply::Bulk(Some(guard.eviction_policy.as_str().into())),

                        _ => RespReply::Bulk(None),
                    }
                }

                "SET" => {
                    let mut guard = state.lock().unwrap();
                    match cmd.args[1].as_str() {
                        "maxmemory" => {
                            guard.max_memory = cmd.args[2].parse().unwrap_or(guard.max_memory);
                            RespReply::Simple("OK".into())
                        }
                        "eviction" => {
                            guard.eviction_policy = match cmd.args[2].as_str() {
                                "lfu" => EvictionPolicy::AllKeysLFU,
                                "lru" => EvictionPolicy::AllKeysLRU,
                                "none" => EvictionPolicy::NoEviction,
                                _ => return RespReply::Error("unknown eviction policy".into()),
                            };
                            RespReply::Simple("OK".into())
                        }
                        _ => RespReply::Error("unknown config".into()),
                    }
                }

                _ => RespReply::Error("CONFIG GET|SET".into()),
            }
        }

        _ => RespReply::Error("unknown command".into()),
    }
}
