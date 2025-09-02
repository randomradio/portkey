use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::secretbox;
use sodiumoxide::crypto::pwhash::argon2id13;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::crypto::{generate_salt, MasterKey};
use crate::models::{Server, VaultData};

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultFile {
    pub salt: argon2id13::Salt,
    pub nonce: secretbox::Nonce,
    pub ciphertext: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Vault {
    data_path: PathBuf,
    master_key: Option<MasterKey>,
    data: Option<VaultData>,
}

impl Vault {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .context("Failed to find data directory")?
            .join("portkey");
        
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
        }

        let data_path = data_dir.join("vault.dat");

        Ok(Self {
            data_path,
            master_key: None,
            data: None,
        })
    }

    pub fn exists(&self) -> bool {
        self.data_path.exists()
    }

    pub fn unlock(&mut self, password: Option<&str>) -> Result<()> {
        if !self.exists() {
            return Err(anyhow::anyhow!("Vault does not exist"));
        }

        let vault_file = self.load_vault_file()?;
        
        // Try to decrypt with password if provided
        if let Some(password) = password {
            let master_key = MasterKey::from_password(password, &vault_file.salt)?;
            
            // Check if this looks like encrypted data by attempting decryption
            let decrypted_data = master_key.decrypt(&vault_file.ciphertext, &vault_file.nonce)?;
            let vault_data: VaultData = serde_json::from_slice(&decrypted_data)
                .context("Failed to deserialize vault data")?;

            self.master_key = Some(master_key);
            self.data = Some(vault_data);
        } else {
            // No password provided, assume unencrypted vault
            let vault_data: VaultData = serde_json::from_slice(&vault_file.ciphertext)
                .context("Failed to deserialize vault data - try providing a password")?;
            
            self.master_key = None;
            self.data = Some(vault_data);
        }

        Ok(())
    }

    pub fn create(&mut self, password: Option<&str>) -> Result<()> {
        if self.exists() {
            return Err(anyhow::anyhow!("Vault already exists"));
        }

        let vault_data = VaultData::new();
        let serialized = serde_json::to_vec(&vault_data)?;

        let vault_file = if let Some(password) = password {
            // Password-protected vault
            let salt = generate_salt();
            let master_key = MasterKey::from_password(password, &salt)?;
            let (nonce, ciphertext) = master_key.encrypt(&serialized);
            
            VaultFile {
                salt,
                nonce,
                ciphertext,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        } else {
            // Unencrypted vault (no password)
            let salt = generate_salt(); // Still use salt for consistency
            let nonce = secretbox::gen_nonce();
            
            VaultFile {
                salt,
                nonce,
                ciphertext: serialized, // Store data unencrypted
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        };

        self.save_vault_file(&vault_file)?;
        
        if password.is_some() {
            let master_key = MasterKey::from_password(password.unwrap(), &vault_file.salt)?;
            self.master_key = Some(master_key);
        }
        self.data = Some(vault_data);

        Ok(())
    }

    pub fn is_unlocked(&self) -> bool {
        self.data.is_some()
    }

    pub fn add_server(&mut self, server: Server) -> Result<()> {
        self.ensure_unlocked()?;
        
        let data = self.data.as_mut().unwrap();
        data.add_server(server);
        
        self.save()?;
        Ok(())
    }

    pub fn remove_server(&mut self, id: &uuid::Uuid) -> Result<bool> {
        self.ensure_unlocked()?;
        
        let data = self.data.as_mut().unwrap();
        let removed = data.remove_server(id);
        
        if removed {
            self.save()?;
        }
        
        Ok(removed)
    }

    pub fn list_servers(&self) -> Result<&Vec<Server>> {
        self.ensure_unlocked()?;
        
        Ok(&self.data.as_ref().unwrap().servers)
    }

    pub fn find_server(&self, id: &uuid::Uuid) -> Result<Option<&Server>> {
        self.ensure_unlocked()?;
        
        Ok(self.data.as_ref().unwrap().find_server(id))
    }

    pub fn replace_server(&mut self, server: Server) -> Result<bool> {
        self.ensure_unlocked()?;
        let data = self.data.as_mut().unwrap();
        let replaced = data.replace_server(server);
        if replaced { self.save()?; }
        Ok(replaced)
    }

    pub fn vault_path(&self) -> &PathBuf {
        &self.data_path
    }

    fn ensure_unlocked(&self) -> Result<()> {
        if !self.is_unlocked() {
            return Err(anyhow::anyhow!("Vault is locked"));
        }
        Ok(())
    }

    fn load_vault_file(&self) -> Result<VaultFile> {
        let content = fs::read(&self.data_path)?;
        let vault_file: VaultFile = serde_json::from_slice(&content)?;
        Ok(vault_file)
    }

    fn save_vault_file(&self, vault_file: &VaultFile) -> Result<()> {
        let content = serde_json::to_vec(vault_file)?;
        
        // Set restrictive permissions before writing
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.data_path)?;
            
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600); // Read/write for owner only
        file.set_permissions(perms)?;
        
        file.write_all(&content)?;
        Ok(())
    }

    fn save(&mut self) -> Result<()> {
        let data = self.data.as_ref().unwrap();
        let serialized = serde_json::to_vec(data)?;

        let vault_file = if let Some(master_key) = &self.master_key {
            // Encrypted vault: reuse existing salt to keep key derivation stable
            let existing = self.load_vault_file().ok();
            let salt = existing.as_ref().map(|f| f.salt).unwrap_or_else(generate_salt);

            let (nonce, ciphertext) = master_key.encrypt(&serialized);
            VaultFile {
                salt,
                nonce,
                ciphertext,
                created_at: existing.map(|f| f.created_at).unwrap_or_else(|| Utc::now()),
                updated_at: Utc::now(),
            }
        } else {
            // Unencrypted vault
            let salt = generate_salt();
            let nonce = secretbox::gen_nonce();
            
            VaultFile {
                salt,
                nonce,
                ciphertext: serialized, // Store unencrypted
                created_at: self.load_vault_file().map(|f| f.created_at).unwrap_or_else(|_| Utc::now()),
                updated_at: Utc::now(),
            }
        };
        
        self.save_vault_file(&vault_file)?;
        Ok(())
    }
}
