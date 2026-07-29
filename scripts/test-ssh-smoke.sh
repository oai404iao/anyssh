#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="anyssh-openssh-fixture:phase0"
PAM_IMAGE_NAME="anyssh-openssh-pam-fixture:phase1"
RUN_SUFFIX="$RANDOM-$$"
JUMP_CONTAINER="anyssh-jump-$RUN_SUFFIX"
JUMP_TWO_CONTAINER="anyssh-jump-two-$RUN_SUFFIX"
TARGET_CONTAINER="anyssh-target-$RUN_SUFFIX"
DEEP_TARGET_CONTAINER="anyssh-deep-target-$RUN_SUFFIX"
BLACKHOLE_CONTAINER="anyssh-blackhole-$RUN_SUFFIX"
INTERACTIVE_CONTAINER="anyssh-interactive-$RUN_SUFFIX"
INTERACTIVE_TARGET_CONTAINER="anyssh-interactive-target-$RUN_SUFFIX"
INTERACTIVE_PASSWORD_TARGET_CONTAINER="anyssh-interactive-password-target-$RUN_SUFFIX"
INTERACTIVE_JUMP_TARGET_CONTAINER="anyssh-interactive-jump-target-$RUN_SUFFIX"
INTERACTIVE_JUMP_TWO_TARGET_CONTAINER="anyssh-interactive-jump-two-target-$RUN_SUFFIX"
EDGE_NETWORK="anyssh-edge-$RUN_SUFFIX"
INTERNAL_NETWORK="anyssh-internal-$RUN_SUFFIX"
DEEP_NETWORK="anyssh-deep-$RUN_SUFFIX"
TARGET_ALIAS="ssh-target-internal"
JUMP_TWO_ALIAS="ssh-jump-two"
DEEP_TARGET_ALIAS="ssh-target-deep"
BLACKHOLE_ALIAS="ssh-blackhole"
INTERACTIVE_TARGET_ALIAS="ssh-interactive-target"
INTERACTIVE_PASSWORD_TARGET_ALIAS="ssh-interactive-password-target"
INTERACTIVE_JUMP_TARGET_ALIAS="ssh-interactive-jump-target"
INTERACTIVE_JUMP_TWO_TARGET_ALIAS="ssh-interactive-jump-two-target"
FORWARD_ECHO_PORT=18080
KEY_PASSPHRASE="anyssh-key-passphrase"
INTERACTIVE_RESPONSE="otp-$RANDOM-$RANDOM"
KEY_DIR=""
AGENT_PID=""
AGENT_SOCKET=""
AGENT_FINGERPRINT=""

cleanup() {
  if [[ -n "$AGENT_PID" ]]; then
    kill "$AGENT_PID" >/dev/null 2>&1 || true
    wait "$AGENT_PID" >/dev/null 2>&1 || true
  fi
  docker rm -f \
    "$JUMP_CONTAINER" \
    "$JUMP_TWO_CONTAINER" \
    "$TARGET_CONTAINER" \
    "$DEEP_TARGET_CONTAINER" \
    "$BLACKHOLE_CONTAINER" \
    "$INTERACTIVE_CONTAINER" \
    "$INTERACTIVE_TARGET_CONTAINER" \
    "$INTERACTIVE_PASSWORD_TARGET_CONTAINER" \
    "$INTERACTIVE_JUMP_TARGET_CONTAINER" \
    "$INTERACTIVE_JUMP_TWO_TARGET_CONTAINER" >/dev/null 2>&1 || true
  docker network rm \
    "$EDGE_NETWORK" \
    "$INTERNAL_NETWORK" \
    "$DEEP_NETWORK" >/dev/null 2>&1 || true
  if [[ -n "$KEY_DIR" ]]; then
    rm -rf "$KEY_DIR"
  fi
}
trap cleanup EXIT

cd "$ROOT_DIR"

for command_name in ssh-keygen ssh-agent ssh-add; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the SSH authentication fixture." >&2
    exit 1
  fi
done

KEY_DIR="$(mktemp -d)"
ssh-keygen \
  -q \
  -t ed25519 \
  -N "" \
  -C "anyssh-phase0-unencrypted" \
  -f "$KEY_DIR/id_ed25519_unencrypted"
ssh-keygen \
  -q \
  -t ed25519 \
  -N "$KEY_PASSPHRASE" \
  -C "anyssh-phase0-encrypted" \
  -f "$KEY_DIR/id_ed25519_encrypted"
ssh-keygen \
  -q \
  -t ed25519 \
  -N "" \
  -C "anyssh-phase0-unauthorized" \
  -f "$KEY_DIR/id_ed25519_unauthorized"
cat \
  "$KEY_DIR/id_ed25519_unencrypted.pub" \
  "$KEY_DIR/id_ed25519_encrypted.pub" \
  >"$KEY_DIR/authorized_keys"

AGENT_SOCKET="$KEY_DIR/agent.sock"
ssh-agent -a "$AGENT_SOCKET" -D \
  >"$KEY_DIR/ssh-agent.stdout.log" \
  2>"$KEY_DIR/ssh-agent.stderr.log" &
AGENT_PID=$!
for _ in $(seq 1 100); do
  if [[ -S "$AGENT_SOCKET" ]]; then
    break
  fi
  if ! kill -0 "$AGENT_PID" >/dev/null 2>&1; then
    echo "ssh-agent exited before creating its socket." >&2
    cat "$KEY_DIR/ssh-agent.stderr.log" >&2 || true
    exit 1
  fi
  sleep 0.05
done
if [[ ! -S "$AGENT_SOCKET" ]]; then
  echo "ssh-agent did not create its socket." >&2
  exit 1
fi
SSH_AUTH_SOCK="$AGENT_SOCKET" ssh-add "$KEY_DIR/id_ed25519_unencrypted" >/dev/null
AGENT_FINGERPRINT="$(
  ssh-keygen -lf "$KEY_DIR/id_ed25519_unencrypted.pub" -E sha256 |
    awk 'NR == 1 { print $2 }'
)"
if [[ "$AGENT_FINGERPRINT" != SHA256:* ]]; then
  echo "Unable to resolve the ssh-agent fixture fingerprint." >&2
  exit 1
fi

docker build \
  --quiet \
  --tag "$IMAGE_NAME" \
  tests/fixtures/openssh >/dev/null
docker build \
  --quiet \
  --file tests/fixtures/openssh/Dockerfile.pam \
  --tag "$PAM_IMAGE_NAME" \
  tests/fixtures/openssh >/dev/null

docker network create "$EDGE_NETWORK" >/dev/null
docker network create --internal "$INTERNAL_NETWORK" >/dev/null
docker network create --internal "$DEEP_NETWORK" >/dev/null

docker run \
  --detach \
  --name "$JUMP_CONTAINER" \
  --network "$EDGE_NETWORK" \
  --publish 127.0.0.1::22 \
  "$IMAGE_NAME" >/dev/null

docker network connect "$INTERNAL_NETWORK" "$JUMP_CONTAINER"

JUMP_PORT="$(
  docker port "$JUMP_CONTAINER" 22/tcp |
    awk -F: 'NR == 1 { print $NF }'
)"
JUMP_INTERNAL_IP="$(
  docker inspect \
    --format "{{(index .NetworkSettings.Networks \"$INTERNAL_NETWORK\").IPAddress}}" \
    "$JUMP_CONTAINER"
)"

if [[ -z "$JUMP_PORT" || -z "$JUMP_INTERNAL_IP" ]]; then
  echo "Unable to resolve the Jump Host fixture addresses." >&2
  exit 1
fi

docker run \
  --detach \
  --name "$JUMP_TWO_CONTAINER" \
  --network "$INTERNAL_NETWORK" \
  --network-alias "$JUMP_TWO_ALIAS" \
  --env "ANYSSH_ALLOW_FROM=$JUMP_INTERNAL_IP" \
  "$IMAGE_NAME" >/dev/null

docker network connect "$DEEP_NETWORK" "$JUMP_TWO_CONTAINER"

JUMP_TWO_DEEP_IP="$(
  docker inspect \
    --format "{{(index .NetworkSettings.Networks \"$DEEP_NETWORK\").IPAddress}}" \
    "$JUMP_TWO_CONTAINER"
)"

if [[ -z "$JUMP_TWO_DEEP_IP" ]]; then
  echo "Unable to resolve the second Jump Host fixture address." >&2
  exit 1
fi

docker run \
  --detach \
  --name "$TARGET_CONTAINER" \
  --network "$INTERNAL_NETWORK" \
  --network-alias "$TARGET_ALIAS" \
  --env "ANYSSH_ALLOW_FROM=$JUMP_INTERNAL_IP" \
  "$IMAGE_NAME" >/dev/null

TARGET_INTERNAL_IP="$(
  docker inspect \
    --format "{{(index .NetworkSettings.Networks \"$INTERNAL_NETWORK\").IPAddress}}" \
    "$TARGET_CONTAINER"
)"

if [[ -z "$TARGET_INTERNAL_IP" ]]; then
  echo "Unable to resolve the Internal Target fixture address." >&2
  exit 1
fi

docker run \
  --detach \
  --name "$DEEP_TARGET_CONTAINER" \
  --network "$DEEP_NETWORK" \
  --network-alias "$DEEP_TARGET_ALIAS" \
  --env "ANYSSH_ALLOW_FROM=$JUMP_TWO_DEEP_IP" \
  "$IMAGE_NAME" >/dev/null

docker run \
  --detach \
  --name "$BLACKHOLE_CONTAINER" \
  --network "$INTERNAL_NETWORK" \
  --network-alias "$BLACKHOLE_ALIAS" \
  alpine:3.22 \
  /bin/sh -c \
  'printf "#!/bin/sh\nsleep 60\n" >/tmp/hold-open && chmod +x /tmp/hold-open && exec nc -lk -p 22 -e /tmp/hold-open' \
  >/dev/null

docker run \
  --detach \
  --name "$INTERACTIVE_CONTAINER" \
  --network "$EDGE_NETWORK" \
  --publish 127.0.0.1::22 \
  --env "ANYSSH_OTP_TOKEN=$INTERACTIVE_RESPONSE" \
  "$PAM_IMAGE_NAME" >/dev/null

docker network connect "$INTERNAL_NETWORK" "$INTERACTIVE_CONTAINER"

INTERACTIVE_PORT="$(
  docker port "$INTERACTIVE_CONTAINER" 22/tcp |
    awk -F: 'NR == 1 { print $NF }'
)"
INTERACTIVE_INTERNAL_IP="$(
  docker inspect \
    --format "{{(index .NetworkSettings.Networks \"$INTERNAL_NETWORK\").IPAddress}}" \
    "$INTERACTIVE_CONTAINER"
)"

if [[ -z "$INTERACTIVE_PORT" || -z "$INTERACTIVE_INTERNAL_IP" ]]; then
  echo "Unable to resolve the Keyboard-interactive fixture addresses." >&2
  exit 1
fi

docker run \
  --detach \
  --name "$INTERACTIVE_TARGET_CONTAINER" \
  --network "$INTERNAL_NETWORK" \
  --network-alias "$INTERACTIVE_TARGET_ALIAS" \
  --env "ANYSSH_ALLOW_FROM=$JUMP_INTERNAL_IP" \
  --env "ANYSSH_AUTHENTICATION_METHODS=publickey,keyboard-interactive:pam" \
  --env "ANYSSH_OTP_TOKEN=$INTERACTIVE_RESPONSE" \
  "$PAM_IMAGE_NAME" >/dev/null

docker network connect "$DEEP_NETWORK" "$INTERACTIVE_TARGET_CONTAINER"
INTERACTIVE_TARGET_DEEP_IP="$(
  docker inspect \
    --format "{{(index .NetworkSettings.Networks \"$DEEP_NETWORK\").IPAddress}}" \
    "$INTERACTIVE_TARGET_CONTAINER"
)"
if [[ -z "$INTERACTIVE_TARGET_DEEP_IP" ]]; then
  echo "Unable to resolve the Interactive Jump 2 deep-network address." >&2
  exit 1
fi

docker run \
  --detach \
  --name "$INTERACTIVE_PASSWORD_TARGET_CONTAINER" \
  --network "$INTERNAL_NETWORK" \
  --network-alias "$INTERACTIVE_PASSWORD_TARGET_ALIAS" \
  --env "ANYSSH_ALLOW_FROM=$JUMP_INTERNAL_IP" \
  --env "ANYSSH_AUTHENTICATION_METHODS=password,keyboard-interactive:pam" \
  --env "ANYSSH_PASSWORD_AUTHENTICATION=yes" \
  --env "ANYSSH_OTP_TOKEN=$INTERACTIVE_RESPONSE" \
  "$PAM_IMAGE_NAME" >/dev/null

docker run \
  --detach \
  --name "$INTERACTIVE_JUMP_TARGET_CONTAINER" \
  --network "$INTERNAL_NETWORK" \
  --network-alias "$INTERACTIVE_JUMP_TARGET_ALIAS" \
  --env "ANYSSH_ALLOW_FROM=$INTERACTIVE_INTERNAL_IP" \
  "$IMAGE_NAME" >/dev/null

docker run \
  --detach \
  --name "$INTERACTIVE_JUMP_TWO_TARGET_CONTAINER" \
  --network "$DEEP_NETWORK" \
  --network-alias "$INTERACTIVE_JUMP_TWO_TARGET_ALIAS" \
  --env "ANYSSH_ALLOW_FROM=$INTERACTIVE_TARGET_DEEP_IP" \
  "$IMAGE_NAME" >/dev/null

install_authorized_keys() {
  local container="$1"
  docker exec "$container" \
    sh -c 'mkdir -p /home/anyssh/.ssh && chmod 700 /home/anyssh/.ssh'
  docker cp \
    "$KEY_DIR/authorized_keys" \
    "$container:/home/anyssh/.ssh/authorized_keys" >/dev/null
  docker exec "$container" \
    sh -c 'chown -R anyssh:anyssh /home/anyssh/.ssh && chmod 600 /home/anyssh/.ssh/authorized_keys'
}

install_authorized_keys "$JUMP_CONTAINER"
install_authorized_keys "$JUMP_TWO_CONTAINER"
install_authorized_keys "$TARGET_CONTAINER"
install_authorized_keys "$DEEP_TARGET_CONTAINER"
install_authorized_keys "$INTERACTIVE_TARGET_CONTAINER"

docker exec -d "$TARGET_CONTAINER" \
  sh -c "exec nc -lk -p $FORWARD_ECHO_PORT -e /bin/cat"

FIXTURES_READY=false
for _ in $(seq 1 100); do
  if ssh-keyscan -p "$JUMP_PORT" 127.0.0.1 >/dev/null 2>&1 \
    && ssh-keyscan -p "$INTERACTIVE_PORT" 127.0.0.1 >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$TARGET_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$TARGET_ALIAS" "$FORWARD_ECHO_PORT" >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$JUMP_TWO_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$JUMP_TWO_CONTAINER" \
      nc -z -w 1 "$DEEP_TARGET_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$BLACKHOLE_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$INTERACTIVE_TARGET_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$INTERACTIVE_PASSWORD_TARGET_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$INTERACTIVE_CONTAINER" \
      nc -z -w 1 "$INTERACTIVE_JUMP_TARGET_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$INTERACTIVE_TARGET_CONTAINER" \
      nc -z -w 1 "$INTERACTIVE_JUMP_TWO_TARGET_ALIAS" 22 >/dev/null 2>&1; then
    FIXTURES_READY=true
    break
  fi
  sleep 0.1
done

if [[ "$FIXTURES_READY" != true ]]; then
  echo "OpenSSH Jump Host topology did not become ready." >&2
  docker logs "$JUMP_CONTAINER" >&2 || true
  docker logs "$JUMP_TWO_CONTAINER" >&2 || true
  docker logs "$TARGET_CONTAINER" >&2 || true
  docker logs "$DEEP_TARGET_CONTAINER" >&2 || true
  docker logs "$INTERACTIVE_CONTAINER" >&2 || true
  docker logs "$INTERACTIVE_TARGET_CONTAINER" >&2 || true
  docker logs "$INTERACTIVE_PASSWORD_TARGET_CONTAINER" >&2 || true
  docker logs "$INTERACTIVE_JUMP_TARGET_CONTAINER" >&2 || true
  docker logs "$INTERACTIVE_JUMP_TWO_TARGET_CONTAINER" >&2 || true
  exit 1
fi

if docker exec "$JUMP_CONTAINER" \
  nc -z -w 1 "$DEEP_TARGET_ALIAS" 22 >/dev/null 2>&1; then
  echo "The first Jump Host unexpectedly reached the deep Target directly." >&2
  exit 1
fi

ANYSSH_TEST_SSH_HOST=127.0.0.1 \
ANYSSH_TEST_SSH_PORT="$JUMP_PORT" \
  cargo test \
    --package anyssh-ssh \
    --test openssh_smoke \
    -- \
    --ignored \
    --nocapture

ANYSSH_TEST_SSH_HOST=127.0.0.1 \
ANYSSH_TEST_SSH_PORT="$JUMP_PORT" \
  cargo test \
    --package anyssh-ssh \
    --test backpressure_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_INTERACTIVE_HOST=127.0.0.1 \
ANYSSH_TEST_INTERACTIVE_PORT="$INTERACTIVE_PORT" \
ANYSSH_TEST_INTERACTIVE_TARGET_HOST="$INTERACTIVE_TARGET_ALIAS" \
ANYSSH_TEST_INTERACTIVE_PASSWORD_TARGET_HOST="$INTERACTIVE_PASSWORD_TARGET_ALIAS" \
ANYSSH_TEST_INTERACTIVE_JUMP_TARGET_HOST="$INTERACTIVE_JUMP_TARGET_ALIAS" \
ANYSSH_TEST_INTERACTIVE_JUMP_TWO_TARGET_HOST="$INTERACTIVE_JUMP_TWO_TARGET_ALIAS" \
ANYSSH_TEST_INTERACTIVE_RESPONSE="$INTERACTIVE_RESPONSE" \
ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_UNENCRYPTED_KEY="$KEY_DIR/id_ed25519_unencrypted" \
ANYSSH_TEST_AGENT_FINGERPRINT="$AGENT_FINGERPRINT" \
SSH_AUTH_SOCK="$AGENT_SOCKET" \
  cargo test \
    --package anyssh-ssh \
    --test keyboard_interactive_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_INTERACTIVE_HOST=127.0.0.1 \
ANYSSH_TEST_INTERACTIVE_PORT="$INTERACTIVE_PORT" \
ANYSSH_TEST_INTERACTIVE_JUMP_TARGET_HOST="$INTERACTIVE_JUMP_TARGET_ALIAS" \
ANYSSH_TEST_INTERACTIVE_RESPONSE="$INTERACTIVE_RESPONSE" \
  cargo test \
    --package anyssh-app \
    --test keyboard_interactive_saved_host_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_TARGET_HOST="$TARGET_ALIAS" \
ANYSSH_TEST_TARGET_PORT=22 \
ANYSSH_TEST_UNENCRYPTED_KEY="$KEY_DIR/id_ed25519_unencrypted" \
ANYSSH_TEST_ENCRYPTED_KEY="$KEY_DIR/id_ed25519_encrypted" \
ANYSSH_TEST_UNAUTHORIZED_KEY="$KEY_DIR/id_ed25519_unauthorized" \
ANYSSH_TEST_KEY_PASSPHRASE="$KEY_PASSPHRASE" \
  cargo test \
    --package anyssh-ssh \
    --test private_key_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

SSH_AUTH_SOCK="$AGENT_SOCKET" \
ANYSSH_TEST_SSH_HOST=127.0.0.1 \
ANYSSH_TEST_SSH_PORT="$JUMP_PORT" \
ANYSSH_TEST_AGENT_FINGERPRINT="$AGENT_FINGERPRINT" \
  cargo test \
    --package anyssh-ssh \
    --test system_agent_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

SSH_AUTH_SOCK="$AGENT_SOCKET" \
ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_TARGET_HOST="$TARGET_ALIAS" \
ANYSSH_TEST_AGENT_FINGERPRINT="$AGENT_FINGERPRINT" \
ANYSSH_TEST_ENCRYPTED_KEY="$KEY_DIR/id_ed25519_encrypted" \
ANYSSH_TEST_KEY_PASSPHRASE="$KEY_PASSPHRASE" \
  cargo test \
    --package anyssh-app \
    --test system_agent_saved_host_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_ENCRYPTED_KEY="$KEY_DIR/id_ed25519_encrypted" \
ANYSSH_TEST_KEY_PASSPHRASE="$KEY_PASSPHRASE" \
  cargo test \
    --package anyssh-app \
    --test vault_credential_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_JUMP_TWO_HOST="$JUMP_TWO_ALIAS" \
ANYSSH_TEST_DEEP_TARGET_HOST="$DEEP_TARGET_ALIAS" \
ANYSSH_TEST_ENCRYPTED_KEY="$KEY_DIR/id_ed25519_encrypted" \
ANYSSH_TEST_KEY_PASSPHRASE="$KEY_PASSPHRASE" \
  cargo test \
    --package anyssh-app \
    --test saved_host_route_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_JUMP_CONTAINER="$JUMP_CONTAINER" \
  cargo test \
    --package anyssh-ssh \
    --test host_key_change_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_TARGET_HOST="$TARGET_ALIAS" \
ANYSSH_TEST_FORWARD_ECHO_PORT="$FORWARD_ECHO_PORT" \
  cargo test \
    --package anyssh-ssh \
    --test port_forwarding_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

ANYSSH_TEST_JUMP_HOST=127.0.0.1 \
ANYSSH_TEST_JUMP_PORT="$JUMP_PORT" \
ANYSSH_TEST_TARGET_HOST="$TARGET_ALIAS" \
ANYSSH_TEST_TARGET_IP="$TARGET_INTERNAL_IP" \
ANYSSH_TEST_TARGET_PORT=22 \
ANYSSH_TEST_BLACKHOLE_HOST="$BLACKHOLE_ALIAS" \
ANYSSH_TEST_BLACKHOLE_PORT=22 \
ANYSSH_TEST_JUMP_CONTAINER="$JUMP_CONTAINER" \
  cargo test \
    --package anyssh-ssh \
    --test jump_host_smoke \
    -- \
    --ignored \
    --nocapture \
    --test-threads=1

echo "OpenSSH password, Keyboard-interactive/OTP, Private Key, System Agent, Vault Credential ID, saved Host Route, host-key, backpressure, Jump Host, and Port Forwarding smoke tests passed."
