mod basic;
mod string;
mod list;
mod set;
pub mod expiry;
mod admin;
mod generic;
mod zset;
mod hash;
pub mod schema;
pub mod json;
pub mod bloom;
pub mod timeseries;
pub mod query;
pub mod namespace;
pub mod auth;

use crate::storage::SharedState;
use crate::protocol::RespReply;

/// Parsed command: what the executor will see
#[derive(Clone)]
pub struct ParsedCommand {
    pub name: String,
    pub args: Vec<String>,
}

pub fn process_command(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    match cmd.name.as_str() {
        // Basic commands
        "PING" => basic::ping(cmd),
        "DEL" => basic::del(cmd, state),
        "EXISTS" => basic::exists(cmd, state),

        // String commands
        "SET" => string::set(cmd, state),
        "GET" => string::get(cmd, state),
        "INCR" => string::incr(cmd, state),
        "DECR" => string::decr(cmd, state),
        "MGET" => string::mget(cmd, state),
        "MSET" => string::mset(cmd, state),
        "APPEND" => string::append(cmd, state),
        "STRLEN" => string::strlen(cmd, state),
        "GETRANGE" => string::getrange(cmd, state),
        "SETRANGE" => string::setrange(cmd, state),

        // List commands
        "LPUSH" => list::lpush(cmd, state),
        "RPUSH" => list::rpush(cmd, state),
        "LPOP" => list::lpop(cmd, state),
        "RPOP" => list::rpop(cmd, state),
        "LLEN" => list::llen(cmd, state),
        "LRANGE" => list::lrange(cmd, state),
        "LINDEX" => list::lindex(cmd, state),
        "LSET" => list::lset(cmd, state),
        "LTRIM" => list::ltrim(cmd, state),

        // Set commands
        "SADD" => set::sadd(cmd, state),
        "SREM" => set::srem(cmd, state),
        "SMEMBERS" => set::smembers(cmd, state),
        "SISMEMBER" => set::sismember(cmd, state),
        "SCARD" => set::scard(cmd, state),
        "SUNION" => set::sunion(cmd, state),
        "SINTER" => set::sinter(cmd, state),
        "SDIFF" => set::sdiff(cmd, state),

        // Sorted Set commands
        "ZADD" => zset::zadd(cmd, state),
        "ZRANGE" => zset::zrange(cmd, state),
        "ZREVRANGE" => zset::zrevrange(cmd, state),
        "ZRANGEBYSCORE" => zset::zrangebyscore(cmd, state),
        "ZREM" => zset::zrem(cmd, state),
        "ZREMRANGEBYRANK" => zset::zremrangebyrank(cmd, state),
        "ZREMRANGEBYSCORE" => zset::zremrangebyscore(cmd, state),
        "ZSCORE" => zset::zscore(cmd, state),
        "ZRANK" => zset::zrank(cmd, state),
        "ZCARD" => zset::zcard(cmd, state),
        "ZCOUNT" => zset::zcount(cmd, state),
        "ZINCRBY" => zset::zincrby(cmd, state),

        // Hash commands
        "HSET" => hash::hset(cmd, state),
        "HSETNX" => hash::hsetnx(cmd, state),
        "HMSET" => hash::hmset(cmd, state),
        "HGET" => hash::hget(cmd, state),
        "HMGET" => hash::hmget(cmd, state),
        "HGETALL" => hash::hgetall(cmd, state),
        "HDEL" => hash::hdel(cmd, state),
        "HEXISTS" => hash::hexists(cmd, state),
        "HLEN" => hash::hlen(cmd, state),
        "HINCRBY" => hash::hincrby(cmd, state),
        "HINCRBYFLOAT" => hash::hincrbyfloat(cmd, state),
        "HKEYS" => hash::hkeys(cmd, state),
        "HVALS" => hash::hvals(cmd, state),
        "HSCAN" => hash::hscan(cmd, state),

        // Expiry commands
        "EXPIRE" => expiry::expire(cmd, state),
        "TTL" => expiry::ttl(cmd, state),

        // Admin commands
        "STATS" => admin::stats(state),
        "HOTKEYS" => admin::hotkeys(state),
        "MEMORY" => admin::memory(cmd, state),
        "SLOWLOG" => admin::slowlog(state),
        "CONFIG" => admin::config(cmd, state),

        // Generic commands
        "TYPE" => generic::type_cmd(cmd, state),
        "RENAME" => generic::rename(cmd, state),
        "RENAMENX" => generic::renamenx(cmd, state),
        "KEYS" => generic::keys(cmd, state),
        "SCAN" => generic::scan(cmd, state),

        // Bloom Filter commands (built-in, not a module!)
        "BF.RESERVE" => bloom::bf_reserve(cmd, state),
        "BF.ADD" => bloom::bf_add(cmd, state),
        "BF.MADD" => bloom::bf_madd(cmd, state),
        "BF.EXISTS" => bloom::bf_exists(cmd, state),
        "BF.MEXISTS" => bloom::bf_mexists(cmd, state),
        "BF.INFO" => bloom::bf_info(cmd, state),
        "BF.CARD" => bloom::bf_card(cmd, state),

        // Time-Series commands (built-in, not a module!)
        "TS.CREATE" => timeseries::ts_create(cmd, state),
        "TS.ADD" => timeseries::ts_add(cmd, state),
        "TS.MADD" => timeseries::ts_madd(cmd, state),
        "TS.GET" => timeseries::ts_get(cmd, state),
        "TS.RANGE" => timeseries::ts_range(cmd, state),
        "TS.MRANGE" => timeseries::ts_mrange(cmd, state),
        "TS.INFO" => timeseries::ts_info(cmd, state),
        "TS.DEL" => timeseries::ts_del(cmd, state),
        "TS.ALTER" => timeseries::ts_alter(cmd, state),
        "TS.INCRBY" => timeseries::ts_incrby(cmd, state),
        "TS.DECRBY" => timeseries::ts_decrby(cmd, state),

        // SQL Query commands (UNIQUE - Redis doesn't have this!)
        "QUERY" => query::query(cmd, state),
        "EXPLAIN" => query::explain(cmd, state),

        _ => RespReply::Error("unknown command".into()),
    }
}
