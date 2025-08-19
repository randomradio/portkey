use portkey::models::Server;
use tempfile::tempdir;

#[test]
fn test_server_creation() {
    let server = Server::new(
        "test-server".to_string(),
        "192.168.1.1".to_string(),
        22,
        "admin".to_string(),
        "password123".to_string(),
        Some("Test server".to_string()),
    );

    assert_eq!(server.name, "test-server");
    assert_eq!(server.host, "192.168.1.1");
    assert_eq!(server.port, 22);
    assert_eq!(server.username, "admin");
    assert_eq!(server.password, "password123");
    assert_eq!(server.description, Some("Test server".to_string()));
}

#[test]
fn test_ssh_command_generation() {
    let server = Server::new(
        "prod-server".to_string(),
        "example.com".to_string(),
        2222,
        "deploy".to_string(),
        "secret".to_string(),
        None,
    );

    let expected = "ssh deploy@example.com -p 2222";
    assert_eq!(server.ssh_command(), expected);
}