#!/usr/bin/env sh
set -eu

repo="dikmri/rndWalker"
install_dir="${RNDWALKER_INSTALL_DIR:-$HOME/.local/share/rndWalker}"
bin_dir="${RNDWALKER_BIN_DIR:-$HOME/.local/bin}"
version="${RNDWALKER_VERSION:-latest}"

log() {
    printf '[rndWalker] %s\n' "$1"
}

warn() {
    printf '[rndWalker] WARNING: %s\n' "$1" >&2
}

die() {
    printf '[rndWalker] ERROR: %s\n' "$1" >&2
    exit 1
}

need_command() {
    command -v "$1" >/dev/null 2>&1 || die "$1 is required."
}

normalize_tag() {
    case "$1" in
        v*) printf '%s\n' "$1" ;;
        *) printf 'v%s\n' "$1" ;;
    esac
}

install_macos_deps() {
    command -v brew >/dev/null 2>&1 || {
        warn "Homebrew was not found. If rndWalker fails to start, install FFmpeg with Homebrew."
        return
    }

    if brew list ffmpeg@7 >/dev/null 2>&1 || brew list ffmpeg >/dev/null 2>&1; then
        return
    fi

    log "Installing FFmpeg with Homebrew"
    brew install ffmpeg@7 || warn "Could not install ffmpeg@7. Install FFmpeg manually if rndWalker fails to start."
}

install_linux_deps() {
    if ! command -v sudo >/dev/null 2>&1; then
        warn "sudo was not found. Install FFmpeg and SDL2 manually if rndWalker fails to start."
        return
    fi

    if command -v apt-get >/dev/null 2>&1; then
        log "Installing common runtime packages with apt-get"
        sudo apt-get update || warn "apt-get update failed."
        sudo apt-get install -y ffmpeg libsdl2-2.0-0 || warn "Could not install runtime packages with apt-get."
        return
    fi

    if command -v dnf >/dev/null 2>&1; then
        log "Installing common runtime packages with dnf"
        sudo dnf install -y ffmpeg SDL2 || warn "Could not install runtime packages with dnf."
        return
    fi

    if command -v pacman >/dev/null 2>&1; then
        log "Installing common runtime packages with pacman"
        sudo pacman -S --needed --noconfirm ffmpeg sdl2 || warn "Could not install runtime packages with pacman."
        return
    fi

    warn "Supported package manager was not found. Install FFmpeg and SDL2 manually if rndWalker fails to start."
}

need_command curl
need_command uname
need_command sed
need_command head
need_command find
need_command ln

extract_zip() {
    archive_path="$1"
    output_dir="$2"

    if command -v unzip >/dev/null 2>&1; then
        unzip -q "$archive_path" -d "$output_dir"
        return
    fi

    if command -v python3 >/dev/null 2>&1; then
        python3 - "$archive_path" "$output_dir" <<'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as archive:
    archive.extractall(sys.argv[2])
PY
        return
    fi

    if command -v python >/dev/null 2>&1; then
        python - "$archive_path" "$output_dir" <<'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as archive:
    archive.extractall(sys.argv[2])
PY
        return
    fi

    die "unzip, python3, or python is required to extract the release archive."
}

os_name="$(uname -s)"
arch_name="$(uname -m)"

case "$os_name:$arch_name" in
    Darwin:x86_64)
        target="x86_64-apple-darwin"
        ;;
    Darwin:arm64)
        target="aarch64-apple-darwin"
        ;;
    Linux:x86_64|Linux:amd64)
        target="x86_64-unknown-linux-gnu"
        ;;
    *)
        die "Unsupported platform: $os_name $arch_name"
        ;;
esac

case "$install_dir" in
    ""|"/"|"$HOME"|"$HOME/")
        die "Refusing to install into an unsafe directory: $install_dir"
        ;;
esac

if [ "$version" = "latest" ]; then
    log "Fetching latest release metadata"
    release_json="$(curl -fsSL -H "Accept: application/vnd.github+json" -H "User-Agent: rndWalker-installer" "https://api.github.com/repos/$repo/releases/latest")"
    tag="$(printf '%s\n' "$release_json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
else
    tag="$(normalize_tag "$version")"
fi

[ -n "$tag" ] || die "Could not determine the release tag."

asset="rndWalker-$tag-$target.zip"
url="https://github.com/$repo/releases/download/$tag/$asset"
tmp_root="${TMPDIR:-/tmp}/rndWalker-install.$$"
zip_path="$tmp_root/$asset"
extract_dir="$tmp_root/extract"

cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT HUP INT TERM

if [ "${RNDWALKER_SKIP_DEPS:-0}" != "1" ]; then
    case "$os_name" in
        Darwin) install_macos_deps ;;
        Linux) install_linux_deps ;;
    esac
fi

mkdir -p "$extract_dir"

log "Downloading $asset"
curl -fsSL -H "User-Agent: rndWalker-installer" -o "$zip_path" "$url"

log "Extracting archive"
extract_zip "$zip_path" "$extract_dir"

log "Installing to $install_dir"
mkdir -p "$install_dir"
find "$install_dir" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
cp -R "$extract_dir"/. "$install_dir"/
chmod +x "$install_dir/rndWalker"

if [ "$os_name" = "Darwin" ] && command -v xattr >/dev/null 2>&1; then
    xattr -dr com.apple.quarantine "$install_dir" >/dev/null 2>&1 || true
fi

mkdir -p "$bin_dir"
ln -sfn "$install_dir/rndWalker" "$bin_dir/rndWalker"

log "Installed $tag"
printf 'Executable: %s\n' "$install_dir/rndWalker"
printf 'Command: %s\n' "$bin_dir/rndWalker"

case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) warn "$bin_dir is not in PATH. Add it to your shell profile to run rndWalker by name." ;;
esac
