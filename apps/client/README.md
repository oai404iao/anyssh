# AnySSH Client

React/xterm.js frontend and Tauri application shell.

Run commands from the repository root:

```bash
pnpm dev
pnpm build
pnpm test
pnpm lint
pnpm qa:browser
pnpm qa:native:xvfb
```

`pnpm dev` starts the browser-compatible frontend. Native Tauri development additionally requires the platform prerequisites documented by Tauri.

Browser mode is explicitly marked as **Browser QA mode** and uses a local SSH terminal simulation. It is intended for UI automation only and never opens a network connection.

Native mode requires creation or unlocking of the Rust-owned encrypted Vault before the
SSH workspace is mounted. Browser QA mode bypasses this gate and does not persist a PIN.
