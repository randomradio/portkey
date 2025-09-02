use anyhow::Result;
use clap::{Parser, Subcommand};
use inquire::{Confirm, Password, Select, Text};

use crate::models::Server;
use crate::vault::Vault;
use crate::tui;
use crate::ssh;
use fuzzy_matcher::FuzzyMatcher;

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

    /// Export SSH config entries for servers
    SshConfig {
        /// Actually write to ~/.ssh/config instead of printing
        #[arg(long)]
        write: bool,
    },

    /// Full-screen TUI application
    Ui,
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
            Some(Commands::SshConfig { write }) => self.handle_ssh_config(write).await?,
            Some(Commands::Ui) => self.handle_interactive().await?,
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

        let use_password = Confirm::new("Would you like to protect your vault with a master password?")
            .with_default(true)
            .prompt()?;

        let password = if use_password {
            Password::new("Enter master password:")
                .with_display_toggle_enabled()
                .prompt()?
        } else {
            println!("Creating vault without password protection...");
            String::new()
        };

        let password_opt = if password.is_empty() { None } else { Some(password.as_str()) };
        self.vault.create(password_opt)?;
        
        if use_password {
            println!("🔒 Vault created with password protection!");
        } else {
            println!("✅ Vault created without password protection!");
        }

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
        // Quick now just launches the full TUI
        self.handle_interactive().await
    }

    async fn handle_search(&mut self, query: String) -> Result<()> {
        self.ensure_unlocked().await?;

        let servers = self.vault.list_servers()?;
        let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
        let mut matches: Vec<(&Server, i64)> = servers
            .iter()
            .filter_map(|s| {
                let hay = format!("{} {} {} {} {}", s.name, s.host, s.username, s.port, s.description.as_deref().unwrap_or(""));
                matcher.fuzzy_match(&hay, &query).map(|score| (s, score))
            })
            .collect();
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        if matches.is_empty() {
            println!("No servers match your search.");
            return Ok(());
        }

        println!("Search results:");
        println!("{:-<60}", "");
        
        for (server, _) in matches {
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

    async fn handle_ssh_config(&mut self, write: bool) -> Result<()> {
        self.ensure_unlocked().await?;
        let servers = self.vault.list_servers()?;

        let mut output = String::new();
        for s in servers {
            output.push_str(&format!(
                "Host {}\n  HostName {}\n  User {}\n  Port {}\n\n",
                s.name, s.host, s.username, s.port
            ));
        }

        if write {
            let mut path = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Home directory not found"))?;
            path.push(".ssh");
            std::fs::create_dir_all(&path)?;
            path.push("config");

            // Append entries; avoid overwriting existing unrelated entries.
            use std::fs::OpenOptions;
            use std::io::Write;
            let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
            writeln!(file, "\n# Portkey managed entries")?;
            write!(file, "{}", output)?;
            println!("Written SSH config entries to {}", path.display());
        } else {
            println!("# Preview: add these to ~/.ssh/config\n{}", output);
        }

        println!("Note: SSH config does not store passwords. Consider setting up SSH keys.");
        Ok(())
    }

    async fn handle_interactive(&mut self) -> Result<()> {
        if !self.vault.exists() {
            println!("No vault found. Run 'portkey init' to create one.");
            return Ok(());
        }

        // Unlock before entering raw mode
        self.ensure_unlocked().await?;
        tui::run_full_ui(&mut self.vault).map_err(|e| anyhow::anyhow!(e))
    }

    async fn ensure_unlocked(&mut self) -> Result<()> {
        if !self.vault.exists() {
            return Err(anyhow::anyhow!("No vault found. Run 'portkey init' to create one."));
        }

        if !self.vault.is_unlocked() {
            // Try to unlock with no password first (for unencrypted vaults)
            match self.vault.unlock(None) {
                Ok(_) => {
                    println!("Vault unlocked (no password required)!");
                }
                Err(_) => {
                    // Encrypted vault - prompt for password
                    let password = Password::new("Enter master password:")
                        .with_display_toggle_enabled()
                        .prompt()?;
                    
                    self.vault.unlock(Some(&password))?;
                    println!("Vault unlocked!");
                }
            }
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
        ssh::connect(server)
    }
}
