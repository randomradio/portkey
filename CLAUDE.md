# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Portkey** is a Rust-based SSH credential manager with encrypted vault storage and a terminal UI (TUI). Users can store server credentials (name, host, port, username, password) protected by a master password using XSalsa20-Poly1305 (libsodium SecretBox) with Argon2id key derivation.

## Build & Test Commands

```bash
# Build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test test_name

# Run the binary
cargo run -- --help
./target/debug/portkey --help

# Install locally
cargo install --path .
```

## High-Level Architecture

The codebase is organized into clear modules with single responsibilities:

### Core Modules

- **`models.rs`**: Data structures (`Server`, `VaultData`) with serde serialization. `Server` contains credentials and metadata; `VaultData` is a container for servers with versioning.

- **`crypto.rs`**: Wrapper around `sodiumoxide` providing `MasterKey` for password-based key derivation (Argon2id) and encryption/decryption (SecretBox). Keys are zeroized on drop.

- **`vault.rs`**: `Vault` struct manages vault file I/O, locking/unlocking, and server CRUD operations. Vault files contain encrypted JSON data with metadata (salt, nonce, timestamps). Vault location: `$XDG_DATA_HOME/portkey/vault.dat`.

- **`cli.rs`**: Command definitions using `clap` derive API and `CliHandler` that dispatches to appropriate handlers. Interactive prompts use `inquire`. Supports both password-protected and unencrypted vaults.

- **`tui.rs`**: Full-screen TUI using `ratatui` with fuzzy search (`fuzzy-matcher`). Main UI modes: Browse, Filter, Add (form), Edit (form), ConfirmDelete, Message. Handles its own terminal cleanup and reinitialization when spawning SSH.

- **`ssh.rs`**: Spawns SSH connection using `sshpass` for password auth. Password passed via `SSHPASS` env var to avoid process args. Checks for `sshpass` availability and provides helpful install instructions.

- **`debug.rs`**: Diagnostic command showing vault path, existence, file size, permissions, and readability.

### Important Patterns

1. **Vault State**: Vault starts locked. Operations like `add_server`, `list_servers` require unlocking first via `unlock()` which takes an optional password. Unlocked vaults have decrypted data in memory.

2. **Terminal Handling in TUI**: When connecting to SSH, the terminal is fully cleaned up (exit raw mode, leave alternate screen), SSH inherits stdio, then terminal is rebuilt from scratch. This is necessary because SSH takes over the terminal.

3. **Tmux Compatibility**: The TUI detects `$TMUX` environment variable and skips `EnableMouseCapture` when inside tmux, as it can interfere with tmux's own mouse handling.

4. **Encryption Optional**: Vaults can be created with or without password protection. The vault file format is the same either way; `ciphertext` field contains plaintext when no password is set.

5. **File Permissions**: Vault files are created with mode 0o600 (owner read/write only) via Unix permissions.

6. **Async Runtime**: Uses `tokio` for async runtime, though most operations are actually synchronous. The main entry point is `#[tokio::main]`.

### Entry Point

`main.rs` initializes `sodiumoxide`, checks for `debug` command, then delegates to `CliHandler::run()`.

### Testing

Integration tests are in `tests/integration_test.rs`. Tests use `tempfile` for isolated test environments.
