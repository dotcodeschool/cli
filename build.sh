#!/bin/bash

set -e

VERSION="v0.1.0"
PROJECT_NAME="dotcodeschool-cli"

# Set OpenSSL directory for x86_64
export OPENSSL_DIR=$(arch -x86_64 brew --prefix openssl@3)
export OPENSSL_INCLUDE_DIR="$OPENSSL_DIR/include"
export OPENSSL_LIB_DIR="$OPENSSL_DIR/lib"
export MACOSX_DEPLOYMENT_TARGET=10.12

# macOS builds
echo "Building for macOS (x86_64)..."
RUSTFLAGS="-C target-cpu=x86-64" cargo build --release --target x86_64-apple-darwin

echo "Building for macOS (aarch64)..."
cargo build --release --target aarch64-apple-darwin

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Docker is not running. Please start Docker and try again."
    exit 1
fi

# Linux builds (using cross)
echo "Building for Linux (x86_64)..."
CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --release --target x86_64-unknown-linux-gnu

echo "Building for Linux (aarch64)..."
CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --release --target aarch64-unknown-linux-gnu

# Create release archives
mkdir -p releases

# Function to create tar.gz and check if binary exists
create_tarball() {
    local target=$1
    local arch=$2
    local os=$3
    if [ -f "target/${target}/release/${PROJECT_NAME}" ]; then
        tar -czf "releases/${VERSION}_${os}_${arch}.tar.gz" -C "target/${target}/release" "${PROJECT_NAME}"
        echo "Created ${os}_${arch} tarball"
    else
        echo "Warning: Binary for ${os}_${arch} not found"
    fi
}

# Create tarballs
create_tarball "x86_64-apple-darwin" "amd64" "darwin"
create_tarball "aarch64-apple-darwin" "arm64" "darwin"
create_tarball "x86_64-unknown-linux-gnu" "amd64" "linux"
create_tarball "aarch64-unknown-linux-gnu" "arm64" "linux"

# Generate SHA256 checksums
cd releases
shasum -a 256 *.tar.gz > checksums.txt
cd ..

echo "Build process completed"
