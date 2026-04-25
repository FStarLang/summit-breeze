#!/usr/bin/env bash
set -euo pipefail

# Package the VS Code extension with a bundled LSP binary.
#
# Usage:
#   ./package.sh                    # build for current platform
#   ./package.sh linux-x64          # explicit target
#   ./package.sh darwin-arm64
#   ./package.sh win32-x64

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
EXTENSION_DIR="$REPO_ROOT/editors/vscode"
SERVER_DIR="$EXTENSION_DIR/server"

# Determine VS Code target platform
if [ $# -ge 1 ]; then
    VSCODE_TARGET="$1"
else
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64)   VSCODE_TARGET="linux-x64" ;;
        Linux-aarch64)  VSCODE_TARGET="linux-arm64" ;;
        Darwin-x86_64)  VSCODE_TARGET="darwin-x64" ;;
        Darwin-arm64)   VSCODE_TARGET="darwin-arm64" ;;
        MINGW*|MSYS*)   VSCODE_TARGET="win32-x64" ;;
        *)              echo "Unknown platform: $(uname -s)-$(uname -m)"; exit 1 ;;
    esac
fi

# Map VS Code target to Rust target triple
case "$VSCODE_TARGET" in
    linux-x64)     RUST_TARGET="x86_64-unknown-linux-gnu" ;;
    linux-arm64)   RUST_TARGET="aarch64-unknown-linux-gnu" ;;
    darwin-x64)    RUST_TARGET="x86_64-apple-darwin" ;;
    darwin-arm64)  RUST_TARGET="aarch64-apple-darwin" ;;
    win32-x64)     RUST_TARGET="x86_64-pc-windows-msvc" ;;
    *)             echo "Unsupported target: $VSCODE_TARGET"; exit 1 ;;
esac

BINARY_NAME="summit-breeze-lsp"
if [[ "$VSCODE_TARGET" == win32-* ]]; then
    BINARY_NAME="summit-breeze-lsp.exe"
fi

echo "==> Building LSP server for $RUST_TARGET..."
cargo build --release --bin summit-breeze-lsp --target "$RUST_TARGET" 2>&1 \
    || cargo build --release --bin summit-breeze-lsp  # fallback: native build

echo "==> Preparing extension directory..."
rm -rf "$SERVER_DIR"
mkdir -p "$SERVER_DIR"

# Copy the binary
BINARY_SRC="$REPO_ROOT/target/$RUST_TARGET/release/$BINARY_NAME"
if [ ! -f "$BINARY_SRC" ]; then
    # Fallback to default target directory
    BINARY_SRC="$REPO_ROOT/target/release/$BINARY_NAME"
fi

if [ ! -f "$BINARY_SRC" ]; then
    echo "ERROR: Binary not found at $BINARY_SRC"
    exit 1
fi

cp "$BINARY_SRC" "$SERVER_DIR/$BINARY_NAME"
chmod +x "$SERVER_DIR/$BINARY_NAME"
echo "   Bundled: $SERVER_DIR/$BINARY_NAME ($(du -h "$SERVER_DIR/$BINARY_NAME" | cut -f1))"

echo "==> Installing npm dependencies..."
cd "$EXTENSION_DIR"
npm install --ignore-scripts 2>&1 | tail -3

echo "==> Compiling TypeScript..."
npx tsc -p .

echo "==> Packaging VSIX for $VSCODE_TARGET..."
npx @vscode/vsce package --target "$VSCODE_TARGET" --no-git-tag-version --no-update-package-json --allow-missing-repository

# Verify VSIX contents
VSIX_FILE=$(ls -t *.vsix 2>/dev/null | head -1)
if [ -z "$VSIX_FILE" ]; then
    echo "ERROR: No .vsix file produced"
    exit 1
fi

echo ""
echo "==> Verifying VSIX contents..."
CONTENTS=$(unzip -l "$VSIX_FILE" 2>/dev/null)

REQUIRED_FILES=("extension/out/extension.js" "extension/server/$BINARY_NAME" "extension/package.json" "extension/syntaxes/smtlib.tmLanguage.json" "extension/language-configuration.json")
ALL_OK=true
for f in "${REQUIRED_FILES[@]}"; do
    if echo "$CONTENTS" | grep -q "$f"; then
        echo "   ✓ $f"
    else
        echo "   ✗ MISSING: $f"
        ALL_OK=false
    fi
done

if [ "$ALL_OK" = true ]; then
    echo ""
    echo "==> Success: $VSIX_FILE"
    echo "   Install with: code --install-extension $EXTENSION_DIR/$VSIX_FILE"
else
    echo ""
    echo "ERROR: VSIX is missing required files!"
    exit 1
fi
