use std::io::{self, BufRead};
use crate::utils::io::read_crlf_line;
use crate::commands::ParsedCommand;

/// RESP frame representation (subset of RESP)
#[derive(Debug)]
pub enum RespFrame {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(Option<Vec<u8>>),         // None = Null bulk string
    Array(Option<Vec<RespFrame>>), // None = Null array
}

fn resp_parse_err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Parse a single RESP frame from the stream
pub fn parse_frame(reader: &mut impl BufRead) -> io::Result<RespFrame> {
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
pub fn frame_to_string(frame: RespFrame) -> Result<String, String> {
    match frame {
        RespFrame::Bulk(Some(bytes)) => {
            String::from_utf8(bytes).map_err(|_| "ERR invalid UTF-8 in bulk string".to_string())
        }
        RespFrame::Simple(s) => Ok(s),
        _ => Err("ERR expected bulk or simple string".to_string()),
    }
}

/// Convert a RESP frame (Array of bulk strings) into ParsedCommand
pub fn frame_to_command(frame: RespFrame) -> Result<ParsedCommand, String> {
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