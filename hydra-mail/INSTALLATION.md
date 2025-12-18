# Hydra Mail Plugin Installation Guide

This guide explains how to install the Hydra Mail plugin for Claude Code.

## Prerequisites

- Claude Code CLI installed
- Linux or macOS (Unix Domain Sockets required)
- Rust toolchain (for building from source)

## Installation Methods

### Method 1: Local Installation (Recommended for Development)

This method installs the plugin from your local hydra-tools directory.

```bash
# Navigate to hydra-tools directory
cd /path/to/hydra-tools

# Build the binary
nix build
# OR
cargo build --release

# Install the plugin locally
claude plugins install --local .
```

**What happens:**
1. Claude Code registers the plugin from the current directory
2. The `skills/hydra-mail/SKILL.md` becomes available as a skill
3. The binary at `target/release/hydra-mail` is used for commands

### Method 2: Install from Git Repository

Once you've pushed to GitHub, others can install directly:

```bash
# Install from GitHub repository
claude plugins install --git https://github.com/0xPD33/hydra-tools.git

# Or if added to a marketplace
claude plugins install hydra-mail
```

### Method 3: Manual Skill Installation (Skills Only)

If you only want the skill without the full plugin:

```bash
# Copy the skill to personal skills directory
mkdir -p ~/.claude/skills/hydra-mail
cp skills/hydra-mail/SKILL.md ~/.claude/skills/hydra-mail/

# The skill will be available in Claude Code
```

## Verifying Installation

### 1. Check Plugin Status

```bash
# List installed plugins
claude plugins list

# You should see:
# hydra-mail@1.3.0 (local)
```

### 2. Check Available Skills

Start a Claude Code session and type:
```
/skills
```

You should see `hydra-mail` in the list.

### 3. Test the Binary

```bash
# Create a test project
mkdir test-hydra && cd test-hydra

# Initialize Hydra
hydra-mail init --daemon

# Check status
hydra-mail status

# Emit a test message
echo '{"test":"message"}' | hydra-mail emit --channel test --type delta

# Subscribe to see it
hydra-mail subscribe --channel test --once

# Cleanup
hydra-mail stop
cd .. && rm -rf test-hydra
```

## Using the Plugin

### In a New Project

```bash
# 1. Initialize Hydra in your project
cd your-project
hydra-mail init --daemon

# 2. The skill is automatically available in Claude Code
# 3. Start using hydra_emit and hydra_subscribe tools in prompts
```

### Skill Auto-Loading

The `hydra-mail` skill will automatically load when:
- Working in a directory with `.hydra/`
- Discussing multi-agent coordination
- Keywords trigger: pub/sub, messaging, agent communication

### Manual Skill Loading

You can explicitly load the skill:
```
Can you load the hydra-mail skill? I want to coordinate with other agents.
```

## Updating the Plugin

### For Local Installation

```bash
cd /path/to/hydra-tools

# Pull latest changes
git pull

# Rebuild
nix build  # or cargo build --release

# Reinstall
claude plugins install --local .
```

### For Git Installation

```bash
# Update to latest version
claude plugins update hydra-mail
```

## Uninstalling

```bash
# Remove the plugin
claude plugins uninstall hydra-mail

# Cleanup project .hydra directories manually if needed
find . -name ".hydra" -type d -exec rm -rf {} +
```

## Troubleshooting

### Plugin Not Found

If `claude plugins list` doesn't show hydra-mail:

```bash
# Verify the plugin structure
ls -la /path/to/hydra-tools/.claude-plugin/plugin.json
ls -la /path/to/hydra-tools/skills/hydra-mail/SKILL.md

# Reinstall
claude plugins install --local /path/to/hydra-tools
```

### Skill Not Loading

If the skill doesn't appear in Claude Code:

```bash
# Check skill file exists
cat ~/.claude/plugins/cache/hydra-mail/skills/hydra-mail/SKILL.md

# Or if manual install:
cat ~/.claude/skills/hydra-mail/SKILL.md

# Restart Claude Code session
```

### Binary Not Found

If `hydra-mail` command not found:

```bash
# Add to PATH (after building)
export PATH="$PATH:/path/to/hydra-tools/target/release"

# Or install to system location
sudo cp target/release/hydra-mail /usr/local/bin/

# Or use Nix
nix profile install .#hydra-mail
```

### Permission Errors

If you get permission errors with `.hydra`:

```bash
# Check permissions
ls -la .hydra/

# Should be:
# drwx------ (0700) for .hydra/
# srw------- (0600) for .hydra/hydra.sock

# Fix if needed
chmod 700 .hydra
chmod 600 .hydra/hydra.sock
```

## Advanced: Creating a Marketplace Entry

To make your plugin available in a marketplace:

1. **Create a marketplace repository** (or use existing)
2. **Add marketplace.json**:

```json
{
  "name": "0xPD33-marketplace",
  "owner": {
    "name": "0xPD33",
    "email": "maintainer@example.com"
  },
  "metadata": {
    "description": "0xPD33's Claude Code plugins and tools",
    "version": "1.0.0"
  },
  "plugins": [
    {
      "name": "hydra-mail",
      "source": {
        "source": "url",
        "url": "https://github.com/0xPD33/hydra-tools.git"
      },
      "description": "Multi-agent pub/sub messaging with TOON encoding",
      "version": "0.1.0",
      "keywords": ["multi-agent", "pub-sub", "messaging", "coordination"]
    }
  ]
}
```

3. **Users install your marketplace**:

```bash
claude marketplaces add 0xPD33-marketplace https://github.com/0xPD33/your-marketplace.git
claude plugins install hydra-mail
```

## Next Steps

After installation:

1. Read [.claude-plugin/README.md](.claude-plugin/README.md) for plugin usage guide
2. Check [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for design details
3. See [CLAUDE.md](CLAUDE.md) for project-specific guidance
4. Run integration tests: `cargo test --release`

## Support

- GitHub Issues: https://github.com/0xPD33/hydra-tools/issues
- Documentation: https://github.com/0xPD33/hydra-tools/tree/master/docs
