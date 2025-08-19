#!/bin/bash

# Portkey SSH Manager - Installation Script
# This script installs dependencies and sets up portkey

set -e

echo "🔧 Portkey SSH Manager Installation"
echo "=================================="

# Detect OS
OS="$(uname -s)"
case "${OS}" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    CYGWIN*)    MACHINE=Cygwin;;
    MINGW*)     MACHINE=MinGw;;
    *)          MACHINE="UNKNOWN:${OS}"
esac

echo "Detected OS: ${MACHINE}"

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Install Rust if not present
if ! command_exists cargo; then
    echo "📦 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Install sshpass based on OS
echo "🔍 Checking for sshpass..."
if ! command_exists sshpass; then
    echo "📥 Installing sshpass..."
    case "${MACHINE}" in
        Mac)
            if command_exists brew; then
                brew install hudochenkov/sshpass/sshpass
            else
                echo "❌ Homebrew not found. Please install Homebrew first:"
                echo "   /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
                exit 1
            fi
            ;;
        Linux)
            if command_exists apt-get; then
                sudo apt-get update && sudo apt-get install -y sshpass
            elif command_exists yum; then
                sudo yum install -y sshpass
            elif command_exists dnf; then
                sudo dnf install -y sshpass
            elif command_exists pacman; then
                sudo pacman -S sshpass
            else
                echo "❌ Package manager not supported. Please install sshpass manually."
                exit 1
            fi
            ;;
        *)
            echo "❌ Unsupported OS. Please install sshpass manually."
            exit 1
            ;;
    esac
else
    echo "✅ sshpass is already installed"
fi

# Build the project
echo "🔨 Building portkey..."
cargo build --release

# Create symlink in PATH
if [ -f "./target/release/portkey" ]; then
    echo "✅ Portkey built successfully!"
    echo ""
    echo "📍 Installation complete!"
    echo "======================="
    echo "Binary location: ./target/release/portkey"
    echo ""
    echo "🚀 Quick start:"
    echo "  ./target/release/portkey init"
    echo "  ./target/release/portkey add"
    echo "  ./target/release/portkey quick"
    echo ""
    echo "💡 To make it globally available:"
    echo "  sudo cp ./target/release/portkey /usr/local/bin/"
    echo "  # OR add to your PATH:"
    echo "  export PATH=\"\$PATH:$(pwd)/target/release\""
else
    echo "❌ Build failed"
    exit 1
fi