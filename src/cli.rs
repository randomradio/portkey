use anyhow::Result;
use clap::{Parser, Subcommand};
use inquire::{Confirm, Password, Select, Text};
use std::process::Command;

use crate::models::Server;
use crate::vault::Vault;

#[derive(Parser)]
#[command(name = "portkey")]
#[command(about = "Secure SSH credential manager")]
#[command(version = "1.0.0")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new vault
    Init,
    
    /// Add a new server
    Add,
    
    /// List all servers
    List,
    
    /// Connect to a server
    Connect {
        /// Server name or ID
        name: Option<String>,
    },
    
    /// Remove a server
    Remove {
        /// Server name or ID
        name: String,
    },
    
    /// Interactive server selection and connection
    Quick,
    
    /// Search servers
    Search {
        query: String,
    },
}

pub struct CliHandler {
    vault: Vault,
}

impl CliHandler {
    pub fn new() -> Result<Self> {
        let vault = Vault::new()?;
        Ok(Self { vault })
    }

    pub async fn run(&mut self) -> Result<()> {
        let cli = Cli::parse();

        match cli.command {
            Some(Commands::Init) => self.handle_init().await?,
            Some(Commands::Add) => self.handle_add().await?,
            Some(Commands::List) => self.handle_list().await?,
            Some(Commands::Connect { name }) => self.handle_connect(name).await?,
            Some(Commands::Remove { name }) => self.handle_remove(name).await?,
            Some(Commands::Quick) => self.handle_quick().await?,
            Some(Commands::Search { query }) => self.handle_search(query).await?,
            None => self.handle_interactive().await?,
        }

        Ok(())
    }

    async fn handle_init(&mut self) -> Result<()> {
        if self.vault.exists() {
            let confirmed = Confirm::new("Vault already exists. Do you want to overwrite it?")
                .with_default(false)
                .prompt()?;
            
            if !confirmed {
                println!("Operation cancelled.");
                return Ok(());
            }
        }

        let password = Password::new("Enter master password:")
            .with_display_toggle_enabled()
            .with_custom_confirmation_message("Confirm master password:")
            .prompt()?;

        self.vault.create(&password)?;
        println!("Vault created successfully!");

        Ok(())
    }

    async fn handle_add(&mut self) -> Result<()> {
        self.ensure_unlocked().await?;

        let name = Text::new("Server name:").prompt()?;
        let host = Text::new("Host/IP:").prompt()?;
        let port = Text::new("Port:")
            .with_default("22")
            .prompt()?
            .parse::<u16>()
            .unwrap_or(22);
        let username = Text::new("Username:").prompt()?;
        let password = Password::new("Password:")
            .with_display_toggle_enabled()
            .prompt()?;
        let description = Text::new("Description (optional):").prompt().ok();

        let server = Server::new(
            name,
            host,
            port,
            username,
            password,
            description,
        );

        self.vault.add_server(server)?;
        println!("Server added successfully!");

        Ok(())
    }

    async fn handle_list(&mut self) -> Result<()> {
        self.ensure_unlocked().await?;

        let servers = self.vault.list_servers()?;
        
        if servers.is_empty() {
            println!("No servers configured.");
            return Ok(());
        }

        println!("\nConfigured servers:");
        println!("{:-<60}", "");
        
        for server in servers {
            println!("ID: {}", server.id);
            println!("Name: {}", server.name);
            println!("Host: {}:{}", server.host, server.port);
            println!("User: {}", server.username);
            if let Some(desc) = &server.description {
                println!("Description: {}", desc);
            }
            println!("{:-<60}", "");
        }

        Ok(())
    }

    async fn handle_connect(&mut self, name: Option<String>) -> Result<()> {
        self.ensure_unlocked().await?;

        let server = match name {
            Some(name) => self.find_server_by_name_or_id(&name)?,
            None => {
                let servers = self.vault.list_servers()?;
                if servers.is_empty() {
                    println!("No servers available.");
                    return Ok(());
                }

                let options: Vec<String> = servers
                    .iter()
                    .map(|s| format!("{} ({})", s.name, s.host))
                    .collect();

                let selection = Select::new("Select server:", options)
                    .prompt()?;

                let index = servers.iter().position(|s| 
                    format!("{} ({})", s.name, s.host) == selection
                ).unwrap();
                
                &servers[index]
            }
        };

        self.connect_to_server(server).await
    }

    async fn handle_remove(&mut self, name: String) -> Result<()> {
        self.ensure_unlocked().await?;

        let server_id = {
            let server = self.find_server_by_name_or_id(&name)?;
            server.id
        };
        
        let server = self.vault.find_server(&server_id)?
            .ok_or_else(|| anyhow::anyhow!("Server not found"))?;
        
        let confirmed = Confirm::new(&format!("Remove server '{}' ({})?", server.name, server.host)
        ).with_default(false).prompt()?;

        if confirmed {
            self.vault.remove_server(&server_id)?;
            println!("Server removed successfully!");
        } else {
            println!("Operation cancelled.");
        }

        Ok(())
    }

    async fn handle_quick(&mut self) -> Result<()> {
        self.ensure_unlocked().await?;

        let servers = self.vault.list_servers()?;
        if servers.is_empty() {
            println!("No servers available.");
            return Ok(());
        }

        let options: Vec<String> = servers
            .iter()
            .map(|s| format!("{}@{}:{}", s.username, s.host, s.port))
            .collect();

        let selection = Select::new("Select server to connect:", options).prompt()?;
        
        let index = servers.iter().position(|s| 
            format!("{}@{}:{}", s.username, s.host, s.port) == selection
        ).unwrap();
        
        let server = &servers[index];
        self.connect_to_server(server).await
    }

    async fn handle_search(&mut self, query: String) -> Result<()> {
        self.ensure_unlocked().await?;

        let servers = self.vault.list_servers()?;
        let query = query.to_lowercase();

        let matches: Vec<&Server> = servers
            .iter()
            .filter(|s| 
                s.name.to_lowercase().contains(&query) ||
                s.host.to_lowercase().contains(&query) ||
                s.username.to_lowercase().contains(&query) ||
                s.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&query))
            )
            .collect();

        if matches.is_empty() {
            println!("No servers match your search.");
            return Ok(());
        }

        println!("Search results:");
        println!("{:-<60}", "");
        
        for server in matches {
            println!("Name: {}", server.name);
            println!("Host: {}:{}", server.host, server.port);
            println!("User: {}", server.username);
            if let Some(desc) = &server.description {
                println!("Description: {}", desc);
            }
            println!("{:-<60}", "");
        }

        Ok(())
    }

    async fn handle_interactive(&mut self) -> Result<()> {
        if !self.vault.exists() {
            println!("No vault found. Run 'portkey init' to create one.");
            return Ok(());
        }

        self.ensure_unlocked().await?;

        loop {
            let options = vec![
                "Add server",
                "List servers", 
                "Connect to server",
                "Remove server",
                "Search servers",
                "Exit",
            ];

            let selection = Select::new("What would you like to do?", options).prompt()?;

            match selection {
                "Add server" => self.handle_add().await?,
                "List servers" => self.handle_list().await?,
                "Connect to server" => self.handle_quick().await?,
                "Remove server" => {
                    let servers = self.vault.list_servers()?;
                    if servers.is_empty() {
                        println!("No servers to remove.");
                        continue;
                    }

                    let options: Vec<String> = servers
                        .iter()
                        .map(|s| format!("{} ({})", s.name, s.host))
                        .collect();

                    if let Ok(selection) = Select::new("Select server to remove:", options).prompt() {
                        let index = servers.iter().position(|s| 
                            format!("{} ({})", s.name, s.host) == selection
                        ).unwrap();
                        
                        let server = &servers[index];
                        self.handle_remove(server.name.clone()).await?;
                    }
                },
                "Search servers" => {
                    let query = Text::new("Search query:").prompt()?;
                    self.handle_search(query).await?;
                },
                "Exit" => break,
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    async fn ensure_unlocked(&mut self) -> Result<()> {
        if !self.vault.exists() {
            return Err(anyhow::anyhow!("No vault found. Run 'portkey init' to create one."));
        }

        if !self.vault.is_unlocked() {
            let password = Password::new("Enter master password:")
                .with_display_toggle_enabled()
                .prompt()?;
            
            self.vault.unlock(&password)?;
            println!("Vault unlocked!");
        }

        Ok(())
    }

    fn find_server_by_name_or_id(&self, name_or_id: &str) -> Result<&Server> {
        let servers = self.vault.list_servers()?;
        
        servers.iter()
            .find(|s| 
                s.name.eq_ignore_ascii_case(name_or_id) || 
                s.id.to_string().starts_with(name_or_id)
            )
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", name_or_id))
    }

    async fn connect_to_server(&self, 
        server: &Server
    ) -> Result<()> {
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
            eprintln!("  ssh {}@{} -p {}", server.username, server.host, server.port);
            eprintln!("  Password: {}", server.password);
            
            return Ok(());
        }

        // Use sshpass for password authentication
        let status = Command::new("sshpass")
            .arg("-p")
            .arg(&server.password)
            .arg("ssh")
            .arg(format!("{}@{}", server.username, server.host))
            .arg("-p")
            .arg(server.port.to_string())
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .status()?;

        if !status.success() {
            eprintln!("❌ SSH connection failed.");
            eprintln!("Possible causes:");
            eprintln!("  - Server is not reachable");
            eprintln!("  - Invalid credentials");
            eprintln!("  - SSH service is not running");
            eprintln!("  - Port is blocked by firewall");
        }

        Ok(())
    }
}