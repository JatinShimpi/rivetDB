use crate::protocol::RespReply;
use crate::storage::SharedState;
use super::ParsedCommand;
use super::expiry::is_expired;

/// PING [message] - Test connection
pub fn ping(cmd: ParsedCommand) -> RespReply {
    if cmd.args.is_empty() {
        RespReply::Simple("PONG".into())
    } else {
        RespReply::Simple(cmd.args[0].clone())
    }
}

/// DEL key [key ...] - Delete keys
pub fn del(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("DEL requires at least one key".into());
    }

    let mut removed = 0;

    for key in &cmd.args {
        if state.db.remove(key).is_some() {
            state.key_access_count.remove(key);
            removed += 1;
        }
    }

    RespReply::Integer(removed)
}

/// EXISTS key [key ...] - Check if keys exist
pub fn exists(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    let mut count = 0;

    for key in &cmd.args {
        if !is_expired(state, key) && state.db.contains_key(key) {
            state.track_key_access(key);
            count += 1;
        }
    }

    RespReply::Integer(count)
}