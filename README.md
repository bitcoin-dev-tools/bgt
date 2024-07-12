# bgt-builder

Bitcoin Guix Tag Builder

## About

Perform automated [Guix builds](https://github.com/bitcoin/bitcoin/blob/master/contrib/guix/README.md) of Bitcoin Core when a new tag is detected via polling the GitHub API.

## Run

```bash
git clone https://github.com/bitcoin-dev-tools/bgt-builder.git
cd bgt-builder

# Run setup wizard
cargo run setup

# Build a specific tag
cargo run build v27.1

# Attest to non-codesigned outputs for a specific tag
cargo run attest v27.1

# Codesign outputs for a specific tag
cargo run codesign v27.1

# Run a watcher to auto-build new tags pushed to GH
# This will also attest, and watch for detached sigs, before codesigning
cargo run watch

# Clean directories leaving cache intact
cargo run clean

# View currently configuration values
cargo run show-config

# Enable debug logging on any command
RUST_LOG=debug cargo run build v26.2
```

## Plans

- [x] implement Guix building
- [x] permit building a specified tag
- [x] enable signing
- [ ] add advanced GPG signing solutions (tbd)
- [ ] remove some dependencies
