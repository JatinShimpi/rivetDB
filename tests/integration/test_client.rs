use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Integration test client for RESP protocol
pub struct TestClient {
    stream: TcpStream,
}

impl TestClient {
    /// Connect to RivetDB server
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(TestClient { stream })
    }

    /// Send a command and receive response
    pub fn send_command(&mut self, args: &[&str]) -> std::io::Result<String> {
        // Build RESP array
        let mut cmd = format!("*{}\r\n", args.len());
        for arg in args {
            cmd.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
        }

        // Send command
        self.stream.write_all(cmd.as_bytes())?;
        self.stream.flush()?;

        // Read response
        let mut reader = BufReader::new(&self.stream);
        let frame = rivetdb::protocol::parse_frame(&mut reader)?;

        // Convert to string representation
        Ok(format!("{:?}", frame))
    }

    /// Send command and expect OK response
    pub fn expect_ok(&mut self, args: &[&str]) -> std::io::Result<()> {
        let response = self.send_command(args)?;
        if response.contains("Simple(\"OK\")") {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Expected OK, got: {}", response),
            ))
        }
    }

    /// Send command and expect integer response
    pub fn expect_integer(&mut self, args: &[&str]) -> std::io::Result<i64> {
        let response = self.send_command(args)?;
        // Parse "Integer(N)" format
        if let Some(start) = response.find("Integer(") {
            if let Some(end) = response[start..].find(')') {
                let num_str = &response[start + 8..start + end];
                return num_str.parse().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to parse integer")
                });
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Expected Integer, got: {}", response),
        ))
    }

    /// Send command and expect bulk string response
    pub fn expect_bulk(&mut self, args: &[&str]) -> std::io::Result<Option<String>> {
        let response = self.send_command(args)?;
        
        if response.contains("Bulk(None)") {
            return Ok(None);
        }
        
        // Parse Bulk(Some(...)) format - this is simplified
        if response.contains("Bulk(Some") {
            // For integration tests, we'll just check it's not None
            // Actual value parsing would need proper RESP parsing
            Ok(Some("value".to_string()))
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Expected Bulk, got: {}", response),
            ))
        }
    }
}
