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

The typed SSH bridge accepts an optional single Jump Host and scopes every host-key prompt
to a request ID, route hop, host, and port. The Phase 0 connection form does not expose
Jump Host editing yet; the Rust core and Docker protocol fixture provide the current
end-to-end validation.

Native terminal output uses an acknowledgement window: Tauri keeps at most eight chunks
in flight and React acknowledges each chunk from the xterm.js `write` callback. Browser QA
mode does not invoke native acknowledgement commands.
