#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_NAME="anyssh-openssh-fixture:phase0"
CONTAINER_NAME="anyssh-native-xvfb-$RANDOM-$$"
RUN_DIR="$ROOT_DIR/artifacts/native-xvfb/smoke-$(date +%s)-$$"
DRIVER="$RUN_DIR/anyssh-x11-driver"
PRIVATE_KEY_FIXTURE="/tmp/000-anyssh-native-import-key"
PRIVATE_KEY_PASSPHRASE="native-key-passphrase"
WRONG_PRIVATE_KEY_PASSPHRASE="wrong-key-passphrase"
APP_GROUP=""
XVFB_PID=""
AGENT_PID=""
AGENT_SOCKET=""
AGENT_FINGERPRINT=""

cleanup() {
  if [[ -n "$APP_GROUP" ]]; then
    kill -TERM -- "-$APP_GROUP" >/dev/null 2>&1 || true
    sleep 1
    kill -KILL -- "-$APP_GROUP" >/dev/null 2>&1 || true
  fi
  if [[ -n "$XVFB_PID" ]]; then
    kill -TERM "$XVFB_PID" >/dev/null 2>&1 || true
    wait "$XVFB_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$AGENT_PID" ]]; then
    kill "$AGENT_PID" >/dev/null 2>&1 || true
    wait "$AGENT_PID" >/dev/null 2>&1 || true
  fi
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
  rm -f "$PRIVATE_KEY_FIXTURE" "$PRIVATE_KEY_FIXTURE.pub"
  if [[ -n "$AGENT_SOCKET" ]]; then
    rm -f "$AGENT_SOCKET"
  fi
}
trap cleanup EXIT

for command in \
  cc \
  dbus-run-session \
  docker \
  grep \
  pkg-config \
  pnpm \
  setsid \
  ss \
  ssh-add \
  ssh-agent \
  ssh-keygen \
  ssh-keyscan \
  Xvfb; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing native Xvfb smoke dependency: $command" >&2
    exit 1
  fi
done

if ! pkg-config --exists webkit2gtk-4.1 javascriptcoregtk-4.1; then
  echo "WebKitGTK 4.1 development files are required." >&2
  exit 1
fi

if ss -ltn 2>/dev/null | grep -Eq '127\.0\.0\.1:2222[[:space:]]'; then
  echo "Port 2222 is already in use; it is reserved by this smoke test." >&2
  exit 1
fi

mkdir -p "$RUN_DIR"
mkdir -p "$RUN_DIR/xdg-cache" "$RUN_DIR/xdg-config" "$RUN_DIR/xdg-data"
rm -f "$PRIVATE_KEY_FIXTURE" "$PRIVATE_KEY_FIXTURE.pub"
ssh-keygen \
  -q \
  -t ed25519 \
  -N "" \
  -C anyssh-native-import \
  -f "$PRIVATE_KEY_FIXTURE"
AGENT_SOCKET="/tmp/anyssh-agent-$RANDOM-$$"
ssh-agent -a "$AGENT_SOCKET" -D \
  >"$RUN_DIR/ssh-agent.stdout.log" \
  2>"$RUN_DIR/ssh-agent.stderr.log" &
AGENT_PID=$!
for _ in $(seq 1 100); do
  if [[ -S "$AGENT_SOCKET" ]]; then
    break
  fi
  if ! kill -0 "$AGENT_PID" >/dev/null 2>&1; then
    echo "ssh-agent exited before creating its socket." >&2
    cat "$RUN_DIR/ssh-agent.stderr.log" >&2 || true
    exit 1
  fi
  sleep 0.05
done
if [[ ! -S "$AGENT_SOCKET" ]]; then
  echo "ssh-agent did not create its socket." >&2
  exit 1
fi
SSH_AUTH_SOCK="$AGENT_SOCKET" ssh-add "$PRIVATE_KEY_FIXTURE" >/dev/null
AGENT_FINGERPRINT="$(
  ssh-keygen -lf "$PRIVATE_KEY_FIXTURE.pub" -E sha256 |
    awk 'NR == 1 { print $2 }'
)"
cc \
  -O2 \
  -Wall \
  -Wextra \
  -o "$DRIVER" \
  "$ROOT_DIR/tests/tools/x11/anyssh-x11-driver.c" \
  -lX11 \
  -lXtst

docker build \
  --quiet \
  --tag "$IMAGE_NAME" \
  "$ROOT_DIR/tests/fixtures/openssh" >/dev/null

docker run \
  --detach \
  --name "$CONTAINER_NAME" \
  --publish 127.0.0.1:2222:22 \
  "$IMAGE_NAME" >/dev/null

docker exec "$CONTAINER_NAME" \
  sh -c 'mkdir -p /home/anyssh/.ssh && chmod 700 /home/anyssh/.ssh'
docker cp \
  "$PRIVATE_KEY_FIXTURE.pub" \
  "$CONTAINER_NAME:/home/anyssh/.ssh/authorized_keys" >/dev/null
docker exec "$CONTAINER_NAME" \
  sh -c 'chown -R anyssh:anyssh /home/anyssh/.ssh && chmod 600 /home/anyssh/.ssh/authorized_keys'
rm -f "$PRIVATE_KEY_FIXTURE.pub"
ssh-keygen \
  -q \
  -p \
  -P "" \
  -N "$PRIVATE_KEY_PASSPHRASE" \
  -f "$PRIVATE_KEY_FIXTURE"

for _ in $(seq 1 50); do
  if ssh-keyscan -p 2222 127.0.0.1 >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

if ! ssh-keyscan -p 2222 127.0.0.1 >/dev/null 2>&1; then
  echo "OpenSSH fixture did not become ready." >&2
  exit 1
fi

DISPLAY_NUMBER=""
for candidate in $(seq 91 109); do
  if [[ ! -e "/tmp/.X${candidate}-lock" &&
    ! -S "/tmp/.X11-unix/X${candidate}" ]]; then
    DISPLAY_NUMBER="$candidate"
    break
  fi
done

if [[ -z "$DISPLAY_NUMBER" ]]; then
  echo "No free X11 display number was found." >&2
  exit 1
fi

export DISPLAY=":$DISPLAY_NUMBER"
Xvfb "$DISPLAY" \
  -screen 0 1440x900x24 \
  -nolisten tcp \
  >"$RUN_DIR/xvfb.log" 2>&1 &
XVFB_PID=$!

for _ in $(seq 1 50); do
  if [[ -S "/tmp/.X11-unix/X${DISPLAY_NUMBER}" ]]; then
    break
  fi
  sleep 0.1
done

if [[ ! -S "/tmp/.X11-unix/X${DISPLAY_NUMBER}" ]]; then
  echo "Xvfb did not become ready." >&2
  exit 1
fi

setsid dbus-run-session -- \
  bash -lc "
    export XDG_CACHE_HOME='$RUN_DIR/xdg-cache'
    export XDG_CONFIG_HOME='$RUN_DIR/xdg-config'
    export XDG_DATA_HOME='$RUN_DIR/xdg-data'
    export SSH_AUTH_SOCK='$AGENT_SOCKET'
    cd '$ROOT_DIR'
    pnpm dev:native
  " \
  >"$RUN_DIR/native.log" 2>&1 &
APP_GROUP=$!

WINDOW_READY=0
for _ in $(seq 1 300); do
  if ! kill -0 "$APP_GROUP" >/dev/null 2>&1; then
    echo "The native process exited before creating a window." >&2
    tail -n 120 "$RUN_DIR/native.log" >&2
    exit 1
  fi
  if "$DRIVER" probe >"$RUN_DIR/windows.txt" 2>/dev/null; then
    WINDOW_READY=1
    break
  fi
  sleep 0.5
done

if [[ "$WINDOW_READY" -ne 1 ]]; then
  echo "The AnySSH native window did not appear." >&2
  tail -n 120 "$RUN_DIR/native.log" >&2
  exit 1
fi

sleep 3
"$DRIVER" probe "$RUN_DIR/01-vault-create.bmp" >"$RUN_DIR/windows.txt"
"$DRIVER" click 640 430
sleep 0.25
"$DRIVER" type "246810"
"$DRIVER" tab
"$DRIVER" type "246810"
sleep 0.5
"$DRIVER" probe "$RUN_DIR/02-vault-pin-entered.bmp" >/dev/null
"$DRIVER" tab
"$DRIVER" enter

VAULT_ROOT="$RUN_DIR/xdg-data/com.spiredive.anyssh/vault"
VAULT_READY=0
for _ in $(seq 1 60); do
  if [[ -f "$VAULT_ROOT/vault.bootstrap.json" &&
    -f "$VAULT_ROOT/vault.db" ]]; then
    VAULT_READY=1
    break
  fi
  sleep 0.25
done

if [[ "$VAULT_READY" -ne 1 ]]; then
  echo "The encrypted Vault was not created." >&2
  "$DRIVER" probe "$RUN_DIR/failed-vault-create.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/native.log" >&2
  exit 1
fi

if grep -R -a -F "246810" "$VAULT_ROOT" >/dev/null 2>&1; then
  echo "The test PIN leaked into a Vault file." >&2
  exit 1
fi
if head -c 16 "$VAULT_ROOT/vault.db" | grep -a -F "SQLite format 3" >/dev/null; then
  echo "The Vault database has a plaintext SQLite header." >&2
  exit 1
fi

sleep 1
"$DRIVER" probe "$RUN_DIR/03-native-ready.bmp" >/dev/null
"$DRIVER" click 1208 44
sleep 1
"$DRIVER" probe "$RUN_DIR/04-vault-locked.bmp" >/dev/null
"$DRIVER" click 640 460
sleep 0.25
"$DRIVER" type "000000"
"$DRIVER" tab
"$DRIVER" enter
sleep 3
"$DRIVER" probe "$RUN_DIR/05-vault-wrong-pin.bmp" >/dev/null
"$DRIVER" click 640 460
sleep 0.25
"$DRIVER" type "246810"
sleep 0.5
"$DRIVER" probe "$RUN_DIR/06-vault-unlock-pin-entered.bmp" >/dev/null
"$DRIVER" tab
"$DRIVER" enter
sleep 3
"$DRIVER" probe "$RUN_DIR/07-vault-reunlocked.bmp" >/dev/null

"$DRIVER" click 100 240
sleep 1
"$DRIVER" click 1080 145
sleep 1
"$DRIVER" type "Native import fixture"
"$DRIVER" tab
"$DRIVER" type "anyssh"
"$DRIVER" tab
"$DRIVER" enter
sleep 3
"$DRIVER" ctrl-l
sleep 0.5
"$DRIVER" type "/tmp"
"$DRIVER" enter
sleep 0.5
"$DRIVER" enter
sleep 2
"$DRIVER" enter
sleep 2

PASSPHRASE_PROMPT_READY=0
for _ in $(seq 1 40); do
  if ANYSSH_X11_WINDOW_MATCH="Unlock SSH private key" \
    "$DRIVER" probe >/dev/null 2>&1; then
    PASSPHRASE_PROMPT_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$PASSPHRASE_PROMPT_READY" -ne 1 ]]; then
  echo "The encrypted Private Key passphrase prompt did not appear." >&2
  "$DRIVER" probe "$RUN_DIR/failed-private-key-prompt.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/native.log" >&2
  exit 1
fi

ANYSSH_X11_WINDOW_MATCH="Unlock SSH private key" \
  "$DRIVER" probe "$RUN_DIR/08-private-key-passphrase-prompt.bmp" >/dev/null
"$DRIVER" type "$WRONG_PRIVATE_KEY_PASSPHRASE"
"$DRIVER" click 750 452
sleep 3
if ! ANYSSH_X11_WINDOW_MATCH="Unlock SSH private key" \
  "$DRIVER" probe "$RUN_DIR/09-private-key-passphrase-retry.bmp" >/dev/null 2>&1; then
  echo "The encrypted Private Key retry prompt did not appear." >&2
  "$DRIVER" probe "$RUN_DIR/failed-private-key-retry.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/native.log" >&2
  exit 1
fi
"$DRIVER" type "$PRIVATE_KEY_PASSPHRASE"
"$DRIVER" click 750 452
sleep 5
"$DRIVER" probe "$RUN_DIR/10-private-key-imported.bmp" >/dev/null

for marker in \
  "BEGIN OPENSSH PRIVATE KEY" \
  "$PRIVATE_KEY_PASSPHRASE" \
  "$WRONG_PRIVATE_KEY_PASSPHRASE"; do
  if grep -R -a -F "$marker" "$VAULT_ROOT" >/dev/null 2>&1; then
    echo "The imported Private Key or Passphrase leaked into a Vault file." >&2
    exit 1
  fi
done
rm -f "$PRIVATE_KEY_FIXTURE"

"$DRIVER" click 900 145
sleep 2
"$DRIVER" type "Native system agent"
"$DRIVER" tab
"$DRIVER" type "anyssh"
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" enter
sleep 4
"$DRIVER" probe "$RUN_DIR/11-system-agent-created.bmp" >/dev/null
for marker in "Native system agent" "$AGENT_FINGERPRINT"; do
  if grep -R -a -F "$marker" "$VAULT_ROOT" >/dev/null 2>&1; then
    echo "System Agent Credential metadata leaked into a Vault file." >&2
    exit 1
  fi
done

"$DRIVER" click 100 106
sleep 1
"$DRIVER" click 1100 440
sleep 0.25
"$DRIVER" type "anyssh-test"
sleep 0.5
"$DRIVER" probe "$RUN_DIR/12-password-entered.bmp" >/dev/null
"$DRIVER" click 1100 495
sleep 1
"$DRIVER" probe "$RUN_DIR/13-host-key-dialog.bmp" >/dev/null

COMMAND_SUCCEEDED=0
for _ in $(seq 1 20); do
  # Before authentication this point is the Trust button. Once connected it
  # is harmless terminal space and helps preserve terminal focus.
  "$DRIVER" click 700 532
  sleep 0.5
  "$DRIVER" click 500 260
  "$DRIVER" type "touch /tmp/anyssh-native-ok"
  "$DRIVER" enter
  sleep 0.5
  if docker exec "$CONTAINER_NAME" \
    test -f /tmp/anyssh-native-ok >/dev/null 2>&1; then
    COMMAND_SUCCEEDED=1
    break
  fi
done

if [[ "$COMMAND_SUCCEEDED" -ne 1 ]]; then
  echo "A real command did not reach the OpenSSH fixture." >&2
  "$DRIVER" probe "$RUN_DIR/failed-command.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/native.log" >&2
  exit 1
fi

LARGE_OUTPUT_SUCCEEDED=0
"$DRIVER" click 500 260
"$DRIVER" type \
  "head -c 4194304 /dev/zero | tr '\\0' x; touch /tmp/anyssh-native-large-ok"
"$DRIVER" enter
for _ in $(seq 1 120); do
  if docker exec "$CONTAINER_NAME" \
    test -f /tmp/anyssh-native-large-ok >/dev/null 2>&1; then
    LARGE_OUTPUT_SUCCEEDED=1
    break
  fi
  sleep 0.5
done

if [[ "$LARGE_OUTPUT_SUCCEEDED" -ne 1 ]]; then
  echo "The native terminal did not drain the 4 MiB output stream." >&2
  "$DRIVER" probe "$RUN_DIR/failed-large-output.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/native.log" >&2
  exit 1
fi

"$DRIVER" probe "$RUN_DIR/14-command-succeeded.bmp" >/dev/null
"$DRIVER" click 1117 44
sleep 1
"$DRIVER" probe "$RUN_DIR/15-disconnected.bmp" >/dev/null
"$DRIVER" click 1208 44
sleep 1
"$DRIVER" probe "$RUN_DIR/16-vault-locked-after-session.bmp" >/dev/null

if ! kill -0 "$APP_GROUP" >/dev/null 2>&1; then
  echo "The native process exited unexpectedly." >&2
  exit 1
fi

cat >"$RUN_DIR/report.md" <<EOF
# AnySSH native Xvfb smoke report

- Result: PASS
- Identifier: \`com.spiredive.anyssh\`
- Display: \`$DISPLAY\`

## Verified

- Tauri launched a mapped X11 window named \`AnySSH\` without a desktop environment.
- WebKitGTK loaded the React/xterm.js application through the native runtime.
- A PIN Slot created a SQLCipher Vault inside an isolated app-data directory.
- The test PIN was absent from the Bootstrap, database, WAL, and sidecar files.
- The SQLCipher database did not expose the plaintext SQLite file header.
- Lock Vault dropped the unlocked Rust storage state and returned to the PIN gate.
- An incorrect PIN was rejected without opening the workspace.
- The same PIN reopened the existing Vault before the SSH session started.
- The Credential UI opened a native file picker for an encrypted Ed25519
  Private Key, then displayed an in-process GTK Secure Entry outside WebView.
- An incorrect Passphrase produced a bounded retry prompt; the correct
  Passphrase imported the original encrypted Key without adding a Secret IPC
  field.
- The imported Key and both test Passphrases remained absent from Vault file
  plaintext scans, and the temporary source file was deleted before SSH testing
  continued.
- The same native UI enumerated one \`SSH_AUTH_SOCK\` Identity and created a
  Fingerprint-selected System Agent Credential without persisting the Agent Key
  or Fingerprint in plaintext.
- Password input reached the native WebView.
- Host-key confirmation displayed the scoped endpoint and SHA-256 fingerprint and was accepted.
- The Rust SSH core authenticated against the Docker OpenSSH fixture.
- X11 keyboard events reached xterm.js and created \`/tmp/anyssh-native-ok\` remotely.
- A 4 MiB terminal stream drained through Tauri/xterm acknowledgement backpressure and
  created \`/tmp/anyssh-native-large-ok\` after the output completed.
- Disconnect returned the UI to the disconnected state.
- Lock Vault returned to the PIN gate after the SSH session ended.

## Evidence

- \`01-vault-create.bmp\`
- \`02-vault-pin-entered.bmp\`
- \`03-native-ready.bmp\`
- \`04-vault-locked.bmp\`
- \`05-vault-wrong-pin.bmp\`
- \`06-vault-unlock-pin-entered.bmp\`
- \`07-vault-reunlocked.bmp\`
- \`08-private-key-passphrase-prompt.bmp\`
- \`09-private-key-passphrase-retry.bmp\`
- \`10-private-key-imported.bmp\`
- \`11-system-agent-created.bmp\`
- \`12-password-entered.bmp\`
- \`13-host-key-dialog.bmp\`
- \`14-command-succeeded.bmp\`
- \`15-disconnected.bmp\`
- \`16-vault-locked-after-session.bmp\`
- \`windows.txt\`
- \`native.log\`
EOF

echo "Native Xvfb SSH smoke passed: $RUN_DIR"
