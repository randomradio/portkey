# 🪄 Portkey - Magical SSH Portal Manager

> *"I solemnly swear I am up to no good... with server management"*

Like the Marauder's Map from Harry Potter, **Portkey** is your magical gateway to instantly teleport between servers without memorizing passwords. One master spell (password) unlocks a hidden world of secure SSH connections, transforming the mundane task of server management into an enchanting experience.

## ✨ What Makes Portkey Magical?

Imagine having a magical map that reveals all your servers at once, and with a simple incantation, **whoosh** — you're instantly connected. Portkey is that map, but for SSH connections. Instead of fumbling with cryptic incantations (passwords), you speak one master spell and gain access to your entire server kingdom.

**🧙‍♂️ The Magic Behind the Spell:**
- **Alohomora for SSH**: One master password unlocks all your server credentials
- **Marauder's Map for DevOps**: Visualize all your servers in one enchanted interface
- **Apparition for SSH**: Instantly teleport to any server with a single command
- **Fidelius Charm**: Military-grade encryption keeps your secrets safe

## 🪄 How the Magic Works

### 1. **Create Your Magical Vault** ✨
```bash
./portkey init  # Cast the creation spell
# Enter your master password - this becomes your magical key
```

### 2. **Populate Your Magical Map** 🗺️
```bash
./portkey add    # Reveal new servers to your map
# Name: production-web
# Host: 192.168.1.100
# Password: ••••••••
# Description: The castle's main gate
```

### 3. **Navigate Your Wizarding World** 🧭
```bash
./portkey quick  # Marauder's Map mode
# Select your destination: prod-web (192.168.1.100)
# 🪄 *Poof* - You're connected!
```

### 4. **Search Like a True Wizard** 🔍
```bash
./portkey search web  # Find all web servers
./portkey search prod  # Find all production servers
```

## 🔮 Magical Features

| Spell | Effect |
|-------|--------|
| `init` | **Creation Charm** - Forge your magical vault |
| `add` | **Revelation Spell** - Add new servers to your map |
| `quick` | **Apparition** - Instant teleportation to any server |
| `list` | **Marauder's Map** - View all accessible servers |
| `search` | **Point Me** - Find servers by keyword |
| `remove` | **Obliviate** - Banish servers from your map |

## 🛡️ Protective Enchantments

- **🔐 Chamber of Secrets**: AES-256-GCM encryption with unique salts
- **🗝️ Master Key**: PBKDF2 key derivation with Argon2id13
- **🧹 Memory Charms**: Zeroizes sensitive data from memory
- **🚪 Restricted Access**: File permissions locked to owner only (600)
- **⚡ Unbreakable Vow**: Rust's memory safety prevents dark magic

## 🚀 Quick Start - Become a Wizard in 60 Seconds

```bash
# ⚡ Install the magical toolkit
./install.sh

# 🪄 Create your vault (choose your master spell)
./portkey init

# 🗺️ Add your first server
./portkey add

# ✨ Connect instantly
./portkey quick
```

## 🌟 Magical Use Cases

### 🏰 **The Digital Castle Manager**
You're the keeper of a vast digital castle with dozens of towers (servers). Instead of memorizing the secret password to each tower, you have one master key that opens them all. Walk through your kingdom with ease!

### 🧙‍♂️ **The DevOps Sorcerer**
Juggle multiple environments like a true wizard. Production, staging, development - all accessible with a flick of your terminal wand. No more "Accio production server!" followed by frantic password hunting.

### 🕵️ **The Infrastructure Detective**
Search your entire infrastructure like you're using the Marauder's Map. "I solemnly swear I need to find all web servers" - and there they are, revealed in all their glory.

## 📜 Spell Book (Command Reference)

```bash
# Basic Spells
./portkey init          # Create your magical vault
./portkey add           # Add a new server to your map
./portkey list          # View all enchanted servers
./portkey quick         # Interactive teleportation
./portkey connect web01 # Direct teleport to specific server
./portkey search web    # Find servers by magic keyword
./portkey remove web01  # Remove server from your map

# Advanced Sorcery
./portkey debug         # Reveal vault diagnostics
```

## 🧪 Magical Architecture

```
┌─────────────────────────────────────────┐
│          Portkey Spell Book             │
├─────────────────────────────────────────┤
│  🔐 Crypto: AES-256-GCM + Argon2id13   │
│  🗃️ Storage: Encrypted JSON vault       │
│  🔍 Search: Fuzzy matching across all   │
│  🔗 SSH: Password-based authentication  │
│  🎨 CLI: Enchanted interactive prompts  │
└─────────────────────────────────────────┘
```

## 🪄 Behind the Magic

When you type a password, Portkey performs an ancient ritual:
1. **Derives a magical key** from your password using PBKDF2
2. **Unseals the vault** using AES-256-GCM decryption
3. **Reveals your servers** in an enchanted interface
4. **Teleports you instantly** via SSH with stored credentials

## 🎭 Role-Playing Guide

| Your Role | Portkey's Magic |
|-----------|-----------------|
| **System Admin** | One ring to rule them all |
| **DevOps Wizard** | Teleportation mastery |
| **Security Mage** | Fort Knox for passwords |
| **Productivity Sorcerer** | Zero-friction connections |

## 🌈 From Muggles to Wizards

**Before Portkey:**
```
❌ "What's the password for prod-web-03 again?"
❌ Searching through spreadsheets of credentials
❌ Copy-pasting passwords like a Muggle
❌ Managing 47 different SSH keys
```

**After Portkey:**
```
✅ One password, unlimited access
✅ Magical server discovery
✅ Instant teleportation
✅ Secure, encrypted storage
✅ "Accio server!" actually works
```

---

<p align="center">
  <i>"Mischief managed." - Every DevOps wizard using Portkey</i>
</p>

## 🪄 Ready to Start Your Magical Journey?

```bash
git clone <repository>
cd portkey
./install.sh
./portkey init
```

*May your connections be swift and your servers ever responsive.*
