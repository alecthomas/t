#!/bin/sh
set -eu

REPO="alecthomas/t"
BINARY="t"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
}

get_arch() {
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
    esac
}

get_os() {
    os=$(uname -s)
    case "$os" in
        Darwin) echo "apple-darwin" ;;
        Linux) echo "unknown-linux-gnu" ;;
        *) echo "Unsupported OS: $os" >&2; exit 1 ;;
    esac
}

main() {
    version="${1:-$(get_latest_version)}"
    arch=$(get_arch)
    os=$(get_os)

    asset="${BINARY}-${arch}-${os}.bz2"
    url="https://github.com/${REPO}/releases/download/${version}/${asset}"

    echo "Downloading ${BINARY} ${version} for ${arch}-${os}..."

    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    curl -fsSL "$url" | bunzip2 > "${tmpdir}/${BINARY}"
    chmod +x "${tmpdir}/${BINARY}"

    if [ -w "$INSTALL_DIR" ]; then
        mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        echo "Installing to ${INSTALL_DIR} (requires sudo)..."
        sudo mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    fi

    echo "Installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"
}

main "$@"
