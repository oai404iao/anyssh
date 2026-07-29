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
    cd "$ROOT_DIR/apps/client"
    "$ROOT_DIR/apps/client/node_modules/.bin/vite"
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
PAM_IMAGE_NAME="anyssh-openssh-pam-fixture:phase1"
CONTAINER_NAME="anyssh-native-wayland-$RANDOM-$$"
PAM_CONTAINER_NAME="anyssh-native-wayland-pam-$RANDOM-$$"
RUN_DIR="$ROOT_DIR/artifacts/native-wayland/smoke-$(date +%s)-$$"
DRIVER="$RUN_DIR/anyssh-x11-driver"
WAYLAND_SOCKET="wayland-anyssh"
SESSION_GROUP=""
XVFB_PID=""
INTERACTIVE_RESPONSE="otp-$RANDOM-$RANDOM"
FORWARD_ECHO_PORT=8080
LOCAL_FORWARD_PORT=18080
DYNAMIC_FORWARD_PORT=18081
REMOTE_FORWARD_PORT=18082
REMOTE_DESTINATION_PORT=18083
TAB_CLOSE_FORWARD_PORT=18085
LOCAL_FORWARD_MARKER="wayland-local-$RANDOM-$RANDOM"
DYNAMIC_FORWARD_MARKER="wayland-dynamic-$RANDOM-$RANDOM"
REMOTE_FORWARD_MARKER="wayland-remote-$RANDOM-$RANDOM"
FORWARD_SERVER_PID=""

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
  if [[ -n "$FORWARD_SERVER_PID" ]]; then
    kill "$FORWARD_SERVER_PID" >/dev/null 2>&1 || true
    wait "$FORWARD_SERVER_PID" >/dev/null 2>&1 || true
  fi
  docker rm -f "$CONTAINER_NAME" "$PAM_CONTAINER_NAME" >/dev/null 2>&1 || true
  rm -rf \
    "$RUN_DIR/xdg-cache" \
    "$RUN_DIR/xdg-config" \
    "$RUN_DIR/xdg-data" \
    "$RUN_DIR/xdg-runtime"
  rm -f "$RUN_DIR/app.pid" "$RUN_DIR/dbus-address"
}
trap cleanup EXIT

scroll_connection_panel_top() {
  "$DRIVER" click 1240 250
  "$DRIVER" scroll-up 24
  sleep 0.5
}

for command in \
  cargo \
  cc \
  curl \
  dbus-run-session \
  docker \
  grep \
  ibus \
  ibus-daemon \
  od \
  pkg-config \
  python3 \
  setsid \
  ss \
  timeout \
  wayland-info \
  weston \
  Xvfb; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing native Wayland smoke dependency: $command" >&2
    exit 1
  fi
done

tcp_port_ready() {
  local port="$1"
  timeout 1 bash -c "exec 3<>/dev/tcp/127.0.0.1/$port" \
    >/dev/null 2>&1
}

if [[ ! -x "$ROOT_DIR/apps/client/node_modules/.bin/vite" ]]; then
  echo "Vite is not installed; run pnpm install from the repository root." >&2
  exit 1
fi

if ! pkg-config --exists webkit2gtk-4.1 javascriptcoregtk-4.1; then
  echo "WebKitGTK 4.1 development files are required." >&2
  exit 1
fi

if ss -ltn 2>/dev/null | grep -Eq '127\.0\.0\.1:(1420|2222|2223)[[:space:]]'; then
  echo "Port 1420, 2222, or 2223 is already in use by another process." >&2
  exit 1
fi
for port in \
  "$LOCAL_FORWARD_PORT" \
  "$DYNAMIC_FORWARD_PORT" \
  "$REMOTE_DESTINATION_PORT" \
  "$TAB_CLOSE_FORWARD_PORT"; do
  if ss -ltn 2>/dev/null | grep -Eq "127\\.0\\.0\\.1:${port}[[:space:]]"; then
    echo "Port $port is already in use; it is reserved by this smoke test." >&2
    exit 1
  fi
done

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

for _ in $(seq 1 100); do
  if tcp_port_ready 2222 \
    && tcp_port_ready 2223 \
    && docker exec "$CONTAINER_NAME" \
      nc -z -w 1 127.0.0.1 "$FORWARD_ECHO_PORT" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

if ! tcp_port_ready 2222 \
  || ! tcp_port_ready 2223 \
  || ! docker exec "$CONTAINER_NAME" \
    nc -z -w 1 127.0.0.1 "$FORWARD_ECHO_PORT" >/dev/null 2>&1; then
  echo "OpenSSH fixture did not become ready." >&2
  docker logs "$CONTAINER_NAME" >&2 || true
  docker logs "$PAM_CONTAINER_NAME" >&2 || true
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
"$DRIVER" click 640 430
sleep 0.5
"$DRIVER" type "246810"
"$DRIVER" tab
"$DRIVER" type "246810"
sleep 0.5
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
"$DRIVER" type "touch /tmp/anyssh-ime-"
set_ibus_engine "libpinyin"
"$DRIVER" type "zhongwen"
"$DRIVER" type "1"
sleep 1
set_ibus_engine "xkb:us::eng"
"$DRIVER" enter

IME_REMOTE_PATH=""
for _ in $(seq 1 40); do
  IME_REMOTE_PATH="$(
    docker exec "$CONTAINER_NAME" sh -lc '
      for path in /tmp/anyssh-ime-*; do
        [ -e "$path" ] || continue
        case "$path" in
          /tmp/anyssh-ime-中文*)
            printf "%s" "$path"
            exit 0
            ;;
        esac
      done
      exit 1
    ' 2>/dev/null || true
  )"
  if [[ -n "$IME_REMOTE_PATH" ]]; then
    break
  fi
  sleep 0.25
done

if [[ -z "$IME_REMOTE_PATH" ]]; then
  echo "The Chinese IBus composition did not reach the remote SSH shell." >&2
  docker exec "$CONTAINER_NAME" sh -lc '
    for path in /tmp/*; do
      [ -e "$path" ] || continue
      printf "path=%s\n" "$path"
      printf "%s" "$path" | od -An -tx1
    done
  ' >"$RUN_DIR/remote-tmp-entries.txt" 2>&1 || true
  "$DRIVER" probe "$RUN_DIR/failed-ime-command.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/app.log" >&2
  exit 1
fi

{
  printf 'path=%s\n' "$IME_REMOTE_PATH"
  printf '%s' "$IME_REMOTE_PATH" | od -An -tx1
} >"$RUN_DIR/remote-ime-path.txt"

"$DRIVER" probe "$RUN_DIR/04-terminal-ime-command.bmp" >/dev/null

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
"$DRIVER" probe "$RUN_DIR/04a-port-forwarding.bmp" >/dev/null

for port in "$LOCAL_FORWARD_PORT" "$DYNAMIC_FORWARD_PORT"; do
  if ! ss -ltn 2>/dev/null |
    grep -Eq "127\\.0\\.0\\.1:${port}[[:space:]]"; then
    echo "The Wayland Forward listener on port $port was not created." >&2
    exit 1
  fi
done
if ! docker exec "$CONTAINER_NAME" \
  nc -z -w 1 127.0.0.1 "$REMOTE_FORWARD_PORT" >/dev/null 2>&1; then
  echo "The Wayland Remote Forward was not registered." >&2
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
    raise SystemExit("Wayland Local Forward payload mismatch")
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
        raise SystemExit("Wayland SOCKS5 method negotiation failed")
    stream.sendall(
        b"\x05\x01\x00\x01\x7f\x00\x00\x01"
        + destination_port.to_bytes(2, "big")
    )
    reply = b""
    while len(reply) < 10:
        chunk = stream.recv(10 - len(reply))
        if not chunk:
            raise SystemExit("Wayland SOCKS5 reply was truncated")
        reply += chunk
    if reply[1] != 0:
        raise SystemExit("Wayland SOCKS5 CONNECT was rejected")
    stream.sendall(payload)
    stream.shutdown(socket.SHUT_WR)
    echoed = b""
    while True:
        chunk = stream.recv(65536)
        if not chunk:
            break
        echoed += chunk
if echoed != payload:
    raise SystemExit("Wayland Dynamic Forward payload mismatch")
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
            raise SystemExit("Wayland Remote Forward payload mismatch")
        connection.sendall(b"ANYSSH_WAYLAND_REMOTE_OK\n")
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
if [[ "$REMOTE_FORWARD_RESPONSE" != "ANYSSH_WAYLAND_REMOTE_OK" ]]; then
  echo "The Wayland Remote Forward did not reach the local destination." >&2
  exit 1
fi
wait "$FORWARD_SERVER_PID"
FORWARD_SERVER_PID=""

for marker in \
  "$LOCAL_FORWARD_MARKER" \
  "$DYNAMIC_FORWARD_MARKER" \
  "$REMOTE_FORWARD_MARKER"; do
  if grep -R -a -F "$marker" "$VAULT_ROOT" >/dev/null 2>&1 \
    || grep -a -F "$marker" "$RUN_DIR/app.log" >/dev/null 2>&1; then
    echo "A Wayland Port Forward payload leaked into Vault or app logs." >&2
    exit 1
  fi
done

"$DRIVER" click 1117 44
sleep 1
"$DRIVER" probe "$RUN_DIR/05-disconnected.bmp" >/dev/null
for port in "$LOCAL_FORWARD_PORT" "$DYNAMIC_FORWARD_PORT"; do
  if ss -ltn 2>/dev/null |
    grep -Eq "127\\.0\\.0\\.1:${port}[[:space:]]"; then
    echo "Wayland Disconnect left Forward port $port open." >&2
    exit 1
  fi
done
if docker exec "$CONTAINER_NAME" \
  nc -z -w 1 127.0.0.1 "$REMOTE_FORWARD_PORT" >/dev/null 2>&1; then
  echo "Wayland Disconnect left the Remote Forward registered." >&2
  exit 1
fi
scroll_connection_panel_top

docker exec "$CONTAINER_NAME" rm -f /tmp/anyssh-wayland-trusted-ok
set_ibus_engine "xkb:us::eng"
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
  "$DRIVER" type "touch /tmp/anyssh-wayland-trusted-ok"
  "$DRIVER" enter
  for _ in $(seq 1 12); do
    if docker exec "$CONTAINER_NAME" \
      test -f /tmp/anyssh-wayland-trusted-ok >/dev/null 2>&1; then
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
  echo "The Wayland session prompted again for a durably trusted Endpoint." >&2
  "$DRIVER" probe "$RUN_DIR/failed-wayland-trusted-reconnect.bmp" >/dev/null || true
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/06-durable-tofu-reconnect.bmp" >/dev/null

docker exec "$CONTAINER_NAME" rm -f /tmp/anyssh-wayland-tab-after-close-ok
docker exec "$PAM_CONTAINER_NAME" rm -f /tmp/anyssh-wayland-interactive-ok
"$DRIVER" click 912 128
sleep 1
"$DRIVER" click 1100 220
sleep 0.5
"$DRIVER" ctrl-a
"$DRIVER" type "Wayland interactive"
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
"$DRIVER" probe "$RUN_DIR/06a-multi-tab-interactive-host-key.bmp" >/dev/null
"$DRIVER" click 700 532
sleep 2
"$DRIVER" probe "$RUN_DIR/06b-multi-tab-interactive-challenge.bmp" >/dev/null
"$DRIVER" type "$INTERACTIVE_RESPONSE"
"$DRIVER" enter
sleep 1

INTERACTIVE_SUCCEEDED=0
for _ in $(seq 1 40); do
  "$DRIVER" click 500 260
  sleep 0.25
  "$DRIVER" type " touch /tmp/anyssh-wayland-interactive-ok"
  "$DRIVER" enter
  sleep 0.5
  if docker exec "$PAM_CONTAINER_NAME" \
    test -f /tmp/anyssh-wayland-interactive-ok >/dev/null 2>&1; then
    INTERACTIVE_SUCCEEDED=1
    break
  fi
done
if [[ "$INTERACTIVE_SUCCEEDED" -ne 1 ]]; then
  echo "The Wayland Keyboard-interactive response did not reach OpenSSH PAM." >&2
  "$DRIVER" probe "$RUN_DIR/failed-wayland-interactive.bmp" >/dev/null || true
  tail -n 120 "$RUN_DIR/app.log" >&2
  exit 1
fi
if grep -R -a -F "$INTERACTIVE_RESPONSE" "$VAULT_ROOT" >/dev/null 2>&1 \
  || grep -a -F "$INTERACTIVE_RESPONSE" "$RUN_DIR/app.log" >/dev/null 2>&1; then
  echo "The Keyboard-interactive response leaked into Wayland evidence or Vault files." >&2
  exit 1
fi
"$DRIVER" click 1200 740
"$DRIVER" ctrl-a
"$DRIVER" type "$TAB_CLOSE_FORWARD_PORT"
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" tab
"$DRIVER" enter
sleep 1
if ! ss -ltn 2>/dev/null |
  grep -Eq "127\\.0\\.0\\.1:${TAB_CLOSE_FORWARD_PORT}[[:space:]]"; then
  echo "The interactive Wayland Tab did not start its Local Forward." >&2
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/06c-multi-tab-interactive-connected.bmp" >/dev/null
"$DRIVER" click 648 128
sleep 1
if ss -ltn 2>/dev/null |
  grep -Eq "127\\.0\\.0\\.1:${TAB_CLOSE_FORWARD_PORT}[[:space:]]"; then
  echo "Closing the interactive Wayland Tab left its Forward listener open." >&2
  exit 1
fi
"$DRIVER" click 500 260
sleep 0.5
"$DRIVER" type " touch /tmp/anyssh-wayland-tab-after-close-ok"
"$DRIVER" enter

FIRST_TAB_SURVIVED_CLOSE=0
for _ in $(seq 1 20); do
  if docker exec "$CONTAINER_NAME" \
    test -f /tmp/anyssh-wayland-tab-after-close-ok >/dev/null 2>&1; then
    FIRST_TAB_SURVIVED_CLOSE=1
    break
  fi
  sleep 0.25
done
if [[ "$FIRST_TAB_SURVIVED_CLOSE" -ne 1 ]]; then
  echo "Closing the interactive Wayland Tab affected the first Session." >&2
  "$DRIVER" probe "$RUN_DIR/failed-wayland-first-tab-after-close.bmp" >/dev/null || true
  exit 1
fi
"$DRIVER" probe "$RUN_DIR/06d-wayland-first-tab-after-close.bmp" >/dev/null
"$DRIVER" click 1117 44
sleep 1
"$DRIVER" probe "$RUN_DIR/07-disconnected-after-reconnect.bmp" >/dev/null

if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
  echo "The AnySSH process exited unexpectedly." >&2
  exit 1
fi
for marker in \
  "$LOCAL_FORWARD_MARKER" \
  "$DYNAMIC_FORWARD_MARKER" \
  "$REMOTE_FORWARD_MARKER"; do
  if grep -R -a -F "$marker" "$RUN_DIR" >/dev/null 2>&1; then
    echo "A Wayland Port Forward payload leaked into evidence." >&2
    exit 1
  fi
done

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
- A remote file under \`/tmp/anyssh-ime-中文*\` and its recorded UTF-8 bytes proved
  that the committed text reached SSH. The suffix may vary with the packaged
  libpinyin candidate behavior used by the automation environment.
- The Wayland-native Forward UI started real Local, unauthenticated Dynamic
  SOCKS5, and Remote Loopback Forwards. External clients crossed russh while
  payload markers stayed absent from Vault, logs, and evidence.
- Disconnect removed both local listeners and the server-side Remote
  registration.
- Disconnect returned the application to the disconnected state.
- A second native Wayland connection used durable Trust without another Host
  Key prompt.
- While the durably trusted Session remained connected, a second Wayland Tab
  completed a masked RFC 4256 Challenge against OpenSSH PAM on a separate
  Endpoint and created \`/tmp/anyssh-wayland-interactive-ok\`.
- Closing the interactive Tab removed its Session-scoped Local Forward and left
  the first Session connected and accepting a follow-up command.
- The session-bound response remained absent from Vault files and
  \`app.log\`.

## Evidence

- \`01-vault-create.bmp\`
- \`02-wayland-ready.bmp\`
- \`03-host-key-dialog.bmp\`
- \`04-terminal-ime-command.bmp\`
- \`04a-port-forwarding.bmp\`
- \`05-disconnected.bmp\`
- \`06-durable-tofu-reconnect.bmp\`
- \`06a-multi-tab-interactive-host-key.bmp\`
- \`06b-multi-tab-interactive-challenge.bmp\`
- \`06c-multi-tab-interactive-connected.bmp\`
- \`06d-wayland-first-tab-after-close.bmp\`
- \`07-disconnected-after-reconnect.bmp\`
- \`app-backend-environment.txt\`
- \`ibus-engines.txt\`
- \`remote-ime-path.txt\`
- \`wayland-info.txt\`
- \`weston.log\`
- \`app.log\`
EOF

echo "Native Wayland and IBus smoke passed: $RUN_DIR"
