#!/usr/bin/env bash
set -euo pipefail

REPOSITORY="keton-id/probelm"
PRIMARY_NAME="probelm"
COMPATIBILITY_NAME="mtest"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${PROBELM_VERSION:-latest}"

info() { printf '\033[34m==>\033[0m %s\n' "$1"; }
ok() { printf '\033[32mOK\033[0m  %s\n' "$1"; }
warn() { printf '\033[33m!\033[0m   %s\n' "$1"; }
error() { printf '\033[31mERROR\033[0m %s\n' "$1" >&2; }

UNINSTALL=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --uninstall|-u) UNINSTALL=true; shift ;;
        --prefix) INSTALL_DIR="${2:?--prefix requires a directory}"; shift 2 ;;
        --prefix=*) INSTALL_DIR="${1#*=}"; shift ;;
        *) error "Unknown option: $1"; exit 2 ;;
    esac
done

if [[ "$UNINSTALL" == true ]]; then
    rm -f "$INSTALL_DIR/$PRIMARY_NAME" "$INSTALL_DIR/$COMPATIBILITY_NAME"
    ok "Removed probelm from $INSTALL_DIR"
    exit 0
fi

case "$(uname -s):$(uname -m)" in
    Darwin:arm64) TARGET="macos-aarch64" ;;
    Darwin:x86_64) TARGET="macos-x86_64" ;;
    Linux:aarch64|Linux:arm64) TARGET="linux-aarch64" ;;
    Linux:x86_64|Linux:amd64) TARGET="linux-x86_64" ;;
    *) error "Unsupported platform: $(uname -s)/$(uname -m)"; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd || pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." 2>/dev/null && pwd || true)"
LOCAL_BINARY="$PROJECT_ROOT/target/release/$PRIMARY_NAME"
TEMP_ROOT=""
BINARY=""

cleanup() {
    if [[ -n "$TEMP_ROOT" ]]; then rm -rf "$TEMP_ROOT"; fi
}
trap cleanup EXIT

if [[ -f "$PROJECT_ROOT/Cargo.toml" && -x "$LOCAL_BINARY" ]]; then
    BINARY="$LOCAL_BINARY"
    info "Installing the existing local release binary"
elif [[ -f "$PROJECT_ROOT/Cargo.toml" && -z "${CI:-}" ]]; then
    command -v cargo >/dev/null 2>&1 || { error "cargo is required to build from this checkout"; exit 1; }
    info "Building probelm from this checkout"
    cargo build --release --locked --manifest-path "$PROJECT_ROOT/Cargo.toml"
    BINARY="$LOCAL_BINARY"
else
    command -v curl >/dev/null 2>&1 || { error "curl is required to download probelm"; exit 1; }
    if [[ "$VERSION" == "latest" ]]; then
        VERSION="$(curl -fsSL -H 'User-Agent: probelm-installer' "https://api.github.com/repos/$REPOSITORY/releases/latest" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
        [[ -n "$VERSION" ]] || { error "Could not resolve the latest release"; exit 1; }
    fi
    [[ "$VERSION" == v* ]] || VERSION="v$VERSION"
    ARCHIVE="probelm-$TARGET.tar.gz"
    TEMP_ROOT="$(mktemp -d)"
    info "Downloading $ARCHIVE ($VERSION)"
    curl -fsSL --retry 3 -o "$TEMP_ROOT/$ARCHIVE" "https://github.com/$REPOSITORY/releases/download/$VERSION/$ARCHIVE"
    curl -fsSL --retry 3 -o "$TEMP_ROOT/$ARCHIVE.sha256" "https://github.com/$REPOSITORY/releases/download/$VERSION/$ARCHIVE.sha256"
    EXPECTED="$(awk '{print tolower($1)}' "$TEMP_ROOT/$ARCHIVE.sha256")"
    if command -v sha256sum >/dev/null 2>&1; then
        ACTUAL="$(sha256sum "$TEMP_ROOT/$ARCHIVE" | awk '{print tolower($1)}')"
    else
        ACTUAL="$(shasum -a 256 "$TEMP_ROOT/$ARCHIVE" | awk '{print tolower($1)}')"
    fi
    [[ "$EXPECTED" =~ ^[0-9a-f]{64}$ && "$EXPECTED" == "$ACTUAL" ]] || { error "SHA-256 verification failed for $ARCHIVE"; exit 1; }
    tar -xzf "$TEMP_ROOT/$ARCHIVE" -C "$TEMP_ROOT"
    BINARY="$TEMP_ROOT/$PRIMARY_NAME"
fi

[[ -f "$BINARY" ]] || { error "Release does not contain $PRIMARY_NAME"; exit 1; }
mkdir -p "$INSTALL_DIR"
install -m 755 "$BINARY" "$INSTALL_DIR/$PRIMARY_NAME"
ln -sf "$PRIMARY_NAME" "$INSTALL_DIR/$COMPATIBILITY_NAME"
ok "Installed probelm to $INSTALL_DIR/$PRIMARY_NAME"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) warn "$INSTALL_DIR is not on PATH. Add it before running probelm." ;;
esac

if "$INSTALL_DIR/$PRIMARY_NAME" --version >/dev/null 2>&1; then
    ok "$($INSTALL_DIR/$PRIMARY_NAME --version)"
fi
