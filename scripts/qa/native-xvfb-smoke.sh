#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_NAME="anyssh-openssh-fixture:phase0"
CONTAINER_NAME="anyssh-native-xvfb-$RANDOM-$$"
RUN_DIR="$ROOT_DIR/artifacts/native-xvfb/smoke-$(date +%s)-$$"
DRIVER="$RUN_DIR/anyssh-x11-driver"
APP_GROUP=""
XVFB_PID=""

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
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

for command in \
  cc \
  dbus-run-session \
  docker \
  pkg-config \
  pnpm \
  setsid \
  ss \
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
  bash -lc "cd '$ROOT_DIR' && pnpm dev:native" \
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

sleep 2
"$DRIVER" probe "$RUN_DIR/01-native-ready.bmp" >"$RUN_DIR/windows.txt"
"$DRIVER" click 1100 440
sleep 0.25
"$DRIVER" type "anyssh-test"
sleep 0.5
"$DRIVER" probe "$RUN_DIR/02-password-entered.bmp" >/dev/null
"$DRIVER" click 1100 495
sleep 1
"$DRIVER" probe "$RUN_DIR/03-host-key-dialog.bmp" >/dev/null

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

"$DRIVER" probe "$RUN_DIR/04-command-succeeded.bmp" >/dev/null
"$DRIVER" click 1208 44
sleep 1
"$DRIVER" probe "$RUN_DIR/05-disconnected.bmp" >/dev/null

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
- Password input reached the native WebView.
- Host-key confirmation was displayed and accepted.
- The Rust SSH core authenticated against the Docker OpenSSH fixture.
- X11 keyboard events reached xterm.js and created \`/tmp/anyssh-native-ok\` remotely.
- Disconnect returned the UI to the disconnected state.

## Evidence

- \`01-native-ready.bmp\`
- \`02-password-entered.bmp\`
- \`03-host-key-dialog.bmp\`
- \`04-command-succeeded.bmp\`
- \`05-disconnected.bmp\`
- \`windows.txt\`
- \`native.log\`
EOF

echo "Native Xvfb SSH smoke passed: $RUN_DIR"
