use rivetdb::commands::ParsedCommand;
use rivetdb::RespReply;

use super::common;

#[test]
fn test_set_and_get() {
    let state = common::create_test_state();
    
    // SET
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["mykey".into(), "myvalue".into()],
    };
    let reply = rivetdb::commands::process_command(set_cmd, &state);
    assert!(matches!(reply, RespReply::Simple(s) if s == "OK"));
    
    // GET
    let get_cmd = ParsedCommand {
        name: "GET".into(),
        args: vec!["mykey".into()],
    };
    let reply = rivetdb::commands::process_command(get_cmd, &state);
    assert!(matches!(reply, RespReply::Bulk(Some(s)) if s == "myvalue"));
}

#[test]
fn test_get_nonexistent_key() {
    let state = common::create_test_state();
    
    let get_cmd = ParsedCommand {
        name: "GET".into(),
        args: vec!["nonexistent".into()],
    };
    let reply = rivetdb::commands::process_command(get_cmd, &state);
    
    assert!(matches!(reply, RespReply::Bulk(None)));
}

#[test]
fn test_set_overwrites_existing() {
    let state = common::create_test_state();
    
    // First SET
    let set_cmd1 = ParsedCommand {
        name: "SET".into(),
        args: vec!["key".into(), "value1".into()],
    };
    rivetdb::commands::process_command(set_cmd1, &state);
    
    // Second SET (overwrite)
    let set_cmd2 = ParsedCommand {
        name: "SET".into(),
        args: vec!["key".into(), "value2".into()],
    };
    rivetdb::commands::process_command(set_cmd2, &state);
    
    // Verify new value
    assert_eq!(common::get_string_value(&state, "key"), Some("value2".into()));
}

#[test]
fn test_incr_new_key() {
    let state = common::create_test_state();
    
    let incr_cmd = ParsedCommand {
        name: "INCR".into(),
        args: vec!["counter".into()],
    };
    let reply = rivetdb::commands::process_command(incr_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(1)));
}

#[test]
fn test_incr_existing_key() {
    let state = common::create_test_state();
    
    // Set initial value
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["counter".into(), "5".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Increment
    let incr_cmd = ParsedCommand {
        name: "INCR".into(),
        args: vec!["counter".into()],
    };
    let reply = rivetdb::commands::process_command(incr_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(6)));
}

#[test]
fn test_incr_multiple_times() {
    let state = common::create_test_state();
    
    for i in 1..=5 {
        let incr_cmd = ParsedCommand {
            name: "INCR".into(),
            args: vec!["counter".into()],
        };
        let reply = rivetdb::commands::process_command(incr_cmd, &state);
        assert!(matches!(reply, RespReply::Integer(n) if n == i));
    }
}

#[test]
fn test_incr_non_integer_value() {
    let state = common::create_test_state();
    
    // Set non-integer value
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["key".into(), "notanumber".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Try to increment
    let incr_cmd = ParsedCommand {
        name: "INCR".into(),
        args: vec!["key".into()],
    };
    let reply = rivetdb::commands::process_command(incr_cmd, &state);
    
    assert!(matches!(reply, RespReply::Error(_)));
}

#[test]
fn test_decr_new_key() {
    let state = common::create_test_state();
    
    let decr_cmd = ParsedCommand {
        name: "DECR".into(),
        args: vec!["counter".into()],
    };
    let reply = rivetdb::commands::process_command(decr_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(-1)));
}

#[test]
fn test_decr_existing_key() {
    let state = common::create_test_state();
    
    // Set initial value
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["counter".into(), "10".into()],
    };
    rivetdb::commands::process_command(set_cmd, &state);
    
    // Decrement
    let decr_cmd = ParsedCommand {
        name: "DECR".into(),
        args: vec!["counter".into()],
    };
    let reply = rivetdb::commands::process_command(decr_cmd, &state);
    
    assert!(matches!(reply, RespReply::Integer(9)));
}

#[test]
fn test_incr_decr_combination() {
    let state = common::create_test_state();
    
    // INCR 3 times
    for _ in 0..3 {
        let incr_cmd = ParsedCommand {
            name: "INCR".into(),
            args: vec!["counter".into()],
        };
        rivetdb::commands::process_command(incr_cmd, &state);
    }
    
    // DECR 2 times
    for _ in 0..2 {
        let decr_cmd = ParsedCommand {
            name: "DECR".into(),
            args: vec!["counter".into()],
        };
        rivetdb::commands::process_command(decr_cmd, &state);
    }
    
    // Should be 1
    assert_eq!(common::get_string_value(&state, "counter"), Some("1".into()));
}

#[test]
fn test_get_wrong_type() {
    let state = common::create_test_state();
    
    // Create a list
    let lpush_cmd = ParsedCommand {
        name: "LPUSH".into(),
        args: vec!["mylist".into(), "value".into()],
    };
    rivetdb::commands::process_command(lpush_cmd, &state);
    
    // Try to GET it (wrong type)
    let get_cmd = ParsedCommand {
        name: "GET".into(),
        args: vec!["mylist".into()],
    };
    let reply = rivetdb::commands::process_command(get_cmd, &state);
    
    assert!(matches!(reply, RespReply::Error(e) if e.contains("WRONGTYPE")));
}
