> [!CAUTION]
> **This repository has been archived and is no longer maintained.**
> 
> This CLI tool was part of an older architecture where students submitted tests via a backend service. Dot Code School now uses a fully static pipeline — courses are ingested from gitorial repos at build time and deployed via Vercel. There is no backend or test submission system to connect to.
> 
> See [dotcodeschool/frontend](https://github.com/dotcodeschool/frontend) for the current platform.

---

# DotCodeSchool CLI

A command-line interface tool for running tests and submitting assignments for DotCodeSchool courses.

## Overview

The DotCodeSchool CLI is a test runner that helps students work on DotCodeSchool course assignments. It provides functionality to run tests locally, list available tests, and submit your work to the DotCodeSchool platform.

## Features

- 🧪 **Test Runner**: Run course tests locally with various modes (single test, all tests, or staggered mode)
- 📋 **Test Listing**: View all available tests for your course
- 🚀 **Assignment Submission**: Submit your work directly to DotCodeSchool
- 🔄 **State Management**: Persistent state tracking using an embedded database
- 🌐 **Backend Integration**: Seamless integration with DotCodeSchool's backend API
- 🖥️ **Cross-Platform**: Support for macOS and Linux (both amd64 and arm64 architectures)

## Installation

### Quick Install (Recommended)

Use the installation script to automatically download and install the latest version:

```bash
curl -sSL https://raw.githubusercontent.com/dotcodeschool/cli/main/install.sh | sh
```

You can customize the installation:

```bash
# Install a specific version
DOTCODESCHOOL_CLI_VERSION=v0.1.0 curl -sSL https://raw.githubusercontent.com/dotcodeschool/cli/main/install.sh | sh

# Install to a custom location
INSTALL_DIR=/usr/local/bin curl -sSL https://raw.githubusercontent.com/dotcodeschool/cli/main/install.sh | sh
```

### Manual Installation

1. Download the appropriate binary for your platform from the [releases page](https://github.com/dotcodeschool/cli/releases)
2. Extract the archive:
   ```bash
   tar -xzf v0.1.0_<os>_<arch>.tar.gz
   ```
3. Move the binary to your PATH:
   ```bash
   sudo mv dotcodeschool /usr/local/bin/
   chmod +x /usr/local/bin/dotcodeschool
   ```

## Usage

### Running Tests

Run tests in staggered mode (default):

```bash
dotcodeschool test
```

Run all tests at once:

```bash
dotcodeschool test --all
```

Run a specific test by name:

```bash
dotcodeschool test <test-name>
```

Keep the test environment after running (useful for debugging):

```bash
dotcodeschool test --keep
```

### Listing Available Tests

View all tests available for your course:

```bash
dotcodeschool test --list
```

### Submitting Your Work

Submit the current commit to DotCodeSchool:

```bash
dotcodeschool submit
```

Create an empty commit and submit it:

```bash
dotcodeschool submit --empty
```

### Custom Database Location

Specify a custom database path:

```bash
dotcodeschool --db /path/to/database test
```

## Building from Source

### Prerequisites

- Rust 1.56 or later
- OpenSSL development headers (for building)
- For cross-platform builds: Docker and [cross](https://github.com/cross-rs/cross)

### Build Instructions

#### Development Build

```bash
cargo build
```

#### Release Build

```bash
cargo build --release
```

#### Production Build (Optimized)

```bash
cargo build --profile production
```

#### Cross-Platform Builds

To build for all supported platforms (requires Docker):

```bash
./build.sh
```

This will create release archives for:

- macOS (x86_64 and ARM64)
- Linux (x86_64 and ARM64)

The built binaries will be in the `releases/` directory with SHA256 checksums.

## Platform Support

| Platform | Architecture          | Status       |
| -------- | --------------------- | ------------ |
| macOS    | x86_64 (Intel)        | ✅ Supported |
| macOS    | ARM64 (Apple Silicon) | ✅ Supported |
| Linux    | x86_64                | ✅ Supported |
| Linux    | ARM64                 | ✅ Supported |

## Configuration

The CLI uses a local database (`.dcs.db` by default) to store state information. Logs are written to `.dcs.log` in the current directory.

## Development

### Project Structure

```
cli/
├── src/
│   ├── main.rs          # Entry point and CLI argument parsing
│   ├── monitor.rs       # State machine and workflow coordination
│   ├── runner/          # Test execution logic
│   ├── lister/          # Test listing functionality
│   ├── validator/       # Validation logic
│   ├── parsing/         # Configuration and data parsing
│   ├── db.rs            # Database operations
│   ├── models.rs        # Data models
│   ├── constants.rs     # Application constants
│   └── str_res.rs       # String resources
├── Cargo.toml           # Rust dependencies and configuration
├── build.sh             # Cross-platform build script
└── install.sh           # Installation script
```

### Dependencies

Key dependencies include:

- `clap` - CLI argument parsing
- `sled` - Embedded database
- `serde` - Serialization/deserialization
- `reqwest` - HTTP client for API communication
- `git2` - Git operations
- `colored` - Terminal output coloring
- `indicatif` - Progress bars

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Support

For issues, questions, or feature requests, please open an issue on GitHub.
