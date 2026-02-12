use anyhow::Result;
use std::process::Command;

use crate::models::Server;

pub fn connect(server: &Server) -> Result<()> {
    println!("Connecting to {}@{}:{}...", server.username, server.host, server.port);

    // Check if sshpass is available
    let sshpass_check = Command::new("which").arg("sshpass").output();
    let sshpass_available = sshpass_check.is_ok() && sshpass_check.unwrap().status.success();

    if !sshpass_available {
        eprintln!("❌ sshpass is not installed or not in PATH.");
        eprintln!("");
        eprintln!("Install sshpass to use password authentication:");
        eprintln!("  macOS: brew install hudochenkov/sshpass/sshpass");
        eprintln!("  Ubuntu/Debian: sudo apt-get install sshpass");
        eprintln!("  CentOS/RHEL: sudo yum install sshpass");
        eprintln!("  Arch: sudo pacman -S sshpass");
        eprintln!("");
        eprintln!("Alternatively, connect manually:");
        eprintln!("  {}", server.ssh_command());
        eprintln!("  Password: {}", server.password);
        return Ok(());
    }

    // Use sshpass with env var to avoid password in process args
    let status = Command::new("sshpass")
        .env("SSHPASS", &server.password)
        .env("TERM", std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()))
        .arg("-e")
        .arg("ssh")
        .arg("-tt") // Force PTY allocation for interactive sessions
        .arg(format!("{}@{}", server.username, server.host))
        .arg("-p")
        .arg(server.port.to_string())
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .status()?;

    if !status.success() {
        eprintln!("❌ SSH connection failed.");
        eprintln!("Possible causes:\n  - Server unreachable\n  - Invalid credentials\n  - SSH service not running\n  - Port blocked by firewall");
    }

    Ok(())
}

