# bgt-builder

Bitcoin Guix Tag Builder

## About

Perform automated [Guix builds](https://github.com/bitcoin/bitcoin/blob/master/contrib/guix/README.md) of Bitcoin Core when a new tag is detected via polling the GitHub API.

## Run

```bash
git clone https://github.com/bitcoin-dev-tools/bgt-builder.git
cd bgt-builder

# Install tool
cargo install --path .

# Show commands
bgt

# Run setup wizard
bgt run setup

# Build a specific tag
bgt run build v27.1

# Attest to non-codesigned outputs for a specific tag
bgt run attest v27.1

# Codesign outputs for a specific tag
bgt run codesign v27.1

# Run a watcher to auto-build new tags pushed to GH
# This will also attest, and watch for detached sigs, before codesigning
# hint: run this process in screen or tmux as it's not daemonised
bgt run watch

# Clean directories leaving cache intact
bgt run clean

# View currently configuration values
bgt run show-config

# Enable debug logging on any command
RUST_LOG=debug cargo run build v26.2
```

## Plans

- [x] implement Guix building
- [x] permit building a specified tag
- [x] enable signing
- [ ] add advanced GPG signing solutions (tbd)
- [ ] remove some dependencies
