use anyhow::{anyhow, Result};
use std::process::Command;

use crate::models::Server;

fn command_exists(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn build_ssh_args(server: &Server) -> Vec<String> {
    let mut args = vec!["-tt".to_string()];

    if let Some(identity_file) = server
        .identity_file
        .as_deref()
        .filter(|path| !path.is_empty())
    {
        args.push("-i".to_string());
        args.push(identity_file.to_string());
    }

    if server.forward_agent {
        args.push("-A".to_string());
    }

    args.push("-p".to_string());
    args.push(server.port.to_string());
    args.push(format!("{}@{}", server.username, server.host));
    args
}

fn shell_quote(arg: &str) -> String {
    if arg
        .chars()
        .all(|c| !c.is_whitespace() && !matches!(c, '\'' | '"' | '\\' | '$' | '`'))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn ssh_command_line(server: &Server) -> String {
    let args = build_ssh_args(server)
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    format!("ssh {args}")
}

pub fn manual_connection_help(server: &Server) -> String {
    format!(
        "Connect manually with:\n  {}\nPassword is stored in Portkey and will not be printed.",
        ssh_command_line(server)
    )
}

pub fn connect(server: &Server) -> Result<()> {
    println!(
        "Connecting to {}@{}:{}...",
        server.username, server.host, server.port
    );

    if !command_exists("ssh") {
        return Err(anyhow!("ssh is not installed or not in PATH"));
    }

    let ssh_args = build_ssh_args(server);
    let has_password = !server.password.is_empty();

    let status = if has_password {
        if !command_exists("sshpass") {
            eprintln!("❌ sshpass is not installed or not in PATH.");
            eprintln!();
            eprintln!("Install sshpass to use password authentication:");
            eprintln!("  macOS: brew install hudochenkov/sshpass/sshpass");
            eprintln!("  Ubuntu/Debian: sudo apt-get install sshpass");
            eprintln!("  CentOS/RHEL: sudo yum install sshpass");
            eprintln!("  Arch: sudo pacman -S sshpass");
            eprintln!();
            eprintln!("{}", manual_connection_help(server));
            return Err(anyhow!(
                "sshpass is required for stored password authentication"
            ));
        }

        Command::new("sshpass")
            .env("SSHPASS", &server.password)
            .env(
                "TERM",
                std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()),
            )
            .arg("-e")
            .arg("ssh")
            .args(&ssh_args)
            .status()?
    } else {
        Command::new("ssh")
            .env(
                "TERM",
                std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()),
            )
            .args(&ssh_args)
            .status()?
    };

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "SSH connection failed. Possible causes: server unreachable, invalid credentials, SSH service not running, or port blocked by firewall"
        ))
    }
}
