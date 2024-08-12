# bgt-builder

Bitcoin Guix Tag Builder

## About

bgt-builder is a tool to perform automated [Guix builds](https://github.com/bitcoin/bitcoin/blob/master/contrib/guix/README.md) of Bitcoin Core when a new tag is detected via polling the GitHub API. It can build, attest, and codesign Bitcoin Core releases.

## Installation

To install bgt-builder, you need to have Rust and Cargo installed on your system. Then, you can install it from the source:

```bash
git clone https://github.com/bitcoin-dev-tools/bgt-builder.git
cd bgt-builder
cargo install --path .
```

## Usage

After installation, you can use the `bgt` command to interact with the tool. Here are the available commands:

### Setup

Run the setup wizard to configure bgt:

```bash
bgt setup
```

This will guide you through setting up your GPG key, signer name, and other necessary configurations.

### Build

Build a specific tag of Bitcoin Core:

```bash
bgt build <tag>
```

Replace `<tag>` with the specific version tag you want to build, e.g., `v27.1`.

### Attest

Attest to non-codesigned build outputs:

```bash
bgt attest <tag>
```

### Codesign

Attach codesignatures to existing non-codesigned outputs and attest:

```bash
bgt codesign <tag>
```

### Watch

Run a continuous watcher to monitor for new tags and automatically build them, optionally as a background daemon:

```bash
bgt watch start <--daemon>
```

Stop a background watcher daemon

```bash
bgt watch stop
```

This command will poll the GitHub API for new tags and automatically build, attest, and codesign new releases.

### Clean

Clean up Guix build directories while leaving caches intact:

```bash
bgt clean
```

### Show Config

View the current configuration settings:

```bash
bgt show-config
```

## Additional Options

- `--multi-package`: Use `JOBS=1 ADDITIONAL_GUIX_COMMON_FLAGS='--max-jobs=8'` for building. This can be added to any command.

## Logging

bgt uses environment variables for logging configuration. You can set the `RUST_LOG` environment variable to control the log level. For example:

```bash
RUST_LOG=debug bgt build v27.1
```

This will run the build command with debug-level logging.

## Contributing

Contributions to bgt-builder are welcome! Please feel free to submit issues and pull requests on our GitHub repository.

## License

See [license](https://github.com/bitcoin-dev-tools/bgt-builder/LICENSE).

## Plans

- [x] implement Guix building
- [x] permit building a specified tag
- [x] enable signing
- [ ] add advanced GPG signing solutions (tbd)
- [ ] remove some dependencies
