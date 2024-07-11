# bgt-builder

Bitcoin Guix Tag Builder

## About

Perform automated [Guix builds](https://github.com/bitcoin/bitcoin/blob/master/contrib/guix/README.md) of Bitcoin Core when a new tag is detected via polling the GitHub API.

## Run

```bash
git clone https://github.com/bitcoin-dev-tools/bgt-builder.git
cd bgt-builder
# to run a build for v27.1 only
BITCOIN_SOURCE_DIR="$HOME/src/bitcoin" SIGNER=<your-pgp-key-name> cargo run -- tag v27.1
```

## Plans

- [x] implement Guix building
- [ ] permit building a specified tag
- [ ] add advanced GPG signing solutions (tbd)
