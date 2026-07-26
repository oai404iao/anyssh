#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLATFORM="${1:-}"

case "$PLATFORM" in
  linux)
    CHECK_COMMAND="bash scripts/check-linux-build.sh"
    ARTIFACT_DIR="linux-build"
    ;;
  android)
    CHECK_COMMAND="bash scripts/check-android-build.sh"
    ARTIFACT_DIR="android-build"
    ;;
  *)
    echo "usage: $0 <linux|android>" >&2
    exit 1
    ;;
esac

for command in docker git tar; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing container build dependency: $command" >&2
    exit 1
  fi
done

IMAGE_NAME="anyssh-build-$PLATFORM:phase0"
CACHE_ROOT="${ANYSSH_CONTAINER_CACHE_ROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/anyssh-build/$PLATFORM}"
mkdir -p "$CACHE_ROOT"
WORK_DIR="$(mktemp -d "$CACHE_ROOT/work.XXXXXX")"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

mkdir -p \
  "$CACHE_ROOT/cargo" \
  "$CACHE_ROOT/gradle" \
  "$CACHE_ROOT/home" \
  "$CACHE_ROOT/node-modules" \
  "$CACHE_ROOT/target"

git -C "$ROOT_DIR" \
  ls-files \
  --cached \
  --others \
  --exclude-standard \
  -z |
  while IFS= read -r -d '' path; do
    if [[ -e "$ROOT_DIR/$path" || -L "$ROOT_DIR/$path" ]]; then
      printf '%s\0' "$path"
    fi
  done |
  tar \
    --null \
    --verbatim-files-from \
    -C "$ROOT_DIR" \
    -cf - \
    --files-from=- |
  tar -C "$WORK_DIR" -xf -

if [[ "${ANYSSH_USE_BUILDX:-0}" == "1" ]]; then
  docker buildx build \
    --load \
    --cache-from "type=gha,scope=anyssh-build-$PLATFORM" \
    --cache-to "type=gha,mode=max,scope=anyssh-build-$PLATFORM" \
    --target "$PLATFORM" \
    --tag "$IMAGE_NAME" \
    "$ROOT_DIR/infra/build"
else
  docker build \
    --target "$PLATFORM" \
    --tag "$IMAGE_NAME" \
    "$ROOT_DIR/infra/build"
fi

docker run \
  --rm \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --user "$(id -u):$(id -g)" \
  --workdir /workspace \
  --env "CARGO_HOME=/cache/cargo" \
  --env "CI=1" \
  --env "GRADLE_USER_HOME=/cache/gradle" \
  --env "HOME=/cache/home" \
  --mount "type=bind,src=$WORK_DIR,dst=/workspace" \
  --mount "type=bind,src=$CACHE_ROOT/cargo,dst=/cache/cargo" \
  --mount "type=bind,src=$CACHE_ROOT/gradle,dst=/cache/gradle" \
  --mount "type=bind,src=$CACHE_ROOT/home,dst=/cache/home" \
  --mount "type=bind,src=$CACHE_ROOT/node-modules,dst=/workspace/node_modules" \
  --mount "type=bind,src=$CACHE_ROOT/target,dst=/workspace/target" \
  "$IMAGE_NAME" \
  bash -c "pnpm install --frozen-lockfile && $CHECK_COMMAND"

mkdir -p "$ROOT_DIR/artifacts/$ARTIFACT_DIR"
cp -a "$WORK_DIR/artifacts/$ARTIFACT_DIR/." "$ROOT_DIR/artifacts/$ARTIFACT_DIR/"

echo "Containerized $PLATFORM build passed."
