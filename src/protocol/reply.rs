/// Reply types that can be sent back to clients
pub enum RespReply {
    Simple(String),        // +OK
    Error(String),         // -ERR msg
    Integer(i64),          // :1
    Bulk(Option<String>),  // $-1 or bulk
    Array(Vec<RespReply>), // *N ...
}

impl RespReply {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            RespReply::Simple(s) => format!("+{}\r\n", s).into_bytes(),

            RespReply::Error(e) => format!("-ERR {}\r\n", e).into_bytes(),

            RespReply::Integer(i) => format!(":{}\r\n", i).into_bytes(),

            RespReply::Bulk(None) => b"$-1\r\n".to_vec(),

            RespReply::Bulk(Some(s)) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),

            RespReply::Array(items) => {
                let mut out = format!("*{}\r\n", items.len()).into_bytes();
                for item in items {
                    out.extend(item.to_bytes());
                }
                out
            }
        }
    }
}
