use super::expiry::is_expired;
use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::evict_if_needed;
use crate::storage::{SharedState, ValueObject, ZSet};

/// ZADD key [NX|XX] [GT|LT] [CH] score member [score member ...] - Add members with scores
pub fn zadd(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("ZADD requires key and score-member pairs".into());
    }

    let key = &cmd.args[0];
    
    // Parse options
    let mut nx = false;
    let mut xx = false;
    let mut gt = false;
    let mut lt = false;
    let mut ch = false;
    let mut args_start = 1;

    while args_start < cmd.args.len() {
        match cmd.args[args_start].to_uppercase().as_str() {
            "NX" => { nx = true; args_start += 1; }
            "XX" => { xx = true; args_start += 1; }
            "GT" => { gt = true; args_start += 1; }
            "LT" => { lt = true; args_start += 1; }
            "CH" => { ch = true; args_start += 1; }
            _ => break,
        }
    }

    if nx && xx { return RespReply::Error("NX and XX are mutually exclusive".into()); }
    if gt && lt { return RespReply::Error("GT and LT are mutually exclusive".into()); }

    let pairs = &cmd.args[args_start..];
    if pairs.is_empty() || pairs.len() % 2 != 0 {
        return RespReply::Error("ZADD requires score-member pairs".into());
    }

    state.track_key_access(key);

    let added_or_changed = {
        let mut entry = state.db.entry(key.clone())
            .or_insert_with(|| ValueObject::ZSet(ZSet::new()));

        match entry.value_mut() {
            ValueObject::ZSet(zset) => {
                let mut count = 0;
                for chunk in pairs.chunks(2) {
                    let score: f64 = match chunk[0].parse() {
                        Ok(s) => s,
                        Err(_) => return RespReply::Error("score is not a valid float".into()),
                    };
                    let member = &chunk[1];

                    if nx || xx || gt || lt {
                        let (changed, _) = zset.add_with_options(member.clone(), score, nx, xx, gt, lt);
                        if changed { count += 1; }
                    } else {
                        let was_added = zset.add(member.clone(), score);
                        if ch || was_added { count += 1; }
                    }
                }
                count
            }
            _ => return RespReply::Error("WRONGTYPE".into()),
        }
    };

    evict_if_needed(state, Some(key));
    RespReply::Integer(added_or_changed)
}

/// ZRANGE key start stop [WITHSCORES] - Get members by rank range
pub fn zrange(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("ZRANGE requires key, start, and stop".into());
    }

    let key = &cmd.args[0];
    let start: isize = cmd.args[1].parse().unwrap_or(0);
    let stop: isize = cmd.args[2].parse().unwrap_or(-1);
    let with_scores = cmd.args.get(3).map(|s| s.to_uppercase() == "WITHSCORES").unwrap_or(false);

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::ZSet(zset) => {
                let range = zset.range_by_rank(start, stop);
                let mut result = Vec::new();
                for (member, score) in range {
                    result.push(RespReply::Bulk(Some(member)));
                    if with_scores {
                        result.push(RespReply::Bulk(Some(score.to_string())));
                    }
                }
                RespReply::Array(result)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// ZREVRANGE key start stop [WITHSCORES] - Get members by rank range (reversed)
pub fn zrevrange(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("ZREVRANGE requires key, start, and stop".into());
    }

    let key = &cmd.args[0];
    let start: isize = cmd.args[1].parse().unwrap_or(0);
    let stop: isize = cmd.args[2].parse().unwrap_or(-1);
    let with_scores = cmd.args.get(3).map(|s| s.to_uppercase() == "WITHSCORES").unwrap_or(false);

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::ZSet(zset) => {
                let mut range = zset.range_by_rank(start, stop);
                range.reverse();
                let mut result = Vec::new();
                for (member, score) in range {
                    result.push(RespReply::Bulk(Some(member)));
                    if with_scores {
                        result.push(RespReply::Bulk(Some(score.to_string())));
                    }
                }
                RespReply::Array(result)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// ZSCORE key member - Get score of member
pub fn zscore(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("ZSCORE requires key and member".into());
    }

    let key = &cmd.args[0];
    let member = &cmd.args[1];

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::ZSet(zset) => {
                match zset.score(member) {
                    Some(score) => RespReply::Bulk(Some(score.to_string())),
                    None => RespReply::Bulk(None),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// ZCARD key - Get member count
pub fn zcard(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("ZCARD requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::ZSet(zset) => RespReply::Integer(zset.len() as i64),
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// ZRANK key member - Get rank of member
pub fn zrank(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("ZRANK requires key and member".into());
    }

    let key = &cmd.args[0];
    let member = &cmd.args[1];

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::ZSet(zset) => {
                match zset.rank(member) {
                    Some(rank) => RespReply::Integer(rank as i64),
                    None => RespReply::Bulk(None),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// ZREM key member [member ...] - Remove members
pub fn zrem(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("ZREM requires key and member(s)".into());
    }

    let key = &cmd.args[0];
    let members = &cmd.args[1..];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::ZSet(zset) => {
                let mut removed = 0;
                for member in members {
                    if zset.remove(member) {
                        removed += 1;
                    }
                }
                RespReply::Integer(removed)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// ZINCRBY key increment member - Increment member's score
pub fn zincrby(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("ZINCRBY requires key, increment, and member".into());
    }

    let key = &cmd.args[0];
    let increment: f64 = match cmd.args[1].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("increment is not a valid float".into()),
    };
    let member = &cmd.args[2];

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::ZSet(ZSet::new()));

    match entry.value_mut() {
        ValueObject::ZSet(zset) => {
            let current = zset.score(member).unwrap_or(0.0);
            let new_score = current + increment;
            zset.add(member.clone(), new_score);
            RespReply::Bulk(Some(new_score.to_string()))
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// ZCOUNT key min max - Count members in score range
pub fn zcount(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("ZCOUNT requires key, min, and max".into());
    }

    let key = &cmd.args[0];
    let min: f64 = parse_score_bound(&cmd.args[1]).unwrap_or(f64::NEG_INFINITY);
    let max: f64 = parse_score_bound(&cmd.args[2]).unwrap_or(f64::INFINITY);

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::ZSet(zset) => {
                RespReply::Integer(zset.count(min, max) as i64)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

fn parse_score_bound(s: &str) -> Option<f64> {
    if s == "-inf" {
        Some(f64::NEG_INFINITY)
    } else if s == "+inf" || s == "inf" {
        Some(f64::INFINITY)
    } else if s.starts_with('(') {
        s[1..].parse().ok()
    } else {
        s.parse().ok()
    }
}

/// ZRANGEBYSCORE key min max [WITHSCORES] [LIMIT offset count]
pub fn zrangebyscore(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("ZRANGEBYSCORE requires key, min, and max".into());
    }

    let key = &cmd.args[0];
    let min = parse_score_bound(&cmd.args[1]).unwrap_or(f64::NEG_INFINITY);
    let max = parse_score_bound(&cmd.args[2]).unwrap_or(f64::INFINITY);
    
    let with_scores = cmd.args.iter().any(|s| s.to_uppercase() == "WITHSCORES");

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::ZSet(zset) => {
                let range = zset.range_by_score(min, max, None);
                let mut result = Vec::new();
                for (member, score) in range {
                    result.push(RespReply::Bulk(Some(member)));
                    if with_scores {
                        result.push(RespReply::Bulk(Some(score.to_string())));
                    }
                }
                RespReply::Array(result)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

fn parse_limit(args: &[String]) -> Option<(usize, usize)> {
    for i in 0..args.len() {
        if args[i].to_uppercase() == "LIMIT" && i + 2 < args.len() {
            let offset: usize = args[i + 1].parse().ok()?;
            let count: usize = args[i + 2].parse().ok()?;
            return Some((offset, count));
        }
    }
    None
}

/// ZREMRANGEBYRANK key start stop
pub fn zremrangebyrank(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("ZREMRANGEBYRANK requires key, start, and stop".into());
    }

    let key = &cmd.args[0];
    let start: isize = cmd.args[1].parse().unwrap_or(0);
    let stop: isize = cmd.args[2].parse().unwrap_or(-1);

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::ZSet(zset) => {
                RespReply::Integer(zset.remove_range_by_rank(start, stop) as i64)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// ZREMRANGEBYSCORE key min max
pub fn zremrangebyscore(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("ZREMRANGEBYSCORE requires key, min, and max".into());
    }

    let key = &cmd.args[0];
    let min = parse_score_bound(&cmd.args[1]).unwrap_or(f64::NEG_INFINITY);
    let max = parse_score_bound(&cmd.args[2]).unwrap_or(f64::INFINITY);

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::ZSet(zset) => {
                RespReply::Integer(zset.remove_range_by_score(min, max) as i64)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}
