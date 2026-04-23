use rivetdb::commands::ParsedCommand;
use rivetdb::RespReply;

use super::common;

#[test]
fn test_ping_without_args() {
    let cmd = ParsedCommand {
        name: "PING".into(),
        args: vec![],
    };
    
    let state = common::create_test_state();
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Simple(s) if s == "PONG"));
}

#[test]
fn test_ping_with_message() {
    let cmd = ParsedCommand {
        name: "PING".into(),
        args: vec!["hello".into()],
    };
    
    let state = common::create_test_state();
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Simple(s) if s == "hello"));
}

#[test]
fn test_del_single_key() {
    let state = common::create_test_state();
    
    // Set a key first
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["key1".into(), "value1".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Delete it
    let del_cmd = ParsedCommand {
        name: "DEL".into(),
        args: vec!["key1".into()],
    };
    let reply = rivetdb::commands::process_command(del_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)));
    assert!(!common::key_exists(&state, "key1"));
}

#[test]
fn test_del_multiple_keys() {
    let state = common::create_test_state();
    
    // Set multiple keys
    for i in 1..=3 {
        let cmd = ParsedCommand {
            name: "SET".into(),
            args: vec![format!("key{}", i), format!("value{}", i)],
        };
        rivetdb::commands::process_command(cmd, &state);
    }
    
    // Delete them
    let del_cmd = ParsedCommand {
        name: "DEL".into(),
        args: vec!["key1".into(), "key2".into(), "key3".into()],
    };
    let reply = rivetdb::commands::process_command(del_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(3)));
    assert_eq!(common::db_size(&state), 0);
}

#[test]
fn test_del_nonexistent_key() {
    let state = common::create_test_state();
    
    let del_cmd = ParsedCommand {
        name: "DEL".into(),
        args: vec!["nonexistent".into()],
    };
    let reply = rivetdb::commands::process_command(del_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(0)));
}

#[test]
fn test_exists_single_key() {
    let state = common::create_test_state();
    
    // Set a key
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["key1".into(), "value1".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Check exists
    let exists_cmd = ParsedCommand {
        name: "EXISTS".into(),
        args: vec!["key1".into()],
    };
    let reply = rivetdb::commands::process_command(exists_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)));
}

#[test]
fn test_exists_multiple_keys() {
    let state = common::create_test_state();
    
    // Set 2 out of 3 keys
    let set_cmd1 = ParsedCommand {
        name: "SET".into(),
        args: vec!["key1".into(), "value1".into()],
    };
    let set_cmd2 = ParsedCommand {
        name: "SET".into(),
        args: vec!["key2".into(), "value2".into()],
    };
    rivetdb::commands::process_command(set_cmd1, &state);
    rivetdb::commands::process_command(set_cmd2, &state);
    
    // Check all 3
    let exists_cmd = ParsedCommand {
        name: "EXISTS".into(),
        args: vec!["key1".into(), "key2".into(), "key3".into()],
    };
    let reply = rivetdb::commands::process_command(exists_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(2)));
}

#[test]
fn test_exists_nonexistent() {
    let state = common::create_test_state();
    
    let exists_cmd = ParsedCommand {
        name: "EXISTS".into(),
        args: vec!["nonexistent".into()],
    };
    let reply = rivetdb::commands::process_command(exists_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(0)));
}
