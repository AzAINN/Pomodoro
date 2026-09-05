#!/bin/sh
# Install a published GitHub release. No Rust, Python, or sudo required.
set -eu

fail() { printf 'pomo: %s\n' "$*" >&2; exit 1; }

main() {
    for command in curl tar mktemp uname awk; do
        command -v "$command" >/dev/null 2>&1 || fail "Required command not found: $command"
    done
    case "$(uname -s)/$(uname -m)" in
        Darwin/arm64|Darwin/aarch64) target=aarch64-apple-darwin ;;
        Darwin/x86_64) target=x86_64-apple-darwin ;;
        Linux/aarch64|Linux/arm64) target=aarch64-unknown-linux-musl ;;
        Linux/x86_64|Linux/amd64) target=x86_64-unknown-linux-musl ;;
        *) fail 'Unsupported platform. Use cargo install --git https://github.com/AzAINN/Pomodoro --locked pomo-tui' ;;
    esac

    if command -v sha256sum >/dev/null 2>&1; then
        checksum_command=sha256sum
    elif command -v shasum >/dev/null 2>&1; then
        checksum_command=shasum
    else
        fail 'Install sha256sum or shasum to verify the download'
    fi

    install_dir=${POMO_INSTALL_DIR:-"${HOME:?Set HOME or POMO_INSTALL_DIR}/.local/bin"}
    case "$install_dir" in /*) ;; *) fail 'POMO_INSTALL_DIR must be an absolute path' ;; esac
    base=https://github.com/AzAINN/Pomodoro/releases/latest/download
    if [ -n "${POMO_VERSION:-}" ]; then
        case "$POMO_VERSION" in
            v[0-9]*) ;;
            *) fail 'POMO_VERSION must be a release tag such as v0.2.0' ;;
        esac
        case "$POMO_VERSION" in *[!a-zA-Z0-9.+-]*) fail 'Invalid release tag' ;; esac
        base="https://github.com/AzAINN/Pomodoro/releases/download/$POMO_VERSION"
    fi

    archive="pomo-$target.tar.gz"
    work=$(mktemp -d "${TMPDIR:-/tmp}/pomo-install.XXXXXXXX")
    staged=
    trap 'rm -rf "$work"; if [ -n "$staged" ]; then rm -f "$staged"; fi' EXIT
    trap 'exit 1' HUP INT TERM
    printf 'Downloading pomo for %s…\n' "$target"
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
        "$base/$archive" --output "$work/$archive" || fail 'Download failed. Check that a public release exists for this platform.'
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
        "$base/SHA256SUMS" --output "$work/SHA256SUMS" || fail 'Could not download release checksums'
    expected=$(awk -v name="$archive" '$2 == name {print $1}' "$work/SHA256SUMS")
    [ "${#expected}" -eq 64 ] || fail 'Missing or malformed release checksum'
    case "$expected" in *[!0-9a-fA-F]*) fail 'Invalid release checksum' ;; esac
    if [ "$checksum_command" = sha256sum ]; then
        actual=$(sha256sum "$work/$archive" | awk '{print $1}')
    else
        actual=$(shasum -a 256 "$work/$archive" | awk '{print $1}')
    fi
    [ "$actual" = "$expected" ] || fail 'Checksum mismatch; nothing was installed'

    # Extract only the binary, not arbitrary paths from the archive.
    tar -xzf "$work/$archive" -C "$work" pomo
    [ -f "$work/pomo" ] && [ ! -L "$work/pomo" ] || fail 'Release has no regular pomo executable'
    mkdir -p "$install_dir"
    [ ! -d "$install_dir/pomo" ] || fail "$install_dir/pomo is a directory"
    staged=$(mktemp "$install_dir/.pomo.XXXXXXXX")
    cp "$work/pomo" "$staged"
    chmod 755 "$staged"
    mv -f "$staged" "$install_dir/pomo"
    staged=
    printf 'Installed %s/pomo\n' "$install_dir"
    case ":$PATH:" in
        *":$install_dir:"*) printf 'Run: pomo\n' ;;
        *) printf 'Add %s to your PATH, then run pomo.\n' "$install_dir" ;;
    esac
}

main "$@"
