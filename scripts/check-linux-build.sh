#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for command in file ldd pnpm sha256sum; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing Linux build dependency: $command" >&2
    exit 1
  fi
done

cd "$ROOT_DIR"
pnpm --filter @anyssh/client tauri build --debug --no-bundle --ci

BINARY_PATH="$ROOT_DIR/target/debug/anyssh-client"
if [[ ! -x "$BINARY_PATH" ]]; then
  echo "Linux build completed without producing: $BINARY_PATH" >&2
  exit 1
fi

RUN_DIR="$ROOT_DIR/artifacts/linux-build/build-$(date +%s)-$$"
mkdir -p "$RUN_DIR"
cp "$BINARY_PATH" "$RUN_DIR/anyssh-client"
file "$RUN_DIR/anyssh-client" >"$RUN_DIR/file.txt"
ldd "$RUN_DIR/anyssh-client" >"$RUN_DIR/ldd.txt"

if ! grep -q 'ELF 64-bit' "$RUN_DIR/file.txt"; then
  echo "The Linux build did not produce a 64-bit ELF executable." >&2
  exit 1
fi
if ! grep -q 'libwebkit2gtk-4.1' "$RUN_DIR/ldd.txt"; then
  echo "The Linux executable is not linked against WebKitGTK 4.1." >&2
  exit 1
fi
if ! grep -a -q 'SQLCipher private heap stats' "$RUN_DIR/anyssh-client"; then
  echo "The Linux executable does not contain the bundled SQLCipher marker." >&2
  exit 1
fi
printf '%s\n' 'SQLCipher private heap stats' >"$RUN_DIR/sqlcipher-marker.txt"

(
  cd "$RUN_DIR"
  sha256sum "anyssh-client" >"SHA256SUMS"
)

cat >"$RUN_DIR/report.md" <<EOF
# AnySSH Linux build report

- Result: PASS
- Identifier: \`com.spiredive.anyssh\`
- Artifact: \`anyssh-client\`

## Verified

- The React frontend completed its production build.
- Tauri, russh, Vault, bundled SQLCipher, and the Linux application shell linked.
- The result is a 64-bit ELF executable linked against WebKitGTK 4.1.
- The executable contains a bundled SQLCipher implementation marker.

## Evidence

- \`anyssh-client\`
- \`SHA256SUMS\`
- \`file.txt\`
- \`ldd.txt\`
- \`sqlcipher-marker.txt\`
EOF

echo "Linux native build passed: $RUN_DIR"
