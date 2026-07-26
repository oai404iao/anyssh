#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_TARGET="${ANYSSH_ANDROID_TARGET:-aarch64}"
ANDROID_NDK_VERSION="${ANYSSH_ANDROID_NDK_VERSION:-29.0.13846066}"
ANDROID_SDK_VERSION="${ANYSSH_ANDROID_SDK_VERSION:-36}"
ANDROID_BUILD_TOOLS_VERSION="${ANYSSH_ANDROID_BUILD_TOOLS_VERSION:-35.0.0}"
RUST_TARGET="aarch64-linux-android"
ANDROID_ABI="arm64-v8a"
GRADLE_VERSION="8.14.3"
GRADLE_WRAPPER_JAR_SHA256="7d3a4ac4de1c32b59bc6a4eb8ecb8e612ccd0cf1ae1e99f66902da64df296172"
GRADLE_DISTRIBUTION_SHA256="bd71102213493060956ec229d946beee57158dbd89d0e62b91bca0fa2c5f3531"

for command in file pnpm rustup sha256sum unzip; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing Android build dependency: $command" >&2
    exit 1
  fi
done

if [[ "$ANDROID_TARGET" != "aarch64" ]]; then
  echo "Phase 0 currently validates only the Android aarch64 target." >&2
  exit 1
fi

ANDROID_HOME="${ANYSSH_ANDROID_HOME:-${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}}"
if [[ ! -e "$ANDROID_HOME/platforms/android-$ANDROID_SDK_VERSION/android.jar" &&
  -e "$HOME/Android/Sdk/platforms/android-$ANDROID_SDK_VERSION/android.jar" ]]; then
  echo "Using the writable Android SDK at $HOME/Android/Sdk instead of $ANDROID_HOME."
  ANDROID_HOME="$HOME/Android/Sdk"
fi
ANDROID_SDK_ROOT="$ANDROID_HOME"
NDK_HOME="${ANYSSH_ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION}"
JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk}"
NDK_TOOLCHAIN_BIN="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"

export ANDROID_HOME ANDROID_SDK_ROOT NDK_HOME JAVA_HOME
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/cmdline-tools/bin:$NDK_TOOLCHAIN_BIN:$PATH"
export AR_aarch64_linux_android="$NDK_TOOLCHAIN_BIN/llvm-ar"
export RANLIB_aarch64_linux_android="$NDK_TOOLCHAIN_BIN/llvm-ranlib"

required_paths=(
  "$JAVA_HOME/bin/java"
  "$ANDROID_HOME/platforms/android-$ANDROID_SDK_VERSION/android.jar"
  "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION"
  "$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION/aapt"
  "$NDK_HOME/source.properties"
  "$AR_aarch64_linux_android"
  "$RANLIB_aarch64_linux_android"
)

for path in "${required_paths[@]}"; do
  if [[ ! -e "$path" ]]; then
    echo "Missing Android SDK component: $path" >&2
    exit 1
  fi
done

JAVA_VERSION_OUTPUT="$("$JAVA_HOME/bin/java" -version 2>&1)"
if ! grep -q 'version "17' <<<"$JAVA_VERSION_OUTPUT"; then
  echo "Android Phase 0 requires JDK 17 at JAVA_HOME=$JAVA_HOME." >&2
  exit 1
fi

if ! rustup target list --installed | grep -Fxq "$RUST_TARGET"; then
  rustup target add "$RUST_TARGET"
fi

cd "$ROOT_DIR"

if [[ ! -x apps/client/src-tauri/gen/android/gradlew ]]; then
  pnpm --filter @anyssh/client tauri android init --ci --skip-targets-install
fi

GRADLE_WRAPPER_ROOT="$ROOT_DIR/apps/client/src-tauri/gen/android/gradle/wrapper"
GRADLE_WRAPPER_JAR="$GRADLE_WRAPPER_ROOT/gradle-wrapper.jar"
GRADLE_WRAPPER_PROPERTIES="$GRADLE_WRAPPER_ROOT/gradle-wrapper.properties"

if [[ ! -f "$GRADLE_WRAPPER_JAR" || ! -f "$GRADLE_WRAPPER_PROPERTIES" ]]; then
  echo "The committed Android Gradle wrapper is incomplete." >&2
  exit 1
fi

echo "$GRADLE_WRAPPER_JAR_SHA256  $GRADLE_WRAPPER_JAR" |
  sha256sum --check --strict
if ! grep -Fxq \
  "distributionUrl=https\\://services.gradle.org/distributions/gradle-$GRADLE_VERSION-bin.zip" \
  "$GRADLE_WRAPPER_PROPERTIES"; then
  echo "The Android Gradle wrapper does not use Gradle $GRADLE_VERSION." >&2
  exit 1
fi
if ! grep -Fxq \
  "distributionSha256Sum=$GRADLE_DISTRIBUTION_SHA256" \
  "$GRADLE_WRAPPER_PROPERTIES"; then
  echo "The Android Gradle distribution checksum is not pinned as expected." >&2
  exit 1
fi

pnpm --filter @anyssh/client tauri android build \
  --debug \
  --target "$ANDROID_TARGET" \
  --apk \
  --ci

APK_PATH="$ROOT_DIR/apps/client/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"
if [[ ! -f "$APK_PATH" ]]; then
  echo "Android build completed without producing the expected APK: $APK_PATH" >&2
  exit 1
fi

RUN_DIR="$ROOT_DIR/artifacts/android-build/build-$(date +%s)-$$"
mkdir -p "$RUN_DIR"
unzip -Z1 "$APK_PATH" >"$RUN_DIR/apk-contents.txt"
"$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS_VERSION/aapt" \
  dump badging \
  "$APK_PATH" \
  >"$RUN_DIR/apk-badging.txt"

if ! grep -Fxq "lib/$ANDROID_ABI/libanyssh_client_lib.so" "$RUN_DIR/apk-contents.txt"; then
  echo "The APK does not contain the AnySSH Rust library for $ANDROID_ABI." >&2
  exit 1
fi
if ! grep -q "^package: name='com\\.spiredive\\.anyssh'" "$RUN_DIR/apk-badging.txt"; then
  echo "The APK does not use the expected AnySSH application ID." >&2
  exit 1
fi
if ! grep -q "^native-code: '$ANDROID_ABI'" "$RUN_DIR/apk-badging.txt"; then
  echo "The APK does not declare the expected native ABI." >&2
  exit 1
fi
if ! grep -Fxq "targetSdkVersion:'$ANDROID_SDK_VERSION'" "$RUN_DIR/apk-badging.txt"; then
  echo "The APK does not declare Android target SDK $ANDROID_SDK_VERSION." >&2
  exit 1
fi

NATIVE_LIBRARY="$(mktemp)"
trap 'rm -f "$NATIVE_LIBRARY"' EXIT
unzip -p \
  "$APK_PATH" \
  "lib/$ANDROID_ABI/libanyssh_client_lib.so" \
  >"$NATIVE_LIBRARY"
file --brief "$NATIVE_LIBRARY" >"$RUN_DIR/native-library-file.txt"
if ! grep -q 'ELF 64-bit.*ARM aarch64' "$RUN_DIR/native-library-file.txt"; then
  echo "The APK native library is not a 64-bit ARM ELF object." >&2
  exit 1
fi
if ! grep -a -q 'SQLCipher private heap stats' "$NATIVE_LIBRARY"; then
  echo "The Android native library does not contain the bundled SQLCipher marker." >&2
  exit 1
fi
printf '%s\n' 'SQLCipher private heap stats' >"$RUN_DIR/sqlcipher-marker.txt"

{
  printf 'gradle-wrapper.jar  %s\n' "$GRADLE_WRAPPER_JAR_SHA256"
  printf 'gradle-%s-bin.zip  %s\n' \
    "$GRADLE_VERSION" \
    "$GRADLE_DISTRIBUTION_SHA256"
} >"$RUN_DIR/gradle-wrapper-checksums.txt"

cp "$APK_PATH" "$RUN_DIR/AnySSH-arm64-debug.apk"
(
  cd "$RUN_DIR"
  sha256sum "AnySSH-arm64-debug.apk" >"SHA256SUMS"
)

cat >"$RUN_DIR/report.md" <<EOF
# AnySSH Android build report

- Result: PASS
- Application ID: \`com.spiredive.anyssh\`
- Rust target: \`$RUST_TARGET\`
- Android ABI: \`$ANDROID_ABI\`
- Android SDK: \`$ANDROID_SDK_VERSION\`
- Android Build Tools: \`$ANDROID_BUILD_TOOLS_VERSION\`
- Android NDK: \`$ANDROID_NDK_VERSION\`
- APK: \`AnySSH-arm64-debug.apk\`

## Verified

- Tauri generated the Android Gradle project.
- The React frontend completed its production build.
- The Rust SSH, Vault, bundled SQLCipher, and Tauri crates cross-compiled for Android ARM64.
- The APK declares \`com.spiredive.anyssh\` and target SDK \`$ANDROID_SDK_VERSION\`.
- The committed Gradle wrapper JAR and Gradle distribution checksum match their pinned values.
- Gradle produced a debug APK containing a 64-bit ARM \`libanyssh_client_lib.so\`.
- The Android native library contains a bundled SQLCipher implementation marker.

## Evidence

- \`AnySSH-arm64-debug.apk\`
- \`SHA256SUMS\`
- \`apk-contents.txt\`
- \`apk-badging.txt\`
- \`native-library-file.txt\`
- \`sqlcipher-marker.txt\`
- \`gradle-wrapper-checksums.txt\`
EOF

echo "Android ARM64 build passed: $RUN_DIR"
