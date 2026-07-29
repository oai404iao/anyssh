#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_NAME="anyssh-openssh-fixture:phase0"
PAM_IMAGE_NAME="anyssh-openssh-pam-fixture:phase1"
CONTAINER_NAME="anyssh-native-xvfb-$RANDOM-$$"
PAM_CONTAINER_NAME="anyssh-native-xvfb-pam-$RANDOM-$$"
RUN_DIR="$ROOT_DIR/artifacts/native-xvfb/smoke-$(date +%s)-$$"
DRIVER="$RUN_DIR/anyssh-x11-driver"
PRIVATE_KEY_FIXTURE="/tmp/000-anyssh-native-import-key"
PRIVATE_KEY_PASSPHRASE="native-key-passphrase"
WRONG_PRIVATE_KEY_PASSPHRASE="wrong-key-passphrase"
GENERATED_EXPORT_PATH="/tmp/anyssh-native-generated-export-$RANDOM-$$.key"
GENERATED_EXPORT_PUBLIC_PATH="$GENERATED_EXPORT_PATH.pub"
GENERATED_REIMPORT_PATH="/tmp/000-anyssh-native-reimport-key"
GENERATED_EXPORT_PASSPHRASE="native-export-passphrase"
WRONG_GENERATED_EXPORT_PASSPHRASE="wrong-native-export-passphrase"
INTERACTIVE_RESPONSE="otp-$RANDOM-$RANDOM"
FORWARD_ECHO_PORT=8080
LOCAL_FORWARD_PORT=18080
DYNAMIC_FORWARD_PORT=18081
REMOTE_FORWARD_PORT=18082
REMOTE_DESTINATION_PORT=18083
VAULT_LOCK_FORWARD_PORT=18084
TAB_CLOSE_FORWARD_PORT=18085
LOCAL_FORWARD_MARKER="local-forward-$RANDOM-$RANDOM"
DYNAMIC_FORWARD_MARKER="dynamic-forward-$RANDOM-$RANDOM"
REMOTE_FORWARD_MARKER="remote-forward-$RANDOM-$RANDOM"
APP_GROUP=""
XVFB_PID=""
AGENT_PID=""
FORWARD_SERVER_PID=""
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
  if [[ -n "$FORWARD_SERVER_PID" ]]; then
    kill "$FORWARD_SERVER_PID" >/dev/null 2>&1 || true
    wait "$FORWARD_SERVER_PID" >/dev/null 2>&1 || true
  fi
  docker rm -f "$CONTAINER_NAME" "$PAM_CONTAINER_NAME" >/dev/null 2>&1 || true
  rm -f \
    "$PRIVATE_KEY_FIXTURE" \
    "$PRIVATE_KEY_FIXTURE.pub" \
    "$GENERATED_EXPORT_PATH" \
    "$GENERATED_EXPORT_PUBLIC_PATH" \
    "$GENERATED_REIMPORT_PATH"
  if [[ -n "$AGENT_SOCKET" ]]; then
    rm -f "$AGENT_SOCKET"
  fi
}
trap cleanup EXIT

scroll_connection_panel_top() {
  "$DRIVER" click 1240 250
  "$DRIVER" scroll-up 24
  sleep 0.5
}

for command in \
  cc \
  dbus-run-session \
  docker \
  grep \
  pkg-config \
  pnpm \
  python3 \
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
if ss -ltn 2>/dev/null | grep -Eq '127\.0\.0\.1:2223[[:space:]]'; then
  echo "Port 2223 is already in use; it is reserved by this smoke test." >&2
  exit 1
fi
for port in \
  "$LOCAL_FORWARD_PORT" \
  "$DYNAMIC_FORWARD_PORT" \
  "$REMOTE_DESTINATION_PORT" \
  "$VAULT_LOCK_FORWARD_PORT" \
  "$TAB_CLOSE_FORWARD_PORT"; do
  if ss -ltn 2>/dev/null | grep -Eq "127\\.0\\.0\\.1:${port}[[:space:]]"; then
    echo "Port $port is already in use; it is reserved by this smoke test." >&2
    exit 1
  fi
done

mkdir -p "$RUN_DIR"
mkdir -p "$RUN_DIR/xdg-cache" "$RUN_DIR/xdg-config" "$RUN_DIR/xdg-data"
rm -f \
  "$PRIVATE_KEY_FIXTURE" \
  "$PRIVATE_KEY_FIXTURE.pub" \
  "$GENERATED_EXPORT_PATH" \
  "$GENERATED_EXPORT_PUBLIC_PATH" \
  "$GENERATED_REIMPORT_PATH"
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
docker build \
  --quiet \
  --file "$ROOT_DIR/tests/fixtures/openssh/Dockerfile.pam" \
  --tag "$PAM_IMAGE_NAME" \
  "$ROOT_DIR/tests/fixtures/openssh" >/dev/null

docker run \
  --detach \
  --name "$CONTAINER_NAME" \
  --publish 127.0.0.1:2222:22 \
  "$IMAGE_NAME" >/dev/null
docker run \
  --detach \
  --name "$PAM_CONTAINER_NAME" \
  --publish 127.0.0.1:2223:22 \
  --env "ANYSSH_OTP_TOKEN=$INTERACTIVE_RESPONSE" \
  "$PAM_IMAGE_NAME" >/dev/null

docker exec -d "$CONTAINER_NAME" \
  sh -c "exec nc -lk -p $FORWARD_ECHO_PORT -e /bin/cat"

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
  if ssh-keyscan -p 2222 127.0.0.1 >/dev/null 2>&1 \
    && ssh-keyscan -p 2223 127.0.0.1 >/dev/null 2>&1 \
    && docker exec "$CONTAINER_NAME" \
      nc -z -w 1 127.0.0.1 "$FORWARD_ECHO_PORT" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

if ! ssh-keyscan -p 2222 127.0.0.1 >/dev/null 2>&1 \
  || ! ssh-keyscan -p 2223 127.0.0.1 >/dev/null 2>&1 \
  || ! docker exec "$CONTAINER_NAME" \
    nc -z -w 1 127.0.0.1 "$FORWARD_ECHO_PORT" >/dev/null 2>&1; then
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

"$DRIVER" click 700 145
sleep 1
"$DRIVER" type "Native generated key"
"$DRIVER" tab
"$DRIVER" type "anyssh"
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" enter
sleep 5
"$DRIVER" probe "$RUN_DIR/10a-generated-private-key.bmp" >/dev/null

"$DRIVER" click 1025 276
sleep 2
"$DRIVER" probe "$RUN_DIR/10b-generated-public-key.bmp" >/dev/null
"$DRIVER" click 878 198
sleep 1

"$DRIVER" click 1115 276
EXPORT_PICKER_READY=0
for _ in $(seq 1 40); do
  if ANYSSH_X11_WINDOW_MATCH="Export encrypted SSH private key" \
    "$DRIVER" probe >/dev/null 2>&1; then
    EXPORT_PICKER_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$EXPORT_PICKER_READY" -ne 1 ]]; then
  echo "The encrypted Private Key export picker did not appear." >&2
  "$DRIVER" probe "$RUN_DIR/failed-private-key-export-picker.bmp" >/dev/null || true
  exit 1
fi
ANYSSH_X11_WINDOW_MATCH="Export encrypted SSH private key" \
  "$DRIVER" probe "$RUN_DIR/10c-private-key-export-picker.bmp" >/dev/null
"$DRIVER" ctrl-l
sleep 0.5
"$DRIVER" type "$GENERATED_EXPORT_PATH"
"$DRIVER" enter
sleep 1
"$DRIVER" enter

STEP_UP_READY=0
for _ in $(seq 1 40); do
  if ANYSSH_X11_WINDOW_MATCH="Confirm AnySSH PIN" \
    "$DRIVER" probe >/dev/null 2>&1; then
    STEP_UP_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$STEP_UP_READY" -ne 1 ]]; then
  echo "The native Vault step-up prompt did not appear." >&2
  "$DRIVER" probe "$RUN_DIR/failed-private-key-export-pin.bmp" >/dev/null || true
  exit 1
fi
ANYSSH_X11_WINDOW_MATCH="Confirm AnySSH PIN" \
  "$DRIVER" probe "$RUN_DIR/10d-private-key-export-pin.bmp" >/dev/null
"$DRIVER" type "000000"
"$DRIVER" enter
sleep 2
if ! ANYSSH_X11_WINDOW_MATCH="Confirm AnySSH PIN" \
  "$DRIVER" probe "$RUN_DIR/10e-private-key-export-pin-retry.bmp" >/dev/null 2>&1; then
  echo "The native Vault step-up retry prompt did not appear." >&2
  exit 1
fi
"$DRIVER" type "246810"
"$DRIVER" enter
sleep 2

EXPORT_PASSPHRASE_READY=0
for _ in $(seq 1 40); do
  if ANYSSH_X11_WINDOW_MATCH="Encrypt exported private key" \
    "$DRIVER" probe >/dev/null 2>&1; then
    EXPORT_PASSPHRASE_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$EXPORT_PASSPHRASE_READY" -ne 1 ]]; then
  echo "The native export Passphrase prompt did not appear." >&2
  exit 1
fi
ANYSSH_X11_WINDOW_MATCH="Encrypt exported private key" \
  "$DRIVER" probe "$RUN_DIR/10f-private-key-export-passphrase.bmp" >/dev/null
"$DRIVER" type "$GENERATED_EXPORT_PASSPHRASE"
"$DRIVER" tab
"$DRIVER" type "$WRONG_GENERATED_EXPORT_PASSPHRASE"
"$DRIVER" enter
sleep 2
if ! ANYSSH_X11_WINDOW_MATCH="Encrypt exported private key" \
  "$DRIVER" probe "$RUN_DIR/10g-private-key-export-passphrase-retry.bmp" >/dev/null 2>&1; then
  echo "The native export Passphrase retry prompt did not appear." >&2
  exit 1
fi
"$DRIVER" type "$GENERATED_EXPORT_PASSPHRASE"
"$DRIVER" tab
"$DRIVER" type "$GENERATED_EXPORT_PASSPHRASE"
"$DRIVER" enter

EXPORT_READY=0
for _ in $(seq 1 60); do
  if [[ -f "$GENERATED_EXPORT_PATH" ]]; then
    EXPORT_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$EXPORT_READY" -ne 1 ]]; then
  echo "The encrypted generated Private Key export was not created." >&2
  "$DRIVER" probe "$RUN_DIR/failed-private-key-export.bmp" >/dev/null || true
  exit 1
fi
if ssh-keygen \
  -y \
  -P "$WRONG_GENERATED_EXPORT_PASSPHRASE" \
  -f "$GENERATED_EXPORT_PATH" >/dev/null 2>&1; then
  echo "The exported Private Key accepted the wrong Passphrase." >&2
  exit 1
fi
ssh-keygen \
  -y \
  -P "$GENERATED_EXPORT_PASSPHRASE" \
  -f "$GENERATED_EXPORT_PATH" \
  >"$GENERATED_EXPORT_PUBLIC_PATH"
docker exec -i "$CONTAINER_NAME" \
  sh -c 'cat >> /home/anyssh/.ssh/authorized_keys' \
  <"$GENERATED_EXPORT_PUBLIC_PATH"
docker exec "$CONTAINER_NAME" \
  sh -c 'chown anyssh:anyssh /home/anyssh/.ssh/authorized_keys && chmod 600 /home/anyssh/.ssh/authorized_keys'
cp "$GENERATED_EXPORT_PATH" "$GENERATED_REIMPORT_PATH"
chmod 600 "$GENERATED_REIMPORT_PATH"
"$DRIVER" probe "$RUN_DIR/10h-private-key-exported.bmp" >/dev/null

"$DRIVER" click 1060 145
sleep 1
"$DRIVER" type "Native reimported key"
"$DRIVER" tab
"$DRIVER" type "anyssh"
"$DRIVER" tab
"$DRIVER" enter
REIMPORT_PICKER_READY=0
for _ in $(seq 1 40); do
  if ANYSSH_X11_WINDOW_MATCH="Import SSH private key" \
    "$DRIVER" probe >/dev/null 2>&1; then
    REIMPORT_PICKER_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$REIMPORT_PICKER_READY" -ne 1 ]]; then
  echo "The generated Key reimport picker did not appear." >&2
  "$DRIVER" probe "$RUN_DIR/failed-generated-key-reimport-picker.bmp" >/dev/null || true
  exit 1
fi
"$DRIVER" ctrl-l
sleep 0.5
"$DRIVER" type "/tmp"
"$DRIVER" enter
sleep 0.5
"$DRIVER" enter
sleep 2
"$DRIVER" enter

REIMPORT_PASSPHRASE_READY=0
for _ in $(seq 1 40); do
  if ANYSSH_X11_WINDOW_MATCH="Unlock SSH private key" \
    "$DRIVER" probe >/dev/null 2>&1; then
    REIMPORT_PASSPHRASE_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$REIMPORT_PASSPHRASE_READY" -ne 1 ]]; then
  echo "The generated Key reimport Passphrase prompt did not appear." >&2
  "$DRIVER" probe "$RUN_DIR/failed-generated-key-reimport-passphrase.bmp" >/dev/null || true
  exit 1
fi
ANYSSH_X11_WINDOW_MATCH="Unlock SSH private key" \
  "$DRIVER" probe "$RUN_DIR/10i-generated-key-reimport-passphrase.bmp" >/dev/null
"$DRIVER" type "$GENERATED_EXPORT_PASSPHRASE"
"$DRIVER" click 750 452
sleep 5
"$DRIVER" probe "$RUN_DIR/10j-generated-key-reimported.bmp" >/dev/null

rm -f \
  "$GENERATED_EXPORT_PATH" \
  "$GENERATED_EXPORT_PUBLIC_PATH" \
  "$GENERATED_REIMPORT_PATH"
for deleted_source in \
  "$GENERATED_EXPORT_PATH" \
  "$GENERATED_EXPORT_PUBLIC_PATH" \
  "$GENERATED_REIMPORT_PATH"; do
  if [[ -e "$deleted_source" ]]; then
    echo "A generated Private Key export/reimport source was not deleted." >&2
    exit 1
  fi
done

"$DRIVER" click 700 145
sleep 1
"$DRIVER" type "Native generated RSA"
"$DRIVER" tab
"$DRIVER" type "anyssh"
"$DRIVER" tab
"$DRIVER" down
"$DRIVER" tab
"$DRIVER" enter
# RSA 4096 generation is intentionally CPU-bound and can exceed a few seconds
# on shared CI runners. Wait long enough for the modal to close before choosing
# the generated Credential's Public Key action.
sleep 30
"$DRIVER" click 1025 371
sleep 2
"$DRIVER" probe "$RUN_DIR/10l-generated-rsa-public-key.bmp" >/dev/null
"$DRIVER" click 878 198
sleep 1

for marker in \
  "BEGIN OPENSSH PRIVATE KEY" \
  "246810" \
  "000000" \
  "$GENERATED_EXPORT_PATH" \
  "$GENERATED_EXPORT_PASSPHRASE" \
  "$WRONG_GENERATED_EXPORT_PASSPHRASE" \
  "Native generated key" \
  "Native generated RSA"; do
  if grep -R -a -F "$marker" "$VAULT_ROOT" >/dev/null 2>&1 \
    || grep -a -F "$marker" "$RUN_DIR/native.log" >/dev/null 2>&1; then
    echo "Generated/exported Private Key material leaked into Vault or log." >&2
    exit 1
  fi
done

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
scroll_connection_panel_top
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

# Configure the visible Local form, then Tab from Destination port to Start.
"$DRIVER" click 1200 704
"$DRIVER" ctrl-a
"$DRIVER" type "$LOCAL_FORWARD_PORT"
"$DRIVER" click 1060 775
"$DRIVER" ctrl-a
"$DRIVER" type "127.0.0.1"
"$DRIVER" click 1200 775
"$DRIVER" ctrl-a
"$DRIVER" type "$FORWARD_ECHO_PORT"
"$DRIVER" tab
"$DRIVER" enter
sleep 1

# Rewind to Kind, switch Local -> Dynamic, then set the Dynamic bind port.
for _ in $(seq 1 5); do
  "$DRIVER" shift-tab
done
"$DRIVER" down
"$DRIVER" down
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" ctrl-a
"$DRIVER" type "$DYNAMIC_FORWARD_PORT"
"$DRIVER" tab
"$DRIVER" enter
sleep 1

# Rewind to Kind, switch Dynamic -> Remote, and configure the local destination.
"$DRIVER" shift-tab
"$DRIVER" shift-tab
"$DRIVER" shift-tab
"$DRIVER" up
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" ctrl-a
"$DRIVER" type "$REMOTE_FORWARD_PORT"
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" ctrl-a
"$DRIVER" type "$REMOTE_DESTINATION_PORT"
"$DRIVER" tab
"$DRIVER" enter
sleep 2
"$DRIVER" probe "$RUN_DIR/14a-port-forwarding.bmp" >/dev/null

for port in "$LOCAL_FORWARD_PORT" "$DYNAMIC_FORWARD_PORT"; do
  if ! ss -ltn 2>/dev/null |
    grep -Eq "127\\.0\\.0\\.1:${port}[[:space:]]"; then
    echo "The native Forward listener on port $port was not created." >&2
    "$DRIVER" probe "$RUN_DIR/failed-port-forward-listener.bmp" >/dev/null || true
    exit 1
  fi
done
if ! docker exec "$CONTAINER_NAME" \
  nc -z -w 1 127.0.0.1 "$REMOTE_FORWARD_PORT" >/dev/null 2>&1; then
  echo "The Remote Forward listener was not registered on the SSH Server." >&2
  exit 1
fi

python3 - "$LOCAL_FORWARD_PORT" "$LOCAL_FORWARD_MARKER" <<'PY'
import socket
import sys

port = int(sys.argv[1])
payload = (sys.argv[2] + "\n").encode()
with socket.create_connection(("127.0.0.1", port), timeout=5) as stream:
    stream.sendall(payload)
    stream.shutdown(socket.SHUT_WR)
    echoed = b""
    while True:
        chunk = stream.recv(65536)
        if not chunk:
            break
        echoed += chunk
if echoed != payload:
    raise SystemExit("Local Forward payload mismatch")
PY

python3 - \
  "$DYNAMIC_FORWARD_PORT" \
  "$FORWARD_ECHO_PORT" \
  "$DYNAMIC_FORWARD_MARKER" <<'PY'
import socket
import sys

proxy_port = int(sys.argv[1])
destination_port = int(sys.argv[2])
payload = (sys.argv[3] + "\n").encode()
with socket.create_connection(("127.0.0.1", proxy_port), timeout=5) as stream:
    stream.sendall(b"\x05\x01\x00")
    if stream.recv(2) != b"\x05\x00":
        raise SystemExit("SOCKS5 method negotiation failed")
    stream.sendall(
        b"\x05\x01\x00\x01\x7f\x00\x00\x01"
        + destination_port.to_bytes(2, "big")
    )
    reply = b""
    while len(reply) < 10:
        chunk = stream.recv(10 - len(reply))
        if not chunk:
            raise SystemExit("SOCKS5 CONNECT reply was truncated")
        reply += chunk
    if reply[1] != 0:
        raise SystemExit("SOCKS5 CONNECT was rejected")
    stream.sendall(payload)
    stream.shutdown(socket.SHUT_WR)
    echoed = b""
    while True:
        chunk = stream.recv(65536)
        if not chunk:
            break
        echoed += chunk
if echoed != payload:
    raise SystemExit("Dynamic Forward payload mismatch")
PY

python3 - \
  "$REMOTE_DESTINATION_PORT" \
  "$REMOTE_FORWARD_MARKER" \
  >"$RUN_DIR/remote-forward-server.log" 2>&1 <<'PY' &
import socket
import sys

port = int(sys.argv[1])
expected = (sys.argv[2] + "\n").encode()
with socket.socket() as listener:
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(("127.0.0.1", port))
    listener.listen(1)
    connection, _ = listener.accept()
    with connection:
        received = b""
        while not received.endswith(b"\n") and len(received) < 4096:
            chunk = connection.recv(4096)
            if not chunk:
                break
            received += chunk
        if received != expected:
            raise SystemExit("Remote Forward payload mismatch")
        connection.sendall(b"ANYSSH_REMOTE_FORWARD_OK\n")
PY
FORWARD_SERVER_PID=$!
for _ in $(seq 1 50); do
  if ss -ltn 2>/dev/null |
    grep -Eq "127\\.0\\.0\\.1:${REMOTE_DESTINATION_PORT}[[:space:]]"; then
    break
  fi
  sleep 0.1
done
REMOTE_FORWARD_RESPONSE="$(
  printf '%s\n' "$REMOTE_FORWARD_MARKER" |
    docker exec -i "$CONTAINER_NAME" \
      nc -w 5 127.0.0.1 "$REMOTE_FORWARD_PORT" || true
)"
if [[ "$REMOTE_FORWARD_RESPONSE" != "ANYSSH_REMOTE_FORWARD_OK" ]]; then
  echo "The native Remote Forward did not reach the local destination." >&2
  exit 1
fi
wait "$FORWARD_SERVER_PID"
FORWARD_SERVER_PID=""

for marker in \
  "$LOCAL_FORWARD_MARKER" \
  "$DYNAMIC_FORWARD_MARKER" \
  "$REMOTE_FORWARD_MARKER" \
  "$GENERATED_EXPORT_PASSPHRASE" \
  "$WRONG_GENERATED_EXPORT_PASSPHRASE"; do
  if grep -R -a -F "$marker" "$VAULT_ROOT" >/dev/null 2>&1 \
    || grep -a -F "$marker" "$RUN_DIR/native.log" >/dev/null 2>&1; then
    echo "A Port Forward payload marker leaked into Vault or native logs." >&2
    exit 1
  fi
done

"$DRIVER" click 1117 44
sleep 1
"$DRIVER" probe "$RUN_DIR/15-disconnected.bmp" >/dev/null
for port in "$LOCAL_FORWARD_PORT" "$DYNAMIC_FORWARD_PORT"; do
  if ss -ltn 2>/dev/null |
    grep -Eq "127\\.0\\.0\\.1:${port}[[:space:]]"; then
    echo "Disconnect left the native Forward listener on port $port open." >&2
    exit 1
  fi
done
if docker exec "$CONTAINER_NAME" \
  nc -z -w 1 127.0.0.1 "$REMOTE_FORWARD_PORT" >/dev/null 2>&1; then
  echo "Disconnect left the Remote Forward registration open." >&2
  exit 1
fi
scroll_connection_panel_top

docker exec "$CONTAINER_NAME" rm -f /tmp/anyssh-native-trusted-ok
TRUSTED_RECONNECT_SUCCEEDED=0
for attempt in 1 2 3; do
  "$DRIVER" click 1100 440
  "$DRIVER" ctrl-a
  "$DRIVER" type "anyssh-test"
  sleep 0.5
  if [[ "$attempt" -eq 1 ]]; then
    "$DRIVER" click 1100 495
  else
    "$DRIVER" click 1100 540
  fi
  sleep 3
  "$DRIVER" click 500 260
  "$DRIVER" type "touch /tmp/anyssh-native-trusted-ok"
  "$DRIVER" enter
  for _ in $(seq 1 12); do
    if docker exec "$CONTAINER_NAME" \
      test -f /tmp/anyssh-native-trusted-ok >/dev/null 2>&1; then
      TRUSTED_RECONNECT_SUCCEEDED=1
      break
    fi
    sleep 0.25
  done
  if [[ "$TRUSTED_RECONNECT_SUCCEEDED" -eq 1 ]]; then
    break
  fi
done
if [[ "$TRUSTED_RECONNECT_SUCCEEDED" -ne 1 ]]; then
  echo "The durably trusted Endpoint prompted again or did not reconnect." >&2
  "$DRIVER" probe "$RUN_DIR/failed-trusted-reconnect.bmp" >/dev/null || true
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/16-durable-tofu-reconnect.bmp" >/dev/null

docker exec "$CONTAINER_NAME" rm -f \
  /tmp/anyssh-native-tab-two-ok \
  /tmp/anyssh-native-tab-inactive-ok \
  /tmp/anyssh-native-tab-after-close-ok
"$DRIVER" click 912 128
sleep 1
scroll_connection_panel_top
"$DRIVER" click 1100 220
sleep 0.5
"$DRIVER" ctrl-a
"$DRIVER" type "Native second"
sleep 0.25
"$DRIVER" click 1100 440
sleep 0.5
"$DRIVER" type "anyssh-test"
sleep 0.5
"$DRIVER" click 1100 495
sleep 3
"$DRIVER" click 500 280
sleep 0.5
"$DRIVER" type " touch /tmp/anyssh-native-tab-two-ok"
"$DRIVER" enter

TAB_TWO_CONNECTED=0
for _ in $(seq 1 20); do
  if docker exec "$CONTAINER_NAME" \
    test -f /tmp/anyssh-native-tab-two-ok >/dev/null 2>&1; then
    TAB_TWO_CONNECTED=1
    break
  fi
  sleep 0.25
done
if [[ "$TAB_TWO_CONNECTED" -ne 1 ]]; then
  echo "The second native Session Tab did not connect independently." >&2
  "$DRIVER" probe "$RUN_DIR/failed-multi-tab-connect.bmp" >/dev/null || true
  exit 1
fi

"$DRIVER" click 1200 704
"$DRIVER" ctrl-a
"$DRIVER" type "$TAB_CLOSE_FORWARD_PORT"
"$DRIVER" click 1060 775
"$DRIVER" ctrl-a
"$DRIVER" type "127.0.0.1"
"$DRIVER" click 1200 775
"$DRIVER" ctrl-a
"$DRIVER" type "$FORWARD_ECHO_PORT"
"$DRIVER" tab
"$DRIVER" enter
sleep 1
if ! ss -ltn 2>/dev/null |
  grep -Eq "127\\.0\\.0\\.1:${TAB_CLOSE_FORWARD_PORT}[[:space:]]"; then
  echo "The second Tab did not start its Session-scoped Local Forward." >&2
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/16a-multi-tab-connected.bmp" >/dev/null

"$DRIVER" click 365 128
sleep 0.5
"$DRIVER" click 500 280
sleep 0.5
"$DRIVER" type \
  " head -c 4194304 /dev/zero; touch /tmp/anyssh-native-tab-inactive-ok"
"$DRIVER" enter
sleep 0.15
"$DRIVER" click 555 128

INACTIVE_TAB_OUTPUT_SUCCEEDED=0
for _ in $(seq 1 120); do
  if docker exec "$CONTAINER_NAME" \
    test -f /tmp/anyssh-native-tab-inactive-ok >/dev/null 2>&1; then
    INACTIVE_TAB_OUTPUT_SUCCEEDED=1
    break
  fi
  sleep 0.5
done
if [[ "$INACTIVE_TAB_OUTPUT_SUCCEEDED" -ne 1 ]]; then
  echo "The inactive native Session Tab did not drain its 4 MiB output." >&2
  "$DRIVER" probe "$RUN_DIR/failed-inactive-tab-output.bmp" >/dev/null || true
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/16b-inactive-tab-output.bmp" >/dev/null

"$DRIVER" click 648 128
sleep 1
if ss -ltn 2>/dev/null |
  grep -Eq "127\\.0\\.0\\.1:${TAB_CLOSE_FORWARD_PORT}[[:space:]]"; then
  echo "Closing the second Tab left its Local Forward listener open." >&2
  exit 1
fi
"$DRIVER" click 500 280
sleep 0.5
"$DRIVER" type " touch /tmp/anyssh-native-tab-after-close-ok"
"$DRIVER" enter

FIRST_TAB_SURVIVED_CLOSE=0
for _ in $(seq 1 20); do
  if docker exec "$CONTAINER_NAME" \
    test -f /tmp/anyssh-native-tab-after-close-ok >/dev/null 2>&1; then
    FIRST_TAB_SURVIVED_CLOSE=1
    break
  fi
  sleep 0.25
done
if [[ "$FIRST_TAB_SURVIVED_CLOSE" -ne 1 ]]; then
  echo "Closing the second native Session Tab affected the first Session." >&2
  "$DRIVER" probe "$RUN_DIR/failed-first-tab-after-close.bmp" >/dev/null || true
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/16c-first-tab-after-close.bmp" >/dev/null

"$DRIVER" click 1117 44
sleep 1

"$DRIVER" click 100 340
sleep 1
"$DRIVER" probe "$RUN_DIR/17-known-hosts.bmp" >/dev/null
"$DRIVER" click 1180 273

FORGET_DIALOG_READY=0
FORGET_DIALOG_LINE=""
for _ in $(seq 1 40); do
  FORGET_DIALOG_LINE="$(
    ANYSSH_X11_WINDOW_MATCH="Forget trusted host keys" \
      "$DRIVER" probe 2>/dev/null |
      grep -F "name=Forget trusted host keys" |
      head -n1 || true
  )"
  if [[ -n "$FORGET_DIALOG_LINE" ]]; then
    FORGET_DIALOG_READY=1
    break
  fi
  sleep 0.25
done
if [[ "$FORGET_DIALOG_READY" -ne 1 ]]; then
  echo "The native Forget Trust confirmation did not appear." >&2
  "$DRIVER" probe "$RUN_DIR/failed-known-host-confirmation.bmp" >/dev/null || true
  exit 1
fi
ANYSSH_X11_WINDOW_MATCH="Forget trusted host keys" \
  "$DRIVER" probe "$RUN_DIR/18-known-host-forget-confirmation.bmp" >/dev/null
if [[ "$FORGET_DIALOG_LINE" =~ geometry=([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+) ]]; then
  FORGET_DIALOG_WIDTH="${BASH_REMATCH[1]}"
  FORGET_DIALOG_HEIGHT="${BASH_REMATCH[2]}"
  FORGET_DIALOG_X="${BASH_REMATCH[3]}"
  FORGET_DIALOG_Y="${BASH_REMATCH[4]}"
else
  echo "The native Forget Trust dialog geometry was unavailable." >&2
  exit 1
fi
"$DRIVER" click \
  "$((FORGET_DIALOG_X + FORGET_DIALOG_WIDTH / 4))" \
  "$((FORGET_DIALOG_Y + FORGET_DIALOG_HEIGHT - 18))"
sleep 2
"$DRIVER" probe "$RUN_DIR/19-known-host-forgotten.bmp" >/dev/null

"$DRIVER" click 100 106
sleep 1
scroll_connection_panel_top
"$DRIVER" click 1100 440
"$DRIVER" type "anyssh-test"
"$DRIVER" click 1100 495
sleep 1
"$DRIVER" probe "$RUN_DIR/20-tofu-after-forget.bmp" >/dev/null
"$DRIVER" click 700 532
docker exec "$CONTAINER_NAME" rm -f /tmp/anyssh-native-retrusted-ok
RETRUSTED_CONNECTION_SUCCEEDED=0
for attempt in 1 2 3; do
  if [[ "$attempt" -gt 1 ]]; then
    "$DRIVER" click 1100 440
    "$DRIVER" ctrl-a
    "$DRIVER" type "anyssh-test"
    sleep 0.5
    "$DRIVER" click 1100 540
  fi
  sleep 3
  "$DRIVER" click 500 260
  "$DRIVER" type "touch /tmp/anyssh-native-retrusted-ok"
  "$DRIVER" enter
  for _ in $(seq 1 12); do
    if docker exec "$CONTAINER_NAME" \
      test -f /tmp/anyssh-native-retrusted-ok >/dev/null 2>&1; then
      RETRUSTED_CONNECTION_SUCCEEDED=1
      break
    fi
    sleep 0.25
  done
  if [[ "$RETRUSTED_CONNECTION_SUCCEEDED" -eq 1 ]]; then
    break
  fi
done
if [[ "$RETRUSTED_CONNECTION_SUCCEEDED" -ne 1 ]]; then
  echo "The connection did not resume after TOFU was re-established." >&2
  "$DRIVER" probe "$RUN_DIR/failed-retrusted-connection.bmp" >/dev/null || true
  exit 1
fi
"$DRIVER" click 1117 44
sleep 1

docker exec "$CONTAINER_NAME" sh -c \
  'rm -f /etc/ssh/ssh_host_* && ssh-keygen -A >/dev/null 2>&1 && kill -HUP 1'
for _ in $(seq 1 50); do
  if ssh-keyscan -p 2222 127.0.0.1 >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
if ! ssh-keyscan -p 2222 127.0.0.1 >/dev/null 2>&1; then
  echo "The rotated OpenSSH fixture did not become ready." >&2
  exit 1
fi
docker exec "$CONTAINER_NAME" rm -f /tmp/anyssh-native-rotation-bypass
scroll_connection_panel_top
"$DRIVER" click 1100 495
sleep 2
"$DRIVER" probe "$RUN_DIR/21-changed-host-key.bmp" >/dev/null
"$DRIVER" click 500 260
"$DRIVER" type "touch /tmp/anyssh-native-rotation-bypass"
"$DRIVER" enter
sleep 1
if docker exec "$CONTAINER_NAME" \
  test -f /tmp/anyssh-native-rotation-bypass >/dev/null 2>&1; then
  echo "The rotated Host Key bypassed the hard-block dialog." >&2
  exit 1
fi
"$DRIVER" click 455 590
sleep 1

scroll_connection_panel_top
"$DRIVER" click 1100 440
"$DRIVER" shift-tab
"$DRIVER" shift-tab
"$DRIVER" shift-tab
"$DRIVER" ctrl-a
"$DRIVER" type "2223"
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" down
"$DRIVER" tab
"$DRIVER" enter
sleep 2
"$DRIVER" probe "$RUN_DIR/22-interactive-host-key.bmp" >/dev/null
"$DRIVER" click 700 532
sleep 2
"$DRIVER" probe "$RUN_DIR/23-interactive-challenge.bmp" >/dev/null
"$DRIVER" type "$INTERACTIVE_RESPONSE"
"$DRIVER" enter

docker exec "$PAM_CONTAINER_NAME" rm -f /tmp/anyssh-native-interactive-ok
INTERACTIVE_SUCCEEDED=0
for _ in $(seq 1 40); do
  "$DRIVER" click 500 260
  "$DRIVER" type "touch /tmp/anyssh-native-interactive-ok"
  "$DRIVER" enter
  sleep 0.5
  if docker exec "$PAM_CONTAINER_NAME" \
    test -f /tmp/anyssh-native-interactive-ok >/dev/null 2>&1; then
    INTERACTIVE_SUCCEEDED=1
    break
  fi
done
if [[ "$INTERACTIVE_SUCCEEDED" -ne 1 ]]; then
  echo "The native Keyboard-interactive response did not reach OpenSSH PAM." >&2
  "$DRIVER" probe "$RUN_DIR/failed-interactive-authentication.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/native.log" >&2
  exit 1
fi
if grep -R -a -F "$INTERACTIVE_RESPONSE" "$VAULT_ROOT" >/dev/null 2>&1 \
  || grep -a -F "$INTERACTIVE_RESPONSE" "$RUN_DIR/native.log" >/dev/null 2>&1; then
  echo "The Keyboard-interactive response leaked into native evidence or Vault files." >&2
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/24-interactive-connected.bmp" >/dev/null

docker exec "$PAM_CONTAINER_NAME" rm -f /tmp/anyssh-native-interactive-lock-ok
"$DRIVER" click 912 128
sleep 1
scroll_connection_panel_top
"$DRIVER" click 1100 220
sleep 0.5
"$DRIVER" ctrl-a
"$DRIVER" type "Native lock companion"
sleep 0.25
"$DRIVER" click 1100 440
sleep 0.5
"$DRIVER" shift-tab
"$DRIVER" shift-tab
"$DRIVER" shift-tab
"$DRIVER" ctrl-a
"$DRIVER" type "2223"
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" down
"$DRIVER" tab
"$DRIVER" enter
sleep 2
"$DRIVER" probe "$RUN_DIR/25-multi-tab-interactive-challenge.bmp" >/dev/null
"$DRIVER" type "$INTERACTIVE_RESPONSE"
"$DRIVER" enter

LOCK_COMPANION_CONNECTED=0
for _ in $(seq 1 40); do
  "$DRIVER" click 500 260
  sleep 0.25
  "$DRIVER" type " touch /tmp/anyssh-native-interactive-lock-ok"
  "$DRIVER" enter
  sleep 0.5
  if docker exec "$PAM_CONTAINER_NAME" \
    test -f /tmp/anyssh-native-interactive-lock-ok >/dev/null 2>&1; then
    LOCK_COMPANION_CONNECTED=1
    break
  fi
done
if [[ "$LOCK_COMPANION_CONNECTED" -ne 1 ]]; then
  echo "The second Session did not connect before the multi-Tab Vault lock." >&2
  "$DRIVER" probe "$RUN_DIR/failed-multi-tab-vault-lock.bmp" >/dev/null || true
  exit 1
fi

"$DRIVER" click 1200 740
"$DRIVER" ctrl-a
"$DRIVER" type "$VAULT_LOCK_FORWARD_PORT"
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" enter
sleep 1
if ! ss -ltn 2>/dev/null |
  grep -Eq "127\\.0\\.0\\.1:${VAULT_LOCK_FORWARD_PORT}[[:space:]]"; then
  echo "The Keyboard-interactive Tab did not start its Local Forward." >&2
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/25a-multi-tab-before-vault-lock.bmp" >/dev/null
"$DRIVER" click 1208 44
sleep 1
"$DRIVER" probe "$RUN_DIR/26-vault-locked-after-session.bmp" >/dev/null
if ss -ltn 2>/dev/null |
  grep -Eq "127\\.0\\.0\\.1:${VAULT_LOCK_FORWARD_PORT}[[:space:]]"; then
  echo "Vault Lock left a Session Forward listener open." >&2
  exit 1
fi
if grep -R -a -F "$INTERACTIVE_RESPONSE" "$VAULT_ROOT" >/dev/null 2>&1 \
  || grep -a -F "$INTERACTIVE_RESPONSE" "$RUN_DIR/native.log" >/dev/null 2>&1; then
  echo "The repeated Keyboard-interactive response leaked during Vault lock." >&2
  exit 1
fi

if ! kill -0 "$APP_GROUP" >/dev/null 2>&1; then
  echo "The native process exited unexpectedly." >&2
  exit 1
fi
for marker in \
  "$LOCAL_FORWARD_MARKER" \
  "$DYNAMIC_FORWARD_MARKER" \
  "$REMOTE_FORWARD_MARKER"; do
  if grep -R -a -F "$marker" "$RUN_DIR" >/dev/null 2>&1; then
    echo "A Port Forward payload marker leaked into native evidence." >&2
    exit 1
  fi
done

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
- The same native UI generated Ed25519 and RSA 4096 Credentials in Rust and
  displayed only their Public Key Algorithm, SHA-256 Fingerprint, and OpenSSH
  Public Key in WebView.
- Encrypted Export used a Rust-owned Native Save Picker, one wrong/correct
  GTK PIN Step-up, and one mismatched/correct new Passphrase confirmation.
  \`ssh-keygen -y\` rejected the wrong Passphrase and derived the expected
  Public Key with the accepted Passphrase.
- The exported Key was reimported through the Native Picker and GTK Secure
  Entry using the new Passphrase, then the exported source file was deleted.
- Generated Private Key material, PIN, Export Passphrases, and the full export
  Path remained absent from Vault plaintext and the native log.
- The same native UI enumerated one \`SSH_AUTH_SOCK\` Identity and created a
  Fingerprint-selected System Agent Credential without persisting the Agent Key
  or Fingerprint in plaintext.
- Password input reached the native WebView.
- Host-key confirmation displayed the scoped endpoint and SHA-256 fingerprint and was accepted.
- The Rust SSH core authenticated against the Docker OpenSSH fixture.
- X11 keyboard events reached xterm.js and created \`/tmp/anyssh-native-ok\` remotely.
- A 4 MiB terminal stream drained through Tauri/xterm acknowledgement backpressure and
  created \`/tmp/anyssh-native-large-ok\` after the output completed.
- The real native UI started Local, unauthenticated Dynamic SOCKS5, and Remote
  Loopback Forwards. External TCP clients crossed the owning russh Session in
  both directions while payload markers stayed absent from IPC logs, Vault, and
  evidence.
- Disconnect removed the Local/Dynamic listeners and Remote registration.
- Disconnect returned the UI to the disconnected state.
- A second connection to the same Endpoint used durable Trust without another
  Host Key prompt.
- Two native Session Tabs connected independently. The first Tab drained a
  4 MiB stream while inactive, and closing the second Tab left the first
  Session connected and accepting a follow-up command. Closing the second Tab
  also removed its Session-scoped Local Forward.
- Known Hosts displayed metadata-only Trust, required a native GTK confirmation
  to forget it, and the next connection required TOFU again.
- Rotating the same OpenSSH Endpoint produced a hard-block dialog and no remote
  command could run through it.
- The Quick Connection selector started a real RFC 4256 Keyboard-interactive
  session against OpenSSH PAM on a separate Endpoint.
- The masked Challenge response authenticated, created
  \`/tmp/anyssh-native-interactive-ok\`, and remained absent from Vault files
  and the native log.
- A second Keyboard-interactive Tab connected concurrently, then Lock Vault
  drained both Sessions, removed their Tabs and active Forward listener, and
  returned to the PIN gate.

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
- \`10a-generated-private-key.bmp\`
- \`10b-generated-public-key.bmp\`
- \`10c-private-key-export-picker.bmp\`
- \`10d-private-key-export-pin.bmp\`
- \`10e-private-key-export-pin-retry.bmp\`
- \`10f-private-key-export-passphrase.bmp\`
- \`10g-private-key-export-passphrase-retry.bmp\`
- \`10h-private-key-exported.bmp\`
- \`10i-generated-key-reimport-passphrase.bmp\`
- \`10j-generated-key-reimported.bmp\`
- \`10l-generated-rsa-public-key.bmp\`
- \`11-system-agent-created.bmp\`
- \`12-password-entered.bmp\`
- \`13-host-key-dialog.bmp\`
- \`14-command-succeeded.bmp\`
- \`14a-port-forwarding.bmp\`
- \`15-disconnected.bmp\`
- \`16-durable-tofu-reconnect.bmp\`
- \`16a-multi-tab-connected.bmp\`
- \`16b-inactive-tab-output.bmp\`
- \`16c-first-tab-after-close.bmp\`
- \`17-known-hosts.bmp\`
- \`18-known-host-forget-confirmation.bmp\`
- \`19-known-host-forgotten.bmp\`
- \`20-tofu-after-forget.bmp\`
- \`21-changed-host-key.bmp\`
- \`22-interactive-host-key.bmp\`
- \`23-interactive-challenge.bmp\`
- \`24-interactive-connected.bmp\`
- \`25-multi-tab-interactive-challenge.bmp\`
- \`25a-multi-tab-before-vault-lock.bmp\`
- \`26-vault-locked-after-session.bmp\`
- \`windows.txt\`
- \`native.log\`
EOF

echo "Native Xvfb SSH smoke passed: $RUN_DIR"
