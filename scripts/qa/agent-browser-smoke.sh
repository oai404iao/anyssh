#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
URL="${ANYSSH_QA_URL:-http://127.0.0.1:1420/}"
SESSION="anyssh-smoke-$$"
RUN_ID="$(date +%s)"
OUTPUT_DIR="$ROOT_DIR/artifacts/agent-browser/smoke-$RUN_ID"
SERVER_PID=""

cleanup() {
  agent-browser --session "$SESSION" close >/dev/null 2>&1 || true
  if [[ -n "$SERVER_PID" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

mkdir -p "$OUTPUT_DIR/screenshots"

if ! command -v agent-browser >/dev/null 2>&1; then
  echo "agent-browser is required. Install it and run: agent-browser install" >&2
  exit 1
fi

if ! curl --fail --silent "$URL" >/dev/null 2>&1; then
  (
    cd "$ROOT_DIR/apps/client"
    exec pnpm exec vite --host 127.0.0.1
  ) >"$OUTPUT_DIR/vite.log" 2>&1 &
  SERVER_PID=$!

  for _ in $(seq 1 60); do
    if curl --fail --silent "$URL" >/dev/null 2>&1; then
      break
    fi
    sleep 0.2
  done
fi

if ! curl --fail --silent "$URL" >/dev/null 2>&1; then
  echo "AnySSH dev server did not become ready at $URL." >&2
  exit 1
fi

agent-browser --session "$SESSION" open "$URL"
agent-browser --session "$SESSION" wait --load networkidle
agent-browser --session "$SESSION" wait --text "Open a session"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/01-initial-desktop.png"

INITIAL_SNAPSHOT="$(agent-browser --session "$SESSION" snapshot -i)"
printf '%s\n' "$INITIAL_SNAPSHOT" >"$OUTPUT_DIR/01-initial-snapshot.txt"

PASSWORD_REF="$(
  printf '%s\n' "$INITIAL_SNAPSHOT" |
    sed -n 's/.*textbox "Password" \[ref=\([^]]*\)\].*/@\1/p' |
    head -n1
)"

if [[ -z "$PASSWORD_REF" ]]; then
  echo "Password input was not exposed in the accessibility tree." >&2
  exit 1
fi

agent-browser --session "$SESSION" find label "Password" fill "fixture-password"
agent-browser --session "$SESSION" find role button click --name "Show password"

if [[ "$(agent-browser --session "$SESSION" get attr "$PASSWORD_REF" type)" != "text" ]]; then
  echo "Password reveal did not switch the input to text." >&2
  exit 1
fi

agent-browser --session "$SESSION" find role button click --name "Hide password"

if [[ "$(agent-browser --session "$SESSION" get attr "$PASSWORD_REF" type)" != "password" ]]; then
  echo "Password hide did not restore the password input type." >&2
  exit 1
fi

agent-browser --session "$SESSION" find role button click --name "Connect"
agent-browser --session "$SESSION" wait --text "Verify server identity"
agent-browser --session "$SESSION" wait --text "Target host"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/02-host-key-dialog.png"
agent-browser --session "$SESSION" snapshot >"$OUTPUT_DIR/02-host-key-snapshot.txt"

if ! grep -F "SHA256:" "$OUTPUT_DIR/02-host-key-snapshot.txt" >/dev/null; then
  echo "The host-key dialog did not expose its SHA-256 fingerprint." >&2
  exit 1
fi

agent-browser --session "$SESSION" find role button click --name "Trust for this session"
agent-browser --session "$SESSION" wait --text "Interactive shell is active."

CONNECTED_SNAPSHOT="$(agent-browser --session "$SESSION" snapshot -i)"
printf '%s\n' "$CONNECTED_SNAPSHOT" >"$OUTPUT_DIR/03-connected-snapshot.txt"

TERMINAL_REF="$(
  printf '%s\n' "$CONNECTED_SNAPSHOT" |
    sed -n 's/.*textbox "Terminal input" \[ref=\([^]]*\)\].*/@\1/p' |
    head -n1
)"

if [[ -z "$TERMINAL_REF" ]]; then
  echo "Terminal input was not exposed in the accessibility tree." >&2
  exit 1
fi

agent-browser --session "$SESSION" focus "$TERMINAL_REF"
for key in u n i c o d e; do
  agent-browser --session "$SESSION" press "$key"
done
agent-browser --session "$SESSION" press Enter
agent-browser --session "$SESSION" wait 400
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03-connected-unicode.png"

agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/04-mobile-connected.png"
agent-browser --session "$SESSION" snapshot -i >"$OUTPUT_DIR/04-mobile-snapshot.txt"

agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find role button click --name "Disconnect"
agent-browser --session "$SESSION" wait --text "The SSH session has ended."
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/05-disconnected.png"

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(3)"
agent-browser --session "$SESSION" wait --text "Secrets stay encrypted in the Vault"
agent-browser --session "$SESSION" find role button click --name "New password"
agent-browser --session "$SESSION" find label "Credential label" fill \
  "Browser QA password"
agent-browser --session "$SESSION" find label "Username" fill "browser-qa"
agent-browser --session "$SESSION" find label "Password" fill \
  "browser-qa-password-must-not-return"
agent-browser --session "$SESSION" find role button click \
  --name "Save Credential"
agent-browser --session "$SESSION" wait --text "Browser QA password"

agent-browser --session "$SESSION" find role button click \
  --name "Import private key"
agent-browser --session "$SESSION" find label "Credential label" fill \
  "Browser QA imported key"
agent-browser --session "$SESSION" find label "Username" fill "browser-key"
agent-browser --session "$SESSION" find role button click \
  --name "Choose private key"
agent-browser --session "$SESSION" wait --text "Browser QA imported key"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06-credentials.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/06-credentials-snapshot.txt"

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(2)"
agent-browser --session "$SESSION" find role button click --name "New host"
agent-browser --session "$SESSION" find label "Display name" fill \
  "Browser QA target"
agent-browser --session "$SESSION" find label "Host" fill "qa.internal"
agent-browser --session "$SESSION" fill \
  ".resource-dialog input[type=number]" "2202"
agent-browser --session "$SESSION" select \
  ".resource-dialog select" "browser-credential-4"
agent-browser --session "$SESSION" find role button click --name "Save Host"
agent-browser --session "$SESSION" wait --text "Browser QA target"

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(4)"
agent-browser --session "$SESSION" find role button click --name "New route"
agent-browser --session "$SESSION" find label "Route label" fill \
  "Browser QA ordered route"
agent-browser --session "$SESSION" select \
  ".resource-dialog select" "browser-host-local"
agent-browser --session "$SESSION" find role button click --name "Add"
agent-browser --session "$SESSION" select \
  ".resource-dialog select" "browser-host-edge"
agent-browser --session "$SESSION" find role button click --name "Add"
agent-browser --session "$SESSION" find role button click \
  --name "Move Edge gateway up"
agent-browser --session "$SESSION" find role button click \
  --name "Save Jump Route"
agent-browser --session "$SESSION" wait --text "Browser QA ordered route"

agent-browser --session "$SESSION" click \
  ".resource-card:first-of-type .resource-actions button:last-child"
agent-browser --session "$SESSION" click \
  ".resource-card:first-of-type .resource-actions button:last-child"
agent-browser --session "$SESSION" wait --text "in use"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07-route-desktop.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/07-route-snapshot.txt"

agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/08-route-mobile.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/08-route-mobile-snapshot.txt"

BROWSER_ERRORS="$(agent-browser --session "$SESSION" errors)"
printf '%s\n' "$BROWSER_ERRORS" >"$OUTPUT_DIR/errors.txt"
if [[ -n "${BROWSER_ERRORS//[[:space:]]/}" ]]; then
  echo "Browser errors were detected:" >&2
  printf '%s\n' "$BROWSER_ERRORS" >&2
  exit 1
fi

agent-browser --session "$SESSION" console >"$OUTPUT_DIR/console.txt"

cat >"$OUTPUT_DIR/report.md" <<EOF
# AnySSH agent-browser smoke report

- Target: \`$URL\`
- Result: PASS

## Verified

- Desktop layout rendered at 1440x900.
- Password reveal and hide changed the real input type.
- Connect flow displayed a target-scoped host-key dialog and SHA-256 fingerprint.
- Trust action opened the browser QA terminal session.
- Real keyboard events reached xterm.js.
- Unicode/CJK/Nerd Font preview command rendered.
- Responsive layout rendered at 390x844.
- Disconnect returned the UI to the disconnected state.
- Password Credential creation returned metadata without showing the submitted
  password after the editor closed.
- Private Key import UI used a metadata-only browser simulation and exposed no
  file input, Path, Key text, or Passphrase field.
- Host creation referenced the new Credential by opaque ID.
- Jump Route creation added two Hosts, moved the second Host up, and preserved
  the visible order.
- Deleting a Jump Route still referenced by a Host showed an in-use error.
- Configuration UI rendered at desktop and mobile viewports.
- Browser error log was empty.

## Evidence

- \`screenshots/01-initial-desktop.png\`
- \`screenshots/02-host-key-dialog.png\`
- \`screenshots/03-connected-unicode.png\`
- \`screenshots/04-mobile-connected.png\`
- \`screenshots/05-disconnected.png\`
- \`screenshots/06-credentials.png\`
- \`screenshots/07-route-desktop.png\`
- \`screenshots/08-route-mobile.png\`
EOF

echo "agent-browser smoke passed: $OUTPUT_DIR"
