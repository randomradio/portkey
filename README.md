# Portkey - Secure SSH Credential Manager

A secure command-line tool for managing SSH server credentials with master password encryption.

## Features

- 🔐 **Secure Storage**: AES-256-GCM encryption with PBKDF2 key derivation
- 🔑 **Master Password**: Single password unlocks all server credentials
- 🖥️ **Interactive CLI**: User-friendly prompts and menus
- 🔍 **Search**: Find servers by name, host, or description
- ⚡ **Quick Connect**: Launch SSH sessions with stored credentials

## Installation

### Quick Install (Recommended)

```bash
# Clone and install with one command
git clone <repository>
cd portkey
./install.sh
```

### Manual Installation

#### Prerequisites
Install `sshpass` for password-based SSH authentication:

```bash
# macOS
brew install hudochenkov/sshpass/sshpass

# Ubuntu/Debian
sudo apt-get install sshpass

# CentOS/RHEL
sudo yum install sshpass

# Arch Linux
sudo pacman -S sshpass
```

#### Build from source

```bash
git clone <repository>
cd portkey
cargo build --release
```

The binary will be available at `target/release/portkey`.

### Making it globally available

```bash
# Option 1: Copy to system PATH
sudo cp target/release/portkey /usr/local/bin/

# Option 2: Add to your PATH
export PATH="$PATH:$(pwd)/target/release"
# Add this line to your ~/.bashrc or ~/.zshrc for persistence
```

## Usage

### 1. Initialize the vault

```bash
./portkey init
```

You'll be prompted to create a master password. This password will be used to unlock your vault.

### 2. Add a server

```bash
./portkey add
```

Interactive prompts will ask for:
- Server name (e.g., "production-web")
- Host/IP (e.g., "192.168.1.100")
- Port (default: 22)
- Username
- Password
- Description (optional)

### 3. List servers

```bash
./portkey list
```

### 4. Connect to a server

```bash
# Interactive selection
./portkey quick

# Connect by name
./portkey connect production-web

# Search and connect
./portkey search web
```

### 5. Interactive mode

```bash
./portkey
```

Opens an interactive menu with all available operations.

### 6. Remove a server

```bash
./portkey remove production-web
```

## Security Features

- **Encryption**: AES-256-GCM with unique salts and nonces
- **Key Derivation**: PBKDF2 with Argon2id13
- **Memory Safety**: Rust's memory safety guarantees
- **File Permissions**: Vault file restricted to owner only (600)
- **Zeroize**: Sensitive data cleared from memory after use

## Troubleshooting

### sshpass not found

If you see "sshpass is not installed or not in PATH":

1. **macOS**: `brew install hudochenkov/sshpass/sshpass`
2. **Ubuntu/Debian**: `sudo apt-get install sshpass`
3. **CentOS/RHEL**: `sudo yum install sshpass`
4. **Arch Linux**: `sudo pacman -S sshpass`

### Manual connection

If sshpass isn't available, you can still use portkey to store credentials and connect manually:

```bash
# After adding a server, get connection details
./portkey list

# Then connect manually
ssh username@host -p port
# Use the password displayed by portkey
```

### SSH key alternative

For better security, consider using SSH keys instead of passwords:

1. Generate SSH key: `ssh-keygen -t ed25519`
2. Copy to server: `ssh-copy-id user@host`
3. Then you can connect without passwords: `ssh user@host`

## Data Storage

- **Location**: `~/.local/share/portkey/vault.dat` (Linux) or `~/Library/Application Support/portkey/vault.dat` (macOS)
- **Encryption**: All data encrypted with your master password
- **Backup**: Back up the vault file to restore your servers

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize a new vault |
| `add` | Add a new server |
| `list` | List all servers |
| `connect [name]` | Connect to a server |
| `remove [name]` | Remove a server |
| `quick` | Interactive server selection |
| `search [query]` | Search servers |
| (no command) | Interactive mode |

## Example Session

```bash
$ ./portkey init
Enter master password: ••••••••
Confirm master password: ••••••••
Vault created successfully!

$ ./portkey add
Server name: production-web
Host/IP: 203.0.113.10
Port: 22
Username: ubuntu
Password: ••••••••
Description (optional): Main production web server
Server added successfully!

$ ./portkey quick
Select server to connect:
❯ ubuntu@203.0.113.10:22
  deploy@203.0.113.11:22

Connecting to ubuntu@203.0.113.10:22...
```

## Security Notes

- **Never share** your vault file or master password
- **Use strong passwords** for the master password
- **Keep backups** of your vault file
- **Consider key-based auth** for production systems instead of passwords

## Development

```bash
# Run tests
cargo test

# Build for release
cargo build --release

# Run with debug output
RUST_LOG=debug cargo run
```