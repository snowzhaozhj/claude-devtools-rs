#!/bin/sh
set -eu

REPO="snowzhaozhj/claude-devtools-rs"
BINARY="cdt"
INSTALL_DIR="${CDT_INSTALL_DIR:-$HOME/.local/bin}"

main() {
    need_cmd curl

    local os arch asset version

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os="darwin" ;;
        Linux)  os="linux" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *) err "Unsupported OS: $os" ;;
    esac

    if [ "$os" = "windows" ]; then
        need_cmd unzip
    else
        need_cmd tar
    fi

    case "$arch" in
        x86_64|amd64) arch="x64" ;;
        arm64|aarch64) arch="arm64" ;;
        *) err "Unsupported architecture: $arch" ;;
    esac

    if [ "$os" = "linux" ] && [ "$arch" = "arm64" ]; then
        err "Linux arm64 binary is not yet available. Build from source: cargo install --git https://github.com/$REPO cdt-cli"
    fi

    if [ "$os" = "windows" ]; then
        asset="cdt-windows-x64.zip"
    else
        asset="cdt-${os}-${arch}.tar.gz"
    fi

    if [ -n "${CDT_VERSION:-}" ]; then
        version="$CDT_VERSION"
    else
        version="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"//;s/".*//')" || true
        if [ -z "$version" ]; then
            err "Failed to determine latest version (GitHub API may be rate-limited). Set CDT_VERSION=vX.Y.Z to install a specific version, or set GH_TOKEN to avoid rate limits."
        fi
    fi

    local url="https://github.com/$REPO/releases/download/${version}/${asset}"

    echo "Installing cdt ${version} (${os}/${arch})..."
    echo "  from: $url"
    echo "  to:   $INSTALL_DIR/$BINARY"
    echo ""

    ensure mkdir -p "$INSTALL_DIR"

    if [ "$os" = "windows" ]; then
        local tmp
        tmp="$(mktemp -d)"
        ensure curl -fsSL "$url" -o "$tmp/cdt.zip"
        ensure unzip -oq "$tmp/cdt.zip" -d "$tmp"
        ensure mv "$tmp/cdt.exe" "$INSTALL_DIR/cdt.exe"
        rm -rf "$tmp"
        echo "Installed cdt.exe to $INSTALL_DIR/cdt.exe"
    else
        curl -fsSL "$url" | tar xz -C "$INSTALL_DIR"
        ensure chmod +x "$INSTALL_DIR/$BINARY"
        echo "Installed $BINARY to $INSTALL_DIR/$BINARY"
    fi
    echo ""

    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo "NOTE: $INSTALL_DIR is not in your PATH."
        echo "Add it by appending to your shell profile:"
        echo ""
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        echo ""
    fi

    echo "Run 'cdt --help' to get started."
    echo ""
    echo "Quick setup for Claude Code integration:"
    echo "  cdt setup mcp --apply   # Register as MCP server"
    echo "  cdt setup skills        # Install session analysis skills"
}

need_cmd() {
    if ! command -v "$1" > /dev/null 2>&1; then
        err "need '$1' (command not found)"
    fi
}

ensure() {
    if ! "$@"; then
        err "command failed: $*"
    fi
}

err() {
    echo "error: $1" >&2
    exit 1
}

main "$@"
