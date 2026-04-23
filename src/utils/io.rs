use std::io::{self, BufRead};

/// Read a single CRLF-terminated line (without the trailing CRLF)
pub fn read_crlf_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "EOF while reading line",
        ));
    }

    // Strip trailing \r\n or \n
    if line.ends_with("\r\n") {
        line.truncate(line.len() - 2);
    } else if line.ends_with('\n') {
        line.truncate(line.len() - 1);
    }

    Ok(line)
}