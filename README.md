# bgt-builder

Bitcoin Guix Tag Builder

## About

Perform automated [Guix builds](https://github.com/bitcoin/bitcoin/blob/master/contrib/guix/README.md) of Bitcoin Core when a new tag is detected via polling the GitHub API.

## Run

```bash
git clone https://github.com/bitcoin-dev-tools/bgt-builder.git
cd bgt-builder

# Run setup wizard
cargo run init

# Build a specific tag
cargo run tag v27.1

# Enable debug logging
RUST_LOG=debug cargo run tag v26.2

# Run a watcher to auto-build new tags pushed to GH
cargo run watcher
```

## Plans

- [x] implement Guix building
- [x] permit building a specified tag
- [ ] enable signing
- [ ] add advanced GPG signing solutions (tbd)
