# RivetDB Testing Guide

## Overview

This document describes the testing infrastructure for RivetDB.

## Test Structure

```
tests/
├── common/
│   └── mod.rs          # Shared test utilities
├── unit/
│   ├── mod.rs          # Unit test module declarations
│   └── commands/       # Command-specific unit tests
│       ├── basic_tests.rs
│       ├── string_tests.rs
│       ├── list_tests.rs
│       ├── set_tests.rs
│       └── expiry_tests.rs
└── integration/
    ├── test_client.rs  # RESP protocol test client
    └── integration_tests.rs  # End-to-end tests
```

## Running Tests

### Unit Tests

Run all unit tests:
```bash
cargo test --lib
```

Run specific module tests:
```bash
cargo test --lib string_tests
cargo test --lib list_tests
```

Run with output:
```bash
cargo test --lib -- --nocapture
```

### Integration Tests

Integration tests require a running server.

1. Start the server in one terminal:
```bash
cargo run --release
```

2. Run integration tests in another terminal:
```bash
cargo test --test integration_tests -- --ignored
```

Or use the provided script (Unix):
```bash
./scripts/run_integration_tests.sh
```

### All Tests

Run everything:
```bash
cargo test --all
```

## Code Coverage

Install tarpaulin:
```bash
cargo install cargo-tarpaulin
```

Generate coverage report:
```bash
cargo tarpaulin --out Html
```

Open `tarpaulin-report.html` in your browser.

## Continuous Integration

Tests run automatically on:
- Every push to `main` or `develop` branches
- Every pull request

CI pipeline includes:
- Unit tests on Linux, Windows, macOS
- Integration tests on Linux
- Code formatting check (`rustfmt`)
- Linting (`clippy`)
- Code coverage reporting

## Writing Tests

### Unit Tests

Unit tests test individual commands in isolation:

```rust
#[test]
fn test_set_and_get() {
    let state = common::create_test_state();
    
    // SET command
    let set_cmd = ParsedCommand {
        name: "SET".into(),
        args: vec!["key".into(), "value".into()],
    };
    let reply = rivetdb::commands::process_command(set_cmd, &state);
    assert!(matches!(reply, RespReply::Simple(s) if s == "OK"));
    
    // GET command
    let get_cmd = ParsedCommand {
        name: "GET".into(),
        args: vec!["key".into()],
    };
    let reply = rivetdb::commands::process_command(get_cmd, &state);
    assert!(matches!(reply, RespReply::Bulk(Some(s)) if s == "value"));
}
```

### Integration Tests

Integration tests test the full RESP protocol:

```rust
#[test]
#[ignore]  // Requires running server
fn test_roundtrip() {
    let mut client = TestClient::connect("127.0.0.1:7878")
        .expect("Failed to connect");
    
    client.expect_ok(&["SET", "key", "value"])
        .expect("SET failed");
    
    let value = client.expect_bulk(&["GET", "key"])
        .expect("GET failed");
    
    assert!(value.is_some());
}
```

## Test Utilities

Common test utilities are in `tests/common/mod.rs`:

- `create_test_state()` - Create fresh server state
- `db_size()` - Get number of keys
- `key_exists()` - Check if key exists
- `get_string_value()` - Get string value safely

## Coverage Goals

- **Unit tests:** >80% code coverage
- **Integration tests:** All major command flows
- **Edge cases:** Error conditions, type mismatches, TTL expiry

## Next Steps

### Planned Test Additions

- [ ] Property-based testing with `proptest`
- [ ] Fuzzing for RESP parser with `cargo-fuzz`
- [ ] Benchmark suite with `criterion`
- [ ] Stress tests for concurrent access
- [ ] Memory leak tests (Valgrind integration)
