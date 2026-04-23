use rivetdb::commands::ParsedCommand;
use rivetdb::RespReply;
use std::thread;
use std::time::Duration;

use super::common;

#[test]
fn test_expire_and_ttl() {
    let state = common::create_test_state();
    
    // Set a key
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Set expiry to 2 seconds
    let expire_cmd = ParsedCommand {
        name: "EXPIRE".into(),
        args: vec!["mykey".into(), "2".into()],
    };
    let reply = rivetdb::commands::process_command(expire_cmd, &state);
    assert!(matches!(reply, RespReply::Integer(1)));
    
    // Check TTL
    let ttl_cmd = ParsedCommand {
        name: "TTL".into(),
        args: vec!["mykey".into()],
    };
    let reply = rivetdb::commands::process_command(ttl_cmd, &state);
    
    // TTL should be around 2 seconds (1 or 2 due to timing)
    if let RespReply::Integer(ttl) = reply {
        assert!(ttl >= 1 && ttl <= 2, "TTL should be 1 or 2, got {}", ttl);
    } else {
        panic!("Expected Integer reply for TTL");
    }
}

#[test]
fn test_ttl_nonexistent_key() {
    let state = common::create_test_state();
    
    let ttl_cmd = ParsedCommand {
        name: "TTL".into(),
        args: vec!["nonexistent".into()],
    };
    let reply = rivetdb::commands::process_command(ttl_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(-2)), "TTL of nonexistent key should be -2");
}

#[test]
fn test_ttl_no_expiry() {
    let state = common::create_test_state();
    
    // Set a key without expiry
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Check TTL
    let ttl_cmd = ParsedCommand {
        name: "TTL".into(),
        args: vec!["mykey".into()],
    };
    let reply = rivetdb::commands::process_command(ttl_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(-1)), "TTL of key without expiry should be -1");
}

#[test]
fn test_expire_nonexistent_key() {
    let state = common::create_test_state();
    
    let expire_cmd = ParsedCommand {
        name: "EXPIRE".into(),
        args: vec!["nonexistent".into(), "10".into()],
    };
    let reply = rivetdb::commands::process_command(expire_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(0)), "EXPIRE on nonexistent key should return 0");
}

#[test]
fn test_key_expires_automatically() {
    let state = common::create_test_state();
    
    // Set a key
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["tempkey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Set very short expiry (1 second)
    let expire_cmd = ParsedCommand {
        name: "EXPIRE".into(),
        args: vec!["tempkey".into(), "1".into()],
    };
    rivetdb::commands::process_command(expire_cmd, &state);
    
    // Key should exist immediately
    assert!(common::key_exists(&state, "tempkey"));
    
    // Wait for expiry
    thread::sleep(Duration::from_millis(1200));
    
    // Try to GET - should trigger expiry check
    let get_cmd = ParsedCommand {
        name: "GET".into(),
        args: vec!["tempkey".into()],
    };
    let reply = rivetdb::commands::process_command(get_cmd, &state);
    
    // Should return None (key expired)
    assert!(matches!(reply, RespReply::Bulk(None)));
}

#[test]
fn test_expire_updates_existing_ttl() {
    let state = common::create_test_state();
    
    // Set a key with expiry
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    let expire_cmd1 = ParsedCommand {
        name: "EXPIRE".into(),
        args: vec!["mykey".into(), "10".into()],
    };
    rivetdb::commands::process_command(expire_cmd1, &state);
    
    // Update expiry
    let expire_cmd2 = ParsedCommand {
        name: "EXPIRE".into(),
        args: vec!["mykey".into(), "100".into()],
    };
    let reply = rivetdb::commands::process_command(expire_cmd2, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)));
}
