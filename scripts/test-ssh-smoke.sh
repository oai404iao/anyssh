#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="anyssh-openssh-fixture:phase0"
CONTAINER_NAME="anyssh-openssh-$RANDOM-$$"

cleanup() {
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

cd "$ROOT_DIR"

docker build \
  --quiet \
  --tag "$IMAGE_NAME" \
  tests/fixtures/openssh >/dev/null

docker run \
  --detach \
  --name "$CONTAINER_NAME" \
  --publish 127.0.0.1::22 \
  "$IMAGE_NAME" >/dev/null

SSH_PORT="$(
  docker port "$CONTAINER_NAME" 22/tcp |
    awk -F: 'NR == 1 { print $NF }'
)"

if [[ -z "$SSH_PORT" ]]; then
  echo "Unable to resolve the OpenSSH fixture port." >&2
  exit 1
fi

for _ in $(seq 1 50); do
  if ssh-keyscan -p "$SSH_PORT" 127.0.0.1 >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

ANYSSH_TEST_SSH_HOST=127.0.0.1 \
ANYSSH_TEST_SSH_PORT="$SSH_PORT" \
  cargo test \
    --package anyssh-ssh \
    --test openssh_smoke \
    -- \
    --ignored \
    --nocapture
