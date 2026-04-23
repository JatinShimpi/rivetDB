/// Integration tests for basic RESP protocol functionality
/// These tests require a running RivetDB server on localhost:7878
/// 
/// To run these tests:
/// 1. Start the server: cargo run --release
/// 2. Run integration tests: cargo test --test integration_tests

#[path = "integration/test_client.rs"]
mod test_client;

use test_client::TestClient;

#[test]
#[ignore] // Ignored by default, run with: cargo test -- --ignored
fn test_ping_pong() {
    let mut client = TestClient::connect("127.0.0.1:7878")
        .expect("Failed to connect - is server running?");
    
    let response = client.send_command(&["PING"]).expect("PING failed");
    assert!(response.contains("PONG"), "Expected PONG, got: {}", response);
}

#[test]
#[ignore]
fn test_set_get_roundtrip() {
    let mut client = TestClient::connect("127.0.0.1:7878")
        .expect("Failed to connect - is server running?");
    
    // SET
    client.expect_ok(&["SET", "testkey", "testvalue"])
        .expect("SET failed");
    
    // GET
    let value = client.expect_bulk(&["GET", "testkey"])
        .expect("GET failed");
    
    assert!(value.is_some(), "Expected value, got None");
}

#[test]
#[ignore]
fn test_incr_decr() {
    let mut client = TestClient::connect("127.0.0.1:7878")
        .expect("Failed to connect - is server running?");
    
    // INCR new key
    let val = client.expect_integer(&["INCR", "counter"])
        .expect("INCR failed");
    assert_eq!(val, 1);
    
    // INCR again
    let val = client.expect_integer(&["INCR", "counter"])
        .expect("INCR failed");
    assert_eq!(val, 2);
    
    // DECR
    let val = client.expect_integer(&["DECR", "counter"])
        .expect("DECR failed");
    assert_eq!(val, 1);
}

#[test]
#[ignore]
fn test_list_operations() {
    let mut client = TestClient::connect("127.0.0.1:7878")
        .expect("Failed to connect - is server running?");
    
    // LPUSH
    let len = client.expect_integer(&["LPUSH", "mylist", "value1"])
        .expect("LPUSH failed");
    assert_eq!(len, 1);
    
    // LLEN
    let len = client.expect_integer(&["LLEN", "mylist"])
        .expect("LLEN failed");
    assert_eq!(len, 1);
}

#[test]
#[ignore]
fn test_set_operations() {
    let mut client = TestClient::connect("127.0.0.1:7878")
        .expect("Failed to connect - is server running?");
    
    // SADD
    let added = client.expect_integer(&["SADD", "myset", "member1", "member2"])
        .expect("SADD failed");
    assert_eq!(added, 2);
    
    // SADD duplicate
    let added = client.expect_integer(&["SADD", "myset", "member1"])
        .expect("SADD failed");
    assert_eq!(added, 0, "Adding duplicate should return 0");
}

#[test]
#[ignore]
fn test_multiple_clients() {
    // Test concurrent connections
    let mut client1 = TestClient::connect("127.0.0.1:7878")
        .expect("Client 1 failed to connect");
    let mut client2 = TestClient::connect("127.0.0.1:7878")
        .expect("Client 2 failed to connect");
    
    // Client 1 sets a key
    client1.expect_ok(&["SET", "key1", "value1"])
        .expect("Client 1 SET failed");
    
    // Client 2 should be able to read it
    let value = client2.expect_bulk(&["GET", "key1"])
        .expect("Client 2 GET failed");
    assert!(value.is_some());
}

#[test]
#[ignore]
fn test_del_exists() {
    let mut client = TestClient::connect("127.0.0.1:7878")
        .expect("Failed to connect - is server running?");
    
    // SET
    client.expect_ok(&["SET", "key", "value"])
        .expect("SET failed");
    
    // EXISTS should return 1
    let exists = client.expect_integer(&["EXISTS", "key"])
        .expect("EXISTS failed");
    assert_eq!(exists, 1);
    
    // DEL
    let deleted = client.expect_integer(&["DEL", "key"])
        .expect("DEL failed");
    assert_eq!(deleted, 1);
    
    // EXISTS should now return 0
    let exists = client.expect_integer(&["EXISTS", "key"])
        .expect("EXISTS failed");
    assert_eq!(exists, 0);
}
