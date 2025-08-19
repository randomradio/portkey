use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

impl Server {
    pub fn new(
        name: String,
        host: String,
        port: u16,
        username: String,
        password: String,
        description: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            host,
            port,
            username,
            password,
            description,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
        }
    }

    pub fn ssh_command(&self) -> String {
        format!("ssh {}@{} -p {}", self.username, self.host, self.port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultData {
    pub servers: Vec<Server>,
    pub version: String,
}

impl VaultData {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            version: "1.0.0".to_string(),
        }
    }

    pub fn add_server(&mut self, server: Server) {
        self.servers.push(server);
    }

    pub fn remove_server(&mut self, id: &Uuid) -> bool {
        let len = self.servers.len();
        self.servers.retain(|s| &s.id != id);
        self.servers.len() != len
    }

    pub fn find_server(&self, id: &Uuid) -> Option<&Server> {
        self.servers.iter().find(|s| &s.id == id)
    }

    pub fn list_servers(&self) -> &Vec<Server> {
        &self.servers
    }
}