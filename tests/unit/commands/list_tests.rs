use rivetdb::commands::ParsedCommand;
use rivetdb::RespReply;

use super::common;

#[test]
fn test_lpush_single_element() {
    let state = common::create_test_state();
    
    let cmd = ParsedCommand {
        name: "LPUSH".into(),
        args: vec!["mylist".into(), "value1".into()],
    };
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)));
}

#[test]
fn test_lpush_multiple_times() {
    let state = common::create_test_state();
    
    for i in 1..=5 {
        let cmd = ParsedCommand {
            name: "LPUSH".into(),
            args: vec!["mylist".into(), format!("value{}", i)],
        };
        let reply = rivetdb::commands::process_command(cmd, &state);
        assert!(matches!(reply, RespReply::Integer(n) if n == i));
    }
}

#[test]
fn test_llen_empty_list() {
    let state = common::create_test_state();
    
    let cmd = ParsedCommand {
        name: "LLEN".into(),
        args: vec!["nonexistent".into()],
    };
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(0)));
}

#[test]
fn test_llen_after_lpush() {
    let state = common::create_test_state();
    
    // Push 3 elements
    for i in 1..=3 {
        let cmd = ParsedCommand {
            name: "LPUSH".into(),
            args: vec!["mylist".into(), format!("value{}", i)],
        };
        rivetdb::commands::process_command(cmd, &state);
    }
    
    // Check length
    let llen_cmd = ParsedCommand {
        name: "LLEN".into(),
        args: vec!["mylist".into()],
    };
    let reply = rivetdb::commands::process_command(llen_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(3)));
}

#[test]
fn test_lrange_full_list() {
    let state = common::create_test_state();
    
    // Push elements (they'll be in reverse order due to LPUSH)
    for i in 1..=3 {
        let cmd = ParsedCommand {
            name: "LPUSH".into(),
            args: vec!["mylist".into(), format!("value{}", i)],
        };
        rivetdb::commands::process_command(cmd, &state);
    }
    
    // Get all elements
    let lrange_cmd = ParsedCommand {
        name: "LRANGE".into(),
        args: vec!["mylist".into(), "0".into(), "-1".into()],
    };
    let reply = rivetdb::commands::process_command(lrange_cmd, &state);
    
    // Should return value3, value2, value1 (LPUSH adds to front)
    if let RespReply::Array(items) = reply {
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], RespReply::Bulk(Some(s)) if s == "value3"));
        assert!(matches!(&items[1], RespReply::Bulk(Some(s)) if s == "value2"));
        assert!(matches!(&items[2], RespReply::Bulk(Some(s)) if s == "value1"));
    } else {
        panic!("Expected Array reply");
    }
}

#[test]
fn test_lrange_partial() {
    let state = common::create_test_state();
    
    // Push 5 elements
    for i in 1..=5 {
        let cmd = ParsedCommand {
            name: "LPUSH".into(),
            args: vec!["mylist".into(), format!("value{}", i)],
        };
        rivetdb::commands::process_command(cmd, &state);
    }
    
    // Get elements 1-3
    let lrange_cmd = ParsedCommand {
        name: "LRANGE".into(),
        args: vec!["mylist".into(), "1".into(), "3".into()],
    };
    let reply = rivetdb::commands::process_command(lrange_cmd, &state);
    
    if let RespReply::Array(items) = reply {
        assert_eq!(items.len(), 3);
    } else {
        panic!("Expected Array reply");
    }
}

#[test]
fn test_lrange_empty_list() {
    let state = common::create_test_state();
    
    let cmd = ParsedCommand {
        name: "LRANGE".into(),
        args: vec!["nonexistent".into(), "0".into(), "-1".into()],
    };
    let reply = rivetdb::commands::process_command(cmd, &state);
    
    assert!(matches!(reply, RespReply::Array(items) if items.is_empty()));
}

#[test]
fn test_lrange_negative_indices() {
    let state = common::create_test_state();
    
    // Push 5 elements
    for i in 1..=5 {
        let cmd = ParsedCommand {
            name: "LPUSH".into(),
            args: vec!["mylist".into(), format!("value{}", i)],
        };
        rivetdb::commands::process_command(cmd, &state);
    }
    
    // Get last 2 elements
    let lrange_cmd = ParsedCommand {
        name: "LRANGE".into(),
        args: vec!["mylist".into(), "-2".into(), "-1".into()],
    };
    let reply = rivetdb::commands::process_command(lrange_cmd, &state);
    
    if let RespReply::Array(items) = reply {
        assert_eq!(items.len(), 2);
    } else {
        panic!("Expected Array reply");
    }
}

#[test]
fn test_lpush_wrong_type() {
    let state = common::create_test_state();
    
    // Create a string key
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Try to LPUSH to it
    let lpush_cmd = ParsedCommand {
        name: "LPUSH".into(),
        args: vec!["mykey".into(), "element".into()],
    };
    let reply = rivetdb::commands::process_command(lpush_cmd, &state);
    
    assert!(matches!(reply, RespReply::Error(e) if e.contains("WRONGTYPE")));
}

#[test]
fn test_llen_wrong_type() {
    let state = common::create_test_state();
    
    // Create a string key
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "value".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Try to get LLEN
    let llen_cmd = ParsedCommand {
        name: "LLEN".into(),
        args: vec!["mykey".into()],
    };
    let reply = rivetdb::commands::process_command(llen_cmd, &state);
    
    assert!(matches!(reply, RespReply::Error(e) if e.contains("WRONGTYPE")));
}
