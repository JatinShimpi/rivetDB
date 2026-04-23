//! Authentication command handler
//!
//! Implements Redis-compatible AUTH command for password authentication.
//! When security.require_auth is enabled, clients must authenticate before
//! running any other commands.

use crate::protocol::RespReply;
use crate::commands::ParsedCommand;

/// Handle AUTH command
/// Usage: AUTH password
/// Returns OK if password matches, error otherwise
pub fn auth(cmd: &ParsedCommand, expected_password: &Option<String>) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("ERR wrong number of arguments for 'AUTH' command".into());
    }

    let provided_password = &cmd.args[0];

    match expected_password {
        Some(expected) => {
            if provided_password == expected {
                RespReply::Simple("OK".into())
            } else {
                RespReply::Error("ERR invalid password".into())
            }
        }
        None => {
            // No password configured - AUTH not required but we accept any password
            RespReply::Simple("OK".into())
        }
    }
}

/// Check if a command requires authentication
/// Some commands like AUTH and PING should work without authentication
pub fn is_auth_exempt(cmd_name: &str) -> bool {
    matches!(
        cmd_name.to_uppercase().as_str(),
        "AUTH" | "PING" | "QUIT" | "COMMAND"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_correct_password() {
        let cmd = ParsedCommand {
            name: "AUTH".into(),
            args: vec!["secret123".into()],
        };
        let expected = Some("secret123".into());
        
        match auth(&cmd, &expected) {
            RespReply::Simple(s) => assert_eq!(s, "OK"),
            _ => panic!("Expected OK response"),
        }
    }

    #[test]
    fn test_auth_wrong_password() {
        let cmd = ParsedCommand {
            name: "AUTH".into(),
            args: vec!["wrongpassword".into()],
        };
        let expected = Some("secret123".into());
        
        match auth(&cmd, &expected) {
            RespReply::Error(e) => assert!(e.contains("invalid password")),
            _ => panic!("Expected error response"),
        }
    }

    #[test]
    fn test_auth_no_password_configured() {
        let cmd = ParsedCommand {
            name: "AUTH".into(),
            args: vec!["anypassword".into()],
        };
        let expected = None;
        
        match auth(&cmd, &expected) {
            RespReply::Simple(s) => assert_eq!(s, "OK"),
            _ => panic!("Expected OK response"),
        }
    }

    #[test]
    fn test_is_auth_exempt() {
        assert!(is_auth_exempt("AUTH"));
        assert!(is_auth_exempt("PING"));
        assert!(is_auth_exempt("QUIT"));
        assert!(!is_auth_exempt("SET"));
        assert!(!is_auth_exempt("GET"));
    }
}
