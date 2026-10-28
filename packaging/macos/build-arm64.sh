#!/usr/bin/env bash
# Build and package the Apple Silicon app locally, without release credentials.
set -euo pipefail

if [ "${1:-}" = --help ] || [ "${1:-}" = -h ]; then
    cat <<'EOF'
Usage: packaging/macos/build-arm64.sh

Build the macOS arm64 release binary and package Termirs.app and an .app.zip.
Requires macOS, rustup and Xcode Command Line Tools (xcode-select --install).
Output: target/macos-arm64/ (under CARGO_TARGET_DIR when set).
BUILD_NUMBER can override the bundle build number (default: UTC timestamp).
The app is ad-hoc signed for local use; it is not notarized.
EOF
    exit 0
fi
if [ "$#" -ne 0 ]; then
    echo "Usage: $0 [--help]" >&2
    exit 1
fi
if [ "$(uname -s)" != Darwin ]; then
    echo "error: run this script on macOS." >&2
    exit 1
fi
for tool in cargo rustup xcrun lipo codesign ditto plutil; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: missing $tool; install rustup and Xcode Command Line Tools." >&2
        exit 1
    fi
done
xcrun --find clang >/dev/null
xcrun --sdk macosx --show-sdk-path >/dev/null

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cd "$root"

target=aarch64-apple-darwin
target_dir=${CARGO_TARGET_DIR:-"$root/target"}
mkdir -p "$target_dir"
target_dir=$(cd "$target_dir" && pwd)
out="$target_dir/macos-arm64"
build=${BUILD_NUMBER:-$(date -u +%Y%m%d%H%M%S)}
case "$build" in
    *[!0-9]*|'') echo "error: BUILD_NUMBER must contain only digits." >&2; exit 1 ;;
esac

# Running inside the repository selects rust-toolchain.toml's pinned toolchain.
rustup target add "$target"
package=$(cargo pkgid --locked --package shellrs)
version=${package##*@}

cargo build --release --locked --bin shellrs --target "$target" --target-dir "$target_dir"
binary="$target_dir/$target/release/shellrs"
lipo "$binary" -verify_arch arm64

mkdir -p "$out"
bash "$here/bundle.sh" "$binary" "$version" "$build" "$out"
app="$out/Termirs.app"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
archive="$out/Termirs-$version-macos-aarch64.app.zip"
ditto -c -k --keepParent "$app" "$archive"

printf '\nBuilt macOS arm64:\n  %s\n  %s\n' "$app" "$archive"
