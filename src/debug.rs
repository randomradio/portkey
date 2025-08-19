use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::vault::Vault;

pub fn debug_vault() {
    println!("🔍 Vault Debug Information");
    println!("==========================");
    
    match Vault::new() {
        Ok(vault) => {
            // Get path via debug method
            let vault_path = vault.vault_path().clone();
            println!("Vault path: {}", vault_path.display());
            
            let exists = vault.exists();
            println!("Vault exists: {}", exists);
            
            if exists {
                if let Ok(metadata) = fs::metadata(&vault_path) {
                    println!("File size: {} bytes", metadata.len());
                    
                    #[cfg(unix)]
                    {
                        println!("Permissions: {:o}", metadata.permissions().mode());
                    }
                    
                    if let Ok(modified) = metadata.modified() {
                        println!("Modified: {:?}", modified);
                    }
                }
                
                if let Ok(content) = fs::read(&vault_path) {
                    println!("File readable: ✅");
                    println!("Content size: {} bytes", content.len());
                } else {
                    println!("File readable: ❌");
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to determine vault path: {}", e);
        }
    }
}