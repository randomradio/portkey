use portkey::cli::password_option_from_choice;
use portkey::models::Server;
use portkey::ssh::{build_ssh_args, manual_connection_help};
use portkey::ssh_config::{render_managed_block, render_ssh_config, upsert_managed_block};
use portkey::vault::Vault;
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

#[test]
fn native_ssh_args_support_identity_file_and_agent_forwarding() {
    let mut server = Server::new(
        "prod".to_string(),
        "example.com".to_string(),
        2222,
        "deploy".to_string(),
        String::new(),
        None,
    );
    server.identity_file = Some("~/.ssh/id_ed25519".to_string());
    server.forward_agent = true;

    let args = build_ssh_args(&server);

    assert!(args.contains(&"-tt".to_string()));
    assert!(args.contains(&"-i".to_string()));
    assert!(args.contains(&"~/.ssh/id_ed25519".to_string()));
    assert!(args.contains(&"-A".to_string()));
    assert!(args.contains(&"deploy@example.com".to_string()));
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"2222".to_string()));
    assert!(!args
        .iter()
        .any(|arg| arg.contains("StrictHostKeyChecking=no")));
}

#[test]
fn manual_connection_help_never_prints_password() {
    let mut server = Server::new(
        "prod".to_string(),
        "example.com".to_string(),
        22,
        "deploy".to_string(),
        "super-secret".to_string(),
        None,
    );
    server.identity_file = Some("~/.ssh/id_ed25519".to_string());
    server.forward_agent = true;

    let help = manual_connection_help(&server);

    assert!(help.contains("ssh -tt -i ~/.ssh/id_ed25519 -A -p 22 deploy@example.com"));
    assert!(!help.contains("super-secret"));
}

#[test]
fn ssh_config_includes_session_options_and_rejects_unsafe_aliases() {
    let mut server = Server::new(
        "prod".to_string(),
        "example.com".to_string(),
        2222,
        "deploy".to_string(),
        String::new(),
        None,
    );
    server.identity_file = Some("~/.ssh/id_ed25519".to_string());
    server.forward_agent = true;

    let config = render_ssh_config(&[server]).unwrap();

    assert!(config.contains("Host prod"));
    assert!(config.contains("  HostName example.com"));
    assert!(config.contains("  User deploy"));
    assert!(config.contains("  Port 2222"));
    assert!(config.contains("  IdentityFile ~/.ssh/id_ed25519"));
    assert!(config.contains("  ForwardAgent yes"));

    let unsafe_server = Server::new(
        "prod\nHost attacker".to_string(),
        "example.com".to_string(),
        22,
        "deploy".to_string(),
        String::new(),
        None,
    );

    assert!(render_ssh_config(&[unsafe_server]).is_err());
}

#[test]
fn ssh_config_managed_block_is_replaced_instead_of_appended() {
    let server = Server::new(
        "prod".to_string(),
        "example.com".to_string(),
        22,
        "deploy".to_string(),
        String::new(),
        None,
    );
    let first_block = render_managed_block(&[server]).unwrap();
    let existing = format!("Host github.com\n  HostName github.com\n\n{first_block}");

    let next_server = Server::new(
        "staging".to_string(),
        "staging.example.com".to_string(),
        2222,
        "deploy".to_string(),
        String::new(),
        None,
    );
    let next_block = render_managed_block(&[next_server]).unwrap();
    let updated = upsert_managed_block(&existing, &next_block);

    assert!(updated.contains("Host github.com"));
    assert!(updated.contains("Host staging"));
    assert!(!updated.contains("Host prod"));
    assert_eq!(
        updated.matches("# BEGIN Portkey managed entries").count(),
        1
    );
}

#[test]
fn password_protected_vault_requires_non_empty_master_password() {
    assert!(password_option_from_choice(true, "").is_err());
    assert_eq!(password_option_from_choice(false, "").unwrap(), None);
    assert_eq!(
        password_option_from_choice(true, "master-password").unwrap(),
        Some("master-password")
    );
}

#[test]
fn vault_round_trip_preserves_key_session_options_with_restrictive_permissions() {
    let temp = tempdir().unwrap();
    let vault_path = temp.path().join("vault.dat");
    let mut vault = Vault::new_at(vault_path.clone()).unwrap();
    vault.create(Some("master-password")).unwrap();

    let mut server = Server::new(
        "prod".to_string(),
        "example.com".to_string(),
        2222,
        "deploy".to_string(),
        String::new(),
        None,
    );
    server.identity_file = Some("~/.ssh/id_ed25519".to_string());
    server.forward_agent = true;
    vault.add_server(server).unwrap();

    let mut reopened = Vault::new_at(vault_path.clone()).unwrap();
    reopened.unlock(Some("master-password")).unwrap();
    let servers = reopened.list_servers().unwrap();

    assert_eq!(servers.len(), 1);
    assert_eq!(
        servers[0].identity_file.as_deref(),
        Some("~/.ssh/id_ed25519")
    );
    assert!(servers[0].forward_agent);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(vault_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
