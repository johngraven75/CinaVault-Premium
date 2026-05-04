#!/bin/bash
set -e

# ========================================================
# CinaVault Premium - Automated Build Script (Linux/macOS)
# ========================================================

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILDS_DIR="$HOME/Desktop/CinaVault Builds"
OS_TYPE="$(uname -s)"

# Helper functions
log_ok() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_step() {
    echo ""
    echo -e "${BLUE}$1${NC}"
}

separator() {
    echo "=========================================================="
}

# Main build function
main() {
    clear
    echo ""
    separator
    echo "  CinaVault Premium - Automated Build"
    echo "  This script installs ALL prerequisites and builds"
    echo "  the full application for you."
    separator
    echo ""

    cd "$SCRIPT_DIR"
    log_info "Running from: $(pwd)"
    echo ""

    # ===============================================
    # STEP 1: Check/Install Node.js
    # ===============================================
    log_step "[1/6] Checking Node.js..."
    
    if ! command -v node &> /dev/null; then
        log_info "Node.js not found. Installing..."
        
        if [[ "$OS_TYPE" == "Darwin" ]]; then
            # macOS
            if ! command -v brew &> /dev/null; then
                log_error "Homebrew not found. Please install from https://brew.sh"
                exit 1
            fi
            brew install node
        else
            # Linux
            if command -v apt-get &> /dev/null; then
                sudo apt-get update
                sudo apt-get install -y nodejs npm
            elif command -v dnf &> /dev/null; then
                sudo dnf install -y nodejs npm
            elif command -v pacman &> /dev/null; then
                sudo pacman -S --noconfirm nodejs npm
            else
                log_error "Could not detect package manager. Please install Node.js manually from https://nodejs.org/"
                exit 1
            fi
        fi
        log_ok "Node.js installed"
    else
        NODE_VERSION=$(node --version)
        log_ok "Node.js $NODE_VERSION found"
    fi

    # ===============================================
    # STEP 2: Check/Install Rust
    # ===============================================
    log_step "[2/6] Checking Rust..."
    
    if ! command -v rustc &> /dev/null; then
        log_info "Rust not found. Installing via rustup..."
        
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source $HOME/.cargo/env
        
        log_ok "Rust installed"
    else
        RUST_VERSION=$(rustc --version)
        log_ok "$RUST_VERSION found"
    fi

    # Verify both tools are accessible
    echo ""
    log_step "[VERIFY] Final prerequisite check..."
    
    if ! command -v node &> /dev/null; then
        log_error "Node.js still not in PATH after install"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo still not in PATH after install"
        exit 1
    fi
    
    log_ok "All prerequisites verified!"

    # ===============================================
    # STEP 3: Install frontend dependencies
    # ===============================================
    log_step "[3/6] Installing frontend dependencies (npm install)..."
    
    if ! npm install; then
        log_warn "npm install failed. Clearing cache and retrying..."
        npm cache clean --force
        rm -rf node_modules package-lock.json
        
        if ! npm install; then
            log_error "npm install failed after retry"
            log_error "Check your internet connection and try again"
            exit 1
        fi
    fi
    
    log_ok "Frontend dependencies installed"

    # ===============================================
    # STEP 4: Install Tauri CLI
    # ===============================================
    log_step "[4/6] Installing Tauri CLI (this may take a few minutes first time)..."
    
    cargo install tauri-cli --version "^2.0" 2>/dev/null || true
    
    log_ok "Tauri CLI ready"

    # ===============================================
    # STEP 5: Build the full app
    # ===============================================
    log_step "[5/6] Building CinaVault Premium..."
    echo "      First build takes 5-15 minutes. Subsequent builds are faster."
    echo "      Please be patient..."
    echo ""
    
    if ! npx tauri build; then
        log_error "Build failed"
        echo ""
        echo "Common fixes:"
        echo "  1. Check for compilation errors above"
        echo "  2. Make sure you have all build tools installed"
        echo "  3. On macOS: xcode-select --install"
        echo "  4. On Linux: sudo apt-get install build-essential libssl-dev"
        echo ""
        exit 1
    fi

    # ===============================================
    # STEP 6: Copy installers to Desktop
    # ===============================================
    log_step "[6/6] Copying builds to Desktop..."
    
    mkdir -p "$BUILDS_DIR"
    
    FOUND=0
    
    # Copy AppImage (Linux)
    if ls src-tauri/target/release/bundle/appimage/*.AppImage 1> /dev/null 2>&1; then
        for file in src-tauri/target/release/bundle/appimage/*.AppImage; do
            cp "$file" "$BUILDS_DIR/"
            log_info "[BUILD] $(basename "$file")"
            FOUND=1
        done
    fi
    
    # Copy DMG (macOS)
    if ls src-tauri/target/release/bundle/dmg/*.dmg 1> /dev/null 2>&1; then
        for file in src-tauri/target/release/bundle/dmg/*.dmg; do
            cp "$file" "$BUILDS_DIR/"
            log_info "[BUILD] $(basename "$file")"
            FOUND=1
        done
    fi
    
    # Copy .deb (Linux)
    if ls src-tauri/target/release/bundle/deb/*.deb 1> /dev/null 2>&1; then
        for file in src-tauri/target/release/bundle/deb/*.deb; do
            cp "$file" "$BUILDS_DIR/"
            log_info "[BUILD] $(basename "$file")"
            FOUND=1
        done
    fi
    
    if [ $FOUND -eq 0 ]; then
        log_warn "No build files found in expected location"
        log_info "Check src-tauri/target/release/bundle/ manually"
    else
        echo ""
        separator
        echo "  BUILD COMPLETE!"
        echo "  Your builds are in: $BUILDS_DIR"
        separator
        
        # Open the builds directory
        if [[ "$OS_TYPE" == "Darwin" ]]; then
            open "$BUILDS_DIR"
        elif command -v xdg-open &> /dev/null; then
            xdg-open "$BUILDS_DIR"
        fi
    fi
    
    echo ""
}

# Run main function
main "$@"
