use anyhow::{anyhow, Result};

use crate::models::Server;

pub const BEGIN_MARKER: &str = "# BEGIN Portkey managed entries";
pub const END_MARKER: &str = "# END Portkey managed entries";

fn validate_non_empty_single_line(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(anyhow!("{label} cannot be empty"));
    }

    if value.chars().any(|c| c == '\n' || c == '\r') {
        return Err(anyhow!("{label} cannot contain newlines"));
    }

    Ok(())
}

fn validate_host_alias(alias: &str) -> Result<()> {
    validate_non_empty_single_line("Host alias", alias)?;

    if alias.chars().any(char::is_whitespace) {
        return Err(anyhow!("Host alias cannot contain whitespace"));
    }

    Ok(())
}

fn validate_server(server: &Server) -> Result<()> {
    validate_host_alias(&server.name)?;
    validate_non_empty_single_line("HostName", &server.host)?;
    validate_non_empty_single_line("User", &server.username)?;

    if let Some(identity_file) = server.identity_file.as_deref() {
        if !identity_file.is_empty() {
            validate_non_empty_single_line("IdentityFile", identity_file)?;
        }
    }

    Ok(())
}

pub fn render_ssh_config(servers: &[Server]) -> Result<String> {
    let mut output = String::new();

    for server in servers {
        validate_server(server)?;
        output.push_str(&format!(
            "Host {}\n  HostName {}\n  User {}\n  Port {}\n",
            server.name, server.host, server.username, server.port
        ));

        if let Some(identity_file) = server
            .identity_file
            .as_deref()
            .filter(|path| !path.is_empty())
        {
            output.push_str(&format!("  IdentityFile {identity_file}\n"));
        }

        if server.forward_agent {
            output.push_str("  ForwardAgent yes\n");
        }

        output.push('\n');
    }

    Ok(output)
}

pub fn render_managed_block(servers: &[Server]) -> Result<String> {
    let config = render_ssh_config(servers)?;
    Ok(format!("{BEGIN_MARKER}\n{config}{END_MARKER}\n"))
}

pub fn upsert_managed_block(existing: &str, managed_block: &str) -> String {
    if let Some(begin) = existing.find(BEGIN_MARKER) {
        if let Some(relative_end) = existing[begin..].find(END_MARKER) {
            let end = begin + relative_end + END_MARKER.len();
            let before = existing[..begin].trim_end();
            let after = existing[end..].trim_start_matches(['\r', '\n']);

            return match (before.is_empty(), after.is_empty()) {
                (true, true) => managed_block.to_string(),
                (true, false) => format!("{managed_block}\n{after}"),
                (false, true) => format!("{before}\n\n{managed_block}"),
                (false, false) => format!("{before}\n\n{managed_block}\n{after}"),
            };
        }
    }

    let existing = existing.trim_end();
    if existing.is_empty() {
        managed_block.to_string()
    } else {
        format!("{existing}\n\n{managed_block}")
    }
}
