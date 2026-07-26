#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="anyssh-openssh-fixture:phase0"
RUN_SUFFIX="$RANDOM-$$"
JUMP_CONTAINER="anyssh-jump-$RUN_SUFFIX"
TARGET_CONTAINER="anyssh-target-$RUN_SUFFIX"
BLACKHOLE_CONTAINER="anyssh-blackhole-$RUN_SUFFIX"
EDGE_NETWORK="anyssh-edge-$RUN_SUFFIX"
INTERNAL_NETWORK="anyssh-internal-$RUN_SUFFIX"
TARGET_ALIAS="ssh-target-internal"
BLACKHOLE_ALIAS="ssh-blackhole"

cleanup() {
  docker rm -f \
    "$JUMP_CONTAINER" \
    "$TARGET_CONTAINER" \
    "$BLACKHOLE_CONTAINER" >/dev/null 2>&1 || true
  docker network rm "$EDGE_NETWORK" "$INTERNAL_NETWORK" >/dev/null 2>&1 || true
}
trap cleanup EXIT

cd "$ROOT_DIR"

docker build \
  --quiet \
  --tag "$IMAGE_NAME" \
  tests/fixtures/openssh >/dev/null

docker network create "$EDGE_NETWORK" >/dev/null
docker network create --internal "$INTERNAL_NETWORK" >/dev/null

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
  --name "$BLACKHOLE_CONTAINER" \
  --network "$INTERNAL_NETWORK" \
  --network-alias "$BLACKHOLE_ALIAS" \
  alpine:3.22 \
  /bin/sh -c \
  'printf "#!/bin/sh\nsleep 60\n" >/tmp/hold-open && chmod +x /tmp/hold-open && exec nc -lk -p 22 -e /tmp/hold-open' \
  >/dev/null

FIXTURES_READY=false
for _ in $(seq 1 100); do
  if ssh-keyscan -p "$JUMP_PORT" 127.0.0.1 >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$TARGET_ALIAS" 22 >/dev/null 2>&1 \
    && docker exec "$JUMP_CONTAINER" \
      nc -z -w 1 "$BLACKHOLE_ALIAS" 22 >/dev/null 2>&1; then
    FIXTURES_READY=true
    break
  fi
  sleep 0.1
done

if [[ "$FIXTURES_READY" != true ]]; then
  echo "OpenSSH Jump Host topology did not become ready." >&2
  docker logs "$JUMP_CONTAINER" >&2 || true
  docker logs "$TARGET_CONTAINER" >&2 || true
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

echo "OpenSSH direct and Jump Host smoke tests passed."
