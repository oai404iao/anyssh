# AnySSH build containers

The multi-stage Dockerfile provides two independent Phase 0 build images:

- `linux`: Tauri/WebKitGTK 4.1 native linking.
- `android`: JDK 17, SDK 36, Build Tools 35.0.0, NDK 29.0.13846066,
  and the Rust ARM64 Android target.

Run from the repository root:

```bash
pnpm check:container:linux
pnpm check:container:android
```

The wrapper copies the current working tree into an isolated directory, masks dependency
and compiler output with reusable caches, passes no host environment or Docker socket into
the container, drops Linux capabilities, and copies only generated build evidence back into
`artifacts/`. Deleted tracked files and ignored local files are not copied.

Platform-specific Cargo, Gradle, pnpm, and target caches live under
`${XDG_CACHE_HOME:-$HOME/.cache}/anyssh-build/`. Override the complete platform cache path
with `ANYSSH_CONTAINER_CACHE_ROOT`. The Android NDK image and debug compiler caches consume
several gigabytes; remove the corresponding cache directory and
`anyssh-build-<platform>:phase0` image when reclaiming disk space.

Windows remains a native Windows CI build because a Linux container does not validate the
MSVC/WebView2 runtime. iOS remains deferred until a macOS/Xcode environment is available.
