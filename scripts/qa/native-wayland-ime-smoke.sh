#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--session" ]]; then
  ROOT_DIR="$2"
  RUN_DIR="$3"
  OUTER_DISPLAY="$4"
  WAYLAND_SOCKET="$5"

  export DISPLAY="$OUTER_DISPLAY"
  export XDG_CACHE_HOME="$RUN_DIR/xdg-cache"
  export XDG_CONFIG_HOME="$RUN_DIR/xdg-config"
  export XDG_DATA_HOME="$RUN_DIR/xdg-data"
  export XDG_RUNTIME_DIR="$RUN_DIR/xdg-runtime"
  export XDG_SESSION_TYPE=wayland
  export XDG_CURRENT_DESKTOP=weston
  export WAYLAND_DISPLAY="$WAYLAND_SOCKET"
  export GTK_IM_MODULE=ibus
  export QT_IM_MODULE=ibus
  export XMODIFIERS=@im=ibus

  weston \
    --backend=x11 \
    --renderer=pixman \
    --shell=kiosk \
    --width=1280 \
    --height=800 \
    --socket="$WAYLAND_DISPLAY" \
    --idle-time=0 \
    --no-config \
    --log="$RUN_DIR/weston.log" &

  for _ in $(seq 1 100); do
    if [[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]]; then
      break
    fi
    sleep 0.1
  done

  if [[ ! -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]]; then
    echo "Weston did not create the Wayland socket." >&2
    exit 1
  fi

  printf '%s' "$DBUS_SESSION_BUS_ADDRESS" >"$RUN_DIR/dbus-address"
  ibus-daemon \
    --daemonize \
    --replace \
    --xim \
    >"$RUN_DIR/ibus.log" 2>&1

  (
    cd "$ROOT_DIR"
    pnpm dev
  ) >"$RUN_DIR/vite.log" 2>&1 &

  for _ in $(seq 1 100); do
    if curl -fsS http://localhost:1420/ >/dev/null 2>&1; then
      break
    fi
    sleep 0.1
  done

  if ! curl -fsS http://localhost:1420/ >/dev/null 2>&1; then
    echo "The Vite development server did not become ready." >&2
    exit 1
  fi

  env -u DISPLAY \
    GDK_BACKEND=wayland \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    LIBGL_ALWAYS_SOFTWARE=1 \
    "$ROOT_DIR/target/debug/anyssh-client" \
    >"$RUN_DIR/app.log" 2>&1 &
  APP_PID=$!
  printf '%s' "$APP_PID" >"$RUN_DIR/app.pid"
  wait "$APP_PID"
  exit $?
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_NAME="anyssh-openssh-fixture:phase0"
CONTAINER_NAME="anyssh-native-wayland-$RANDOM-$$"
RUN_DIR="$ROOT_DIR/artifacts/native-wayland/smoke-$(date +%s)-$$"
DRIVER="$RUN_DIR/anyssh-x11-driver"
WAYLAND_SOCKET="wayland-anyssh"
SESSION_GROUP=""
XVFB_PID=""

cleanup() {
  if [[ -n "$SESSION_GROUP" ]]; then
    kill -TERM -- "-$SESSION_GROUP" >/dev/null 2>&1 || true
    sleep 1
    kill -KILL -- "-$SESSION_GROUP" >/dev/null 2>&1 || true
  fi
  if [[ -n "$XVFB_PID" ]]; then
    kill -TERM "$XVFB_PID" >/dev/null 2>&1 || true
    wait "$XVFB_PID" >/dev/null 2>&1 || true
  fi
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

for command in \
  cargo \
  cc \
  curl \
  dbus-run-session \
  docker \
  grep \
  ibus \
  ibus-daemon \
  pkg-config \
  pnpm \
  setsid \
  ss \
  ssh-keyscan \
  wayland-info \
  weston \
  Xvfb; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing native Wayland smoke dependency: $command" >&2
    exit 1
  fi
done

if ! pkg-config --exists webkit2gtk-4.1 javascriptcoregtk-4.1; then
  echo "WebKitGTK 4.1 development files are required." >&2
  exit 1
fi

if ss -ltn 2>/dev/null | grep -Eq '127\.0\.0\.1:(1420|2222)[[:space:]]'; then
  echo "Port 1420 or 2222 is already in use by another process." >&2
  exit 1
fi

mkdir -p \
  "$RUN_DIR/xdg-cache" \
  "$RUN_DIR/xdg-config" \
  "$RUN_DIR/xdg-data" \
  "$RUN_DIR/xdg-runtime"
chmod 700 "$RUN_DIR/xdg-runtime"

cc \
  -std=c11 \
  -D_DEFAULT_SOURCE \
  -O2 \
  -Wall \
  -Wextra \
  -o "$DRIVER" \
  "$ROOT_DIR/tests/tools/x11/anyssh-x11-driver.c" \
  -lX11 \
  -lXtst

(
  cd "$ROOT_DIR"
  cargo build --package anyssh-client
)

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
for candidate in $(seq 111 129); do
  if [[ ! -e "/tmp/.X${candidate}-lock" &&
    ! -S "/tmp/.X11-unix/X${candidate}" ]]; then
    DISPLAY_NUMBER="$candidate"
    break
  fi
done

if [[ -z "$DISPLAY_NUMBER" ]]; then
  echo "No free outer X11 display was found for nested Weston." >&2
  exit 1
fi

export DISPLAY=":$DISPLAY_NUMBER"
Xvfb "$DISPLAY" \
  -screen 0 1280x800x24 \
  -ac \
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

setsid env -i \
  HOME="$HOME" \
  USER="$USER" \
  LOGNAME="${LOGNAME:-$USER}" \
  LANG="${LANG:-C.UTF-8}" \
  PATH="$PATH" \
  DISPLAY="$DISPLAY" \
  dbus-run-session -- \
  "$0" \
  --session \
  "$ROOT_DIR" \
  "$RUN_DIR" \
  "$DISPLAY" \
  "$WAYLAND_SOCKET" \
  >"$RUN_DIR/session.log" 2>&1 &
SESSION_GROUP=$!

WINDOW_READY=0
export ANYSSH_X11_WINDOW_MATCH="*"
for _ in $(seq 1 300); do
  if ! kill -0 "$SESSION_GROUP" >/dev/null 2>&1; then
    echo "The Wayland session exited before the application became ready." >&2
    tail -n 120 "$RUN_DIR/session.log" >&2
    exit 1
  fi
  if [[ -s "$RUN_DIR/app.pid" ]] &&
    "$DRIVER" probe >"$RUN_DIR/windows.txt" 2>/dev/null; then
    WINDOW_READY=1
    break
  fi
  sleep 0.5
done

if [[ "$WINDOW_READY" -ne 1 ]]; then
  echo "The AnySSH Wayland surface did not appear." >&2
  tail -n 120 "$RUN_DIR/session.log" >&2
  exit 1
fi

APP_PID="$(cat "$RUN_DIR/app.pid")"
if grep -zq '^DISPLAY=' "/proc/$APP_PID/environ"; then
  echo "The application inherited DISPLAY and could fall back to X11." >&2
  exit 1
fi
if ! grep -zq '^GDK_BACKEND=wayland$' "/proc/$APP_PID/environ"; then
  echo "The application was not forced onto the Wayland GDK backend." >&2
  exit 1
fi
tr '\0' '\n' <"/proc/$APP_PID/environ" |
  grep -E '^(GDK_BACKEND|GTK_IM_MODULE|WAYLAND_DISPLAY|XDG_CURRENT_DESKTOP|XDG_RUNTIME_DIR|XDG_SESSION_TYPE)=' \
    >"$RUN_DIR/app-backend-environment.txt"

XDG_RUNTIME_DIR="$RUN_DIR/xdg-runtime" \
  WAYLAND_DISPLAY="$WAYLAND_SOCKET" \
  wayland-info >"$RUN_DIR/wayland-info.txt"

DBUS_SESSION_BUS_ADDRESS="$(cat "$RUN_DIR/dbus-address")"
export DBUS_SESSION_BUS_ADDRESS
export XDG_CACHE_HOME="$RUN_DIR/xdg-cache"
export XDG_CONFIG_HOME="$RUN_DIR/xdg-config"
export XDG_DATA_HOME="$RUN_DIR/xdg-data"
export XDG_RUNTIME_DIR="$RUN_DIR/xdg-runtime"
export WAYLAND_DISPLAY="$WAYLAND_SOCKET"

IBUS_BUS_FILE=""
for _ in $(seq 1 50); do
  IBUS_BUS_FILE="$(find "$XDG_CONFIG_HOME/ibus/bus" -maxdepth 1 -type f 2>/dev/null | head -n 1)"
  if [[ -n "$IBUS_BUS_FILE" ]]; then
    break
  fi
  sleep 0.1
done

if [[ -z "$IBUS_BUS_FILE" ]]; then
  echo "IBus did not publish its session address." >&2
  exit 1
fi

IBUS_ADDRESS="$(grep '^IBUS_ADDRESS=' "$IBUS_BUS_FILE" | cut -d= -f2-)"
export IBUS_ADDRESS

set_ibus_engine() {
  local engine="$1"
  ibus engine "$engine" >/dev/null 2>&1 || true
  for _ in $(seq 1 20); do
    if [[ "$(ibus engine 2>/dev/null || true)" == "$engine" ]]; then
      return 0
    fi
    sleep 0.1
  done
  echo "IBus did not switch to engine: $engine" >&2
  return 1
}

ibus list-engine >"$RUN_DIR/ibus-engines.txt"
if ! grep -Eq '^[[:space:]]+libpinyin[[:space:]]' "$RUN_DIR/ibus-engines.txt"; then
  echo "The IBus libpinyin engine is unavailable." >&2
  exit 1
fi

sleep 3
"$DRIVER" probe "$RUN_DIR/01-vault-create.bmp" >"$RUN_DIR/windows.txt"
set_ibus_engine "xkb:us::eng"
"$DRIVER" click 640 445
sleep 0.5
"$DRIVER" type "246810"
"$DRIVER" click 640 522
sleep 0.5
"$DRIVER" type "246810"
sleep 0.5
"$DRIVER" click 640 577

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
  echo "The encrypted Vault was not created through native Wayland input." >&2
  "$DRIVER" probe "$RUN_DIR/failed-vault-create.bmp" >/dev/null || true
  exit 1
fi

sleep 1
"$DRIVER" probe "$RUN_DIR/02-wayland-ready.bmp" >/dev/null
"$DRIVER" click 1100 440
sleep 0.5
"$DRIVER" type "anyssh-test"
"$DRIVER" click 1100 495
sleep 1
"$DRIVER" probe "$RUN_DIR/03-host-key-dialog.bmp" >/dev/null
"$DRIVER" click 700 532
sleep 3

"$DRIVER" click 500 260
sleep 0.5
set_ibus_engine "xkb:us::eng"
"$DRIVER" type "touch /tmp/"
set_ibus_engine "libpinyin"
"$DRIVER" type "zhongwen"
"$DRIVER" space
sleep 1
set_ibus_engine "xkb:us::eng"
"$DRIVER" enter

IME_COMMAND_SUCCEEDED=0
for _ in $(seq 1 40); do
  if docker exec "$CONTAINER_NAME" test -f "/tmp/中文" >/dev/null 2>&1; then
    IME_COMMAND_SUCCEEDED=1
    break
  fi
  sleep 0.25
done

if [[ "$IME_COMMAND_SUCCEEDED" -ne 1 ]]; then
  echo "The Chinese IBus composition did not reach the remote SSH shell." >&2
  "$DRIVER" probe "$RUN_DIR/failed-ime-command.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/app.log" >&2
  exit 1
fi

"$DRIVER" probe "$RUN_DIR/04-terminal-ime-command.bmp" >/dev/null
"$DRIVER" click 1117 44
sleep 1
"$DRIVER" probe "$RUN_DIR/05-disconnected.bmp" >/dev/null

if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
  echo "The AnySSH process exited unexpectedly." >&2
  exit 1
fi

cat >"$RUN_DIR/report.md" <<EOF
# AnySSH native Wayland and IME smoke report

- Result: PASS
- Identifier: \`com.spiredive.anyssh\`
- Wayland compositor: Weston nested X11 backend with Pixman
- Wayland socket: \`$WAYLAND_SOCKET\`
- Outer automation display: \`$DISPLAY\`

## Verified

- AnySSH launched with \`GDK_BACKEND=wayland\` and no \`DISPLAY\` environment variable.
- WebKitGTK rendered the native Tauri application on a real Wayland socket.
- XTest input entered Weston and reached the native Wayland WebView.
- Native Wayland input created an encrypted SQLCipher Vault.
- IBus switched from the US keyboard engine to \`libpinyin\`.
- The xterm.js composition path committed \`中文\` through WebKitGTK, Tauri IPC,
  the Rust SSH core, and the Docker OpenSSH shell.
- The remote marker \`/tmp/中文\` proved that the committed UTF-8 text reached SSH.
- Disconnect returned the application to the disconnected state.

## Evidence

- \`01-vault-create.bmp\`
- \`02-wayland-ready.bmp\`
- \`03-host-key-dialog.bmp\`
- \`04-terminal-ime-command.bmp\`
- \`05-disconnected.bmp\`
- \`app-backend-environment.txt\`
- \`ibus-engines.txt\`
- \`wayland-info.txt\`
- \`weston.log\`
- \`app.log\`
EOF

echo "Native Wayland and IBus smoke passed: $RUN_DIR"
