use anyhow::Result;
use sodiumoxide::crypto::secretbox;
use sodiumoxide::crypto::pwhash::argon2id13;
use zeroize::Zeroize;

pub struct MasterKey {
    key: secretbox::Key,
}

impl MasterKey {
    pub fn from_password(password: &str, salt: &argon2id13::Salt) -> Result<Self> {
        let mut key = secretbox::Key([0; secretbox::KEYBYTES]);
        
        argon2id13::derive_key(
            &mut key.0,
            password.as_bytes(),
            salt,
            argon2id13::OPSLIMIT_INTERACTIVE,
            argon2id13::MEMLIMIT_INTERACTIVE,
        )
        .map_err(|_| anyhow::anyhow!("Failed to derive key from password"))?;

        Ok(Self { key })
    }

    pub fn encrypt(&self, data: &[u8]) -> (secretbox::Nonce, Vec<u8>) {
        let nonce = secretbox::gen_nonce();
        let ciphertext = secretbox::seal(data, &nonce, &self.key);
        (nonce, ciphertext)
    }

    pub fn decrypt(&self, ciphertext: &[u8], nonce: &secretbox::Nonce) -> Result<Vec<u8>> {
        secretbox::open(ciphertext, nonce, &self.key)
            .map_err(|_| anyhow::anyhow!("Failed to decrypt data - invalid password or corrupted data"))
    }

    pub fn key(&self) -> &secretbox::Key {
        &self.key
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.key.0.zeroize();
    }
}

pub fn generate_salt() -> argon2id13::Salt {
    argon2id13::gen_salt()
}