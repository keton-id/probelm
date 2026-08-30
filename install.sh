#!/usr/bin/env bash
set -e

# ==============================================================================
# Installer for probelm / mtest (macOS & Linux)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="mtest"
ALIAS_NAME="probelm"

# Default install target directory
DEFAULT_INSTALL_DIR="$HOME/.local/bin"
INSTALL_DIR="${INSTALL_DIR:-$DEFAULT_INSTALL_DIR}"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}==>${NC} $1"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}!${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} $1" >&2
}

# ------------------------------------------------------------------------------
# Handle --uninstall flag
# ------------------------------------------------------------------------------
if [ "$1" == "--uninstall" ] || [ "$1" == "-u" ]; then
    log_info "Uninstalling $BIN_NAME and $ALIAS_NAME from $INSTALL_DIR..."
    rm -f "$INSTALL_DIR/$BIN_NAME"
    rm -f "$INSTALL_DIR/$ALIAS_NAME"
    log_success "Uninstalled successfully."
    exit 0
fi

# ------------------------------------------------------------------------------
# Parse custom directory argument if provided
# ------------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix=*)
            INSTALL_DIR="${1#*=}"
            shift
            ;;
        --prefix)
            INSTALL_DIR="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

echo -e "${BLUE}=======================================${NC}"
echo -e "${BLUE}       probelm / mtest Installer       ${NC}"
echo -e "${BLUE}=======================================${NC}"

# Check OS
OS="$(uname -s)"
ARCH="$(uname -m)"
log_info "Detected OS: $OS ($ARCH)"

# Ensure cargo is available if build is needed
RELEASE_BIN="$SCRIPT_DIR/target/release/$BIN_NAME"

if [ ! -f "$RELEASE_BIN" ]; then
    log_info "Release binary not found. Building with cargo..."
    if ! command -v cargo &> /dev/null; then
        log_error "cargo is not installed or not in PATH. Please install Rust from https://rustup.rs."
        exit 1
    fi
    (cd "$SCRIPT_DIR" && cargo build --release)
else
    log_info "Found existing built binary at target/release/$BIN_NAME"
    # If source files are newer than binary, rebuild
    NEWEST_SRC=$(find "$SCRIPT_DIR/src" "$SCRIPT_DIR/Cargo.toml" -type f -exec stat -f "%m %N" {} + 2>/dev/null | sort -nr | head -n1 | cut -d' ' -f1 || echo 0)
    BIN_MTIME=$(stat -f "%m" "$RELEASE_BIN" 2>/dev/null || echo 0)
    if [ "$NEWEST_SRC" -gt "$BIN_MTIME" ]; then
        log_info "Source code changed since last build. Rebuilding..."
        (cd "$SCRIPT_DIR" && cargo build --release)
    fi
fi

# Prepare target directory
mkdir -p "$INSTALL_DIR"

# Install binary
log_info "Installing binary to $INSTALL_DIR/$BIN_NAME..."
cp "$RELEASE_BIN" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

# Create symlink for probelm alias
ln -sf "$INSTALL_DIR/$BIN_NAME" "$INSTALL_DIR/$ALIAS_NAME"

log_success "Installed $BIN_NAME and $ALIAS_NAME to $INSTALL_DIR"

# Check if INSTALL_DIR is in PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        # Already in PATH
        ;;
    *)
        log_warn "$INSTALL_DIR is not in your current PATH."
        SHELL_NAME="$(basename "${SHELL:-/bin/zsh}")"
        RC_FILE="$HOME/.zshrc"
        if [ "$SHELL_NAME" = "bash" ]; then
            RC_FILE="$HOME/.bashrc"
        fi
        echo ""
        echo "To add it to your PATH, add the following line to $RC_FILE:"
        echo -e "  ${YELLOW}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
        echo ""
        ;;
esac

# Verify execution
if command -v "$BIN_NAME" &> /dev/null; then
    VER="$("$BIN_NAME" --version)"
    log_success "Verification passed: $VER is ready to use!"
elif [ -x "$INSTALL_DIR/$BIN_NAME" ]; then
    VER="$("$INSTALL_DIR/$BIN_NAME" --version)"
    log_success "Verification passed: $VER ($INSTALL_DIR/$BIN_NAME)"
fi

echo ""
echo -e "${GREEN}Done!${NC} You can now run:"
echo "  mtest --help"
echo "  probelm --help"
