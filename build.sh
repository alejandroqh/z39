#!/usr/bin/env bash
set -euo pipefail

# Static musl linking opens many file descriptors at once; raise the limit.
ulimit -n 4096 2>/dev/null || true

VERSION=$(cargo metadata --no-deps --format-version 1 | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])")
NAME="z39"
Z3_VERSION="4.16.0"
OUT_DIR="dist"

mkdir -p "$OUT_DIR"

echo "=== Building $NAME v$VERSION with Z3 v$Z3_VERSION ==="

# --- Helper: download Z3 release binary for macOS/Windows ---
download_z3() {
    local SUFFIX="$1"
    local EXT="$2"  # zip for macOS/windows
    local DEST="$3"
    local Z3_ZIP="z3-${Z3_VERSION}-${SUFFIX}.${EXT}"
    local Z3_URL="https://github.com/Z3Prover/z3/releases/download/z3-${Z3_VERSION}/${Z3_ZIP}"

    if [ -x "${DEST}/z3" ] || [ -x "${DEST}/z3.exe" ]; then
        echo "  Z3 already downloaded in ${DEST}, skipping"
        return 0
    fi

    echo "  Downloading ${Z3_ZIP}..."
    local TMPDIR
    TMPDIR=$(mktemp -d)
    curl -sL "$Z3_URL" -o "${TMPDIR}/${Z3_ZIP}"
    unzip -o "${TMPDIR}/${Z3_ZIP}" -d "${TMPDIR}/z3out" > /dev/null

    mkdir -p "${DEST}"
    # Find z3 binary
    local Z3_BIN
    Z3_BIN=$(find "${TMPDIR}/z3out" -type f \( -name "z3" -o -name "z3.exe" \) | head -1) || true
    if [ -z "$Z3_BIN" ]; then
        echo "  ERROR: z3 binary not found in ${Z3_ZIP}"
        rm -rf "${TMPDIR}"
        return 1
    fi
    cp "$Z3_BIN" "${DEST}/"
    chmod +x "${DEST}/z3" "${DEST}/z3.exe" 2>/dev/null || true

    # Also copy libz3 dll for Windows
    local Z3_DLL
    Z3_DLL=$(find "${TMPDIR}/z3out" -name "libz3.dll" -o -name "libz3.so" | head -1) || true
    [ -n "$Z3_DLL" ] && cp "$Z3_DLL" "${DEST}/" || true

    rm -rf "${TMPDIR}"
    echo "  Z3 downloaded to ${DEST}"
}

# --- macOS ARM64 (native) ---
echo ""
echo "--- macOS arm64 (aarch64-apple-darwin) ---"
download_z3 "arm64-osx-15.7.3" "zip" "vendor/z3-macos-arm64"
cargo build --release --target aarch64-apple-darwin
mkdir -p "$OUT_DIR/$NAME-macos-arm64"
cp target/aarch64-apple-darwin/release/$NAME "$OUT_DIR/$NAME-macos-arm64/"
cp vendor/z3-macos-arm64/z3 "$OUT_DIR/$NAME-macos-arm64/"
echo "  -> $OUT_DIR/$NAME-macos-arm64/"

# --- macOS x86_64 ---
echo ""
echo "--- macOS x64 (x86_64-apple-darwin) ---"
download_z3 "x64-osx-15.7.3" "zip" "vendor/z3-macos-x64"
cargo build --release --target x86_64-apple-darwin
mkdir -p "$OUT_DIR/$NAME-macos-x64"
cp target/x86_64-apple-darwin/release/$NAME "$OUT_DIR/$NAME-macos-x64/"
cp vendor/z3-macos-x64/z3 "$OUT_DIR/$NAME-macos-x64/"
echo "  -> $OUT_DIR/$NAME-macos-x64/"

# --- Linux ARM64 (glibc 2.38+) ---
echo ""
echo "--- Linux arm64 (aarch64-unknown-linux-gnu) ---"
download_z3 "arm64-glibc-2.38" "zip" "vendor/z3-linux-arm64"
cargo zigbuild --release --target aarch64-unknown-linux-gnu
mkdir -p "$OUT_DIR/$NAME-linux-arm64"
cp target/aarch64-unknown-linux-gnu/release/$NAME "$OUT_DIR/$NAME-linux-arm64/"
cp vendor/z3-linux-arm64/z3 "$OUT_DIR/$NAME-linux-arm64/"
echo "  -> $OUT_DIR/$NAME-linux-arm64/"

# --- Linux x86_64 (glibc 2.39+) ---
echo ""
echo "--- Linux x64 (x86_64-unknown-linux-gnu) ---"
download_z3 "x64-glibc-2.39" "zip" "vendor/z3-linux-x64"
cargo zigbuild --release --target x86_64-unknown-linux-gnu
mkdir -p "$OUT_DIR/$NAME-linux-x64"
cp target/x86_64-unknown-linux-gnu/release/$NAME "$OUT_DIR/$NAME-linux-x64/"
cp vendor/z3-linux-x64/z3 "$OUT_DIR/$NAME-linux-x64/"
echo "  -> $OUT_DIR/$NAME-linux-x64/"

# --- Windows x64 (GNU via zigbuild) ---
echo ""
echo "--- Windows x64 (x86_64-pc-windows-gnu) ---"
download_z3 "x64-win" "zip" "vendor/z3-windows-x64"
cargo zigbuild --release --target x86_64-pc-windows-gnu
mkdir -p "$OUT_DIR/$NAME-windows-x64"
cp target/x86_64-pc-windows-gnu/release/$NAME.exe "$OUT_DIR/$NAME-windows-x64/"
cp vendor/z3-windows-x64/z3.exe "$OUT_DIR/$NAME-windows-x64/"
cp vendor/z3-windows-x64/libz3.dll "$OUT_DIR/$NAME-windows-x64/" 2>/dev/null || true
echo "  -> $OUT_DIR/$NAME-windows-x64/"

# --- Create distribution archives ---
echo ""
echo "=== Creating archives ==="
cd "$OUT_DIR"
for dir in $NAME-*/; do
    PLATFORM="${dir%/}"
    echo "  ${PLATFORM}.zip"
    rm -f "${PLATFORM}.zip"
    zip -qr "${PLATFORM}.zip" "${PLATFORM}/"
    rm -rf "${PLATFORM}/"
done
cd ..

echo ""
echo "=== Done ==="
echo "Distribution packages:"
ls -lh "$OUT_DIR"/*.zip 2>/dev/null || echo "  (no archives created)"