#!/bin/bash
#
# SlowOS Build System
# ===================
#
# Builds SlowOS for development (native) or production (Raspberry Pi).
#
# Usage:
#   ./build.sh dev        Build for local development (macOS/Linux)
#   ./build.sh release    Build optimized for local machine
#   ./build.sh pi         Cross-compile for Raspberry Pi (requires toolchain)
#   ./build.sh image      Build complete Buildroot SD card image
#   ./build.sh clean      Clean all build artifacts
#   ./build.sh run        Build and run the desktop shell locally
#
# Requirements:
#   - Rust 1.70+ (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh)
#   - For 'pi': aarch64-unknown-linux-gnu target
#   - For 'image': Buildroot (auto-downloaded)
#

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[slowos]${NC} $1"; }
warn() { echo -e "${YELLOW}[slowos]${NC} $1"; }
error() { echo -e "${RED}[slowos]${NC} $1"; }

# All binary targets
APPS=(
    slowdesktop
    slowwrite
    slowpaint
    slowreader
    slownotes
    slowchess
    slowfiles
    slowmusic
    slowclock
    trash
    slowterm
    slowview
    settings
    slowcalc
    slowmidi
    slowbreath
    slowsolitaire
    slowdesign
    credits
)

check_rust() {
    if ! command -v cargo &> /dev/null; then
        error "Rust not found. Install with:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
}

build_dev() {
    check_rust
    info "building SlowOS (debug)..."
    cargo build --workspace
    info "done. run with: cargo run -p slowdesktop"
}

build_release() {
    check_rust
    info "building SlowOS (release)..."
    cargo build --release --workspace
    
    # Report binary sizes
    info "binary sizes:"
    for app in "${APPS[@]}"; do
        if [ -f "target/release/$app" ]; then
            size=$(du -h "target/release/$app" | cut -f1)
            printf "  %-16s %s\n" "$app" "$size"
        fi
    done
    
    info "done. all binaries in target/release/"
}

build_pi() {
    check_rust
    
    # Check for cross-compilation target
    if ! rustup target list --installed | grep -q aarch64-unknown-linux-gnu; then
        warn "adding aarch64 target..."
        rustup target add aarch64-unknown-linux-gnu
    fi
    
    # Check for linker
    if ! command -v aarch64-linux-gnu-gcc &> /dev/null; then
        error "cross-compiler not found. install with:"
        echo "  macOS:  brew install aarch64-elf-gcc"
        echo "  Ubuntu: sudo apt install gcc-aarch64-linux-gnu"
        exit 1
    fi
    
    info "cross-compiling for Raspberry Pi (aarch64)..."
    
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    cargo build --release --target aarch64-unknown-linux-gnu --workspace
    
    info "done. binaries in target/aarch64-unknown-linux-gnu/release/"
}

build_image() {
    info "building complete SD card image..."
    
    BUILDROOT_DIR="$SCRIPT_DIR/buildroot/.buildroot"
    
    if [ ! -d "$BUILDROOT_DIR" ]; then
        info "downloading Buildroot..."
        git clone --depth 1 https://github.com/buildroot/buildroot.git "$BUILDROOT_DIR"
    fi
    
    cd "$BUILDROOT_DIR"
    make BR2_EXTERNAL="$SCRIPT_DIR/buildroot" slowos_defconfig
    make -j$(nproc)
    
    info "SD card image ready at: $BUILDROOT_DIR/output/images/sdcard.img"
    info "flash with: dd if=$BUILDROOT_DIR/output/images/sdcard.img of=/dev/sdX bs=4M"
}

clean_build() {
    info "cleaning build artifacts..."
    cargo clean
    rm -rf buildroot/.buildroot/output
    info "done."
}

run_desktop() {
    check_rust
    info "building and launching SlowOS desktop..."
    cargo build --release --workspace
    info "starting slowdesktop..."
    ./target/release/slowdesktop
}

# Parse command
case "${1:-dev}" in
    dev|debug)
        build_dev
        ;;
    release)
        build_release
        ;;
    pi|rpi|arm)
        build_pi
        ;;
    image|sdcard)
        build_image
        ;;
    clean)
        clean_build
        ;;
    run)
        run_desktop
        ;;
    *)
        echo "SlowOS Build System"
        echo ""
        echo "Usage: $0 {dev|release|pi|image|clean|run}"
        echo ""
        echo "  dev      Build debug binaries for local development"
        echo "  release  Build optimized binaries for local machine"
        echo "  pi       Cross-compile for Raspberry Pi Zero 2 W"
        echo "  image    Build complete Buildroot SD card image"
        echo "  clean    Clean all build artifacts"
        echo "  run      Build release and launch the desktop shell"
        exit 1
        ;;
esac
