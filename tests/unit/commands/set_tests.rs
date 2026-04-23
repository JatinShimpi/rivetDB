use rivetdb::commands::ParsedCommand;
use rivetdb::RespReply;

use super::common;

#[test]
fn test_sadd_single_member() {
    let state = common::create_test_state();
    
    let cmd = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member1".into()],
    };
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)));
}

#[test]
fn test_sadd_multiple_members() {
    let state = common::create_test_state();
    
    let cmd = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member1".into(), "member2".into(), "member3".into()],
    };
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(3)));
}

#[test]
fn test_sadd_duplicate_member() {
    let state = common::create_test_state();
    
    // Add first time
    let cmd1 = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member1".into()],
    };
    let reply1 = rivetdb::commands::process_command(cmd1, &state);
    assert!(matches!(reply1, RespReply::Integer(1)));
    
    // Add same member again
    let cmd2 = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member1".into()],
    };
    let reply2 = rivetdb::commands::process_command(cmd2, &state);
    assert!(matches!(reply2, RespReply::Integer(0)), "Duplicate should return 0");
}

#[test]
fn test_srem_existing_member() {
    let state = common::create_test_state();
    
    // Add members
    let sadd_cmd = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member1".into(), "member2".into()],
    };
    rivetdb::commands::process_command(sadd_cmd, &state);
    
    // Remove one
    let srem_cmd = ParsedCommand {
        name: "SREM".into(),
        args: vec!["myset".into(), "member1".into()],
    };
    let reply = rivetdb::commands::process_command(srem_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)));
}

#[test]
fn test_srem_nonexistent_member() {
    let state = common::create_test_state();
    
    // Add a member
    let sadd_cmd = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member1".into()],
    };
    rivetdb::commands::process_command(sadd_cmd, &state);
    
    // Try to remove non-existent member
    let srem_cmd = ParsedCommand {
        name: "SREM".into(),
        args: vec!["myset".into(), "nonexistent".into()],
    };
    let reply = rivetdb::commands::process_command(srem_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(0)));
}

#[test]
fn test_srem_multiple_members() {
    let state = common::create_test_state();
    
    // Add members
    let sadd_cmd = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "m1".into(), "m2".into(), "m3".into(), "m4".into()],
    };
    rivetdb::commands::process_command(sadd_cmd, &state);
    
    // Remove multiple
    let srem_cmd = ParsedCommand {
        name: "SREM".into(),
        args: vec!["myset".into(), "m1".into(), "m3".into()],
    };
    let reply = rivetdb::commands::process_command(srem_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(2)));
}

#[test]
fn test_smembers_empty_set() {
    let state = common::create_test_state();
    
    let cmd = ParsedCommand {
        name: "SMEMBERS".into(),
        args: vec!["nonexistent".into()],
    };
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Array(items) if items.is_empty()));
}

#[test]
fn test_smembers_with_elements() {
    let state = common::create_test_state();
    
    // Add members
    let sadd_cmd = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "a".into(), "b".into(), "c".into()],
    };
    rivetdb::commands::process_command(sadd_cmd, &state);
    
    // Get all members
    let smembers_cmd = ParsedCommand {
        name: "SMEMBERS".into(),
        args: vec!["myset".into()],
    };
    let reply = rivetdb::commands::process_command(smembers_cmd, &state);
    
    if let RespReply::Array(items) = reply {
        assert_eq!(items.len(), 3);
        // Note: Set order is not guaranteed, so we just check count
    } else {
        panic!("Expected Array reply");
    }
}

#[test]
fn test_sadd_after_srem() {
    let state = common::create_test_state();
    
    // Add, remove, add again
    let sadd1 = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member".into()],
    };
    rivetdb::commands::process_command(sadd1, &state);
    
    let srem = ParsedCommand {
        name: "SREM".into(),
        args: vec!["myset".into(), "member".into()],
    };
    rivetdb::commands::process_command(srem, &state);
    
    let sadd2 = ParsedCommand {
        name: "SADD".into(),
        args: vec!["myset".into(), "member".into()],
    };
    let reply = rivetdb::commands::process_command(sadd2, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)), "Should be able to re-add after removal");
}

#[test]
fn test_sadd_wrong_type() {
    let state = common::create_test_state();
    
    // Create a string key
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Try to SADD to it
    let sadd_cmd = ParsedCommand {
        name: "SADD".into(),
        args: vec!["mykey".into(), "member".into()],
    };
    let reply = rivetdb::commands::process_command(sadd_cmd, &state);
    
    assert!(matches!(reply, RespReply::Error(e) if e.contains("WRONGTYPE")));
}

#[test]
fn test_smembers_wrong_type() {
    let state = common::create_test_state();
    
    // Create a string key
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Try to SMEMBERS
    let smembers_cmd = ParsedCommand {
        name: "SMEMBERS".into(),
        args: vec!["mykey".into()],
    };
    let reply = rivetdb::commands::process_command(smembers_cmd, &state);
    
    assert!(matches!(reply, RespReply::Error(e) if e.contains("WRONGTYPE")));
}
