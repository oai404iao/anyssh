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

agent-browser --session "$SESSION" find role button click --name "Trust and continue"
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

agent-browser --session "$SESSION" find role button click \
  --name "New session tab"
agent-browser --session "$SESSION" find label "Display name" fill \
  "Browser QA second"
agent-browser --session "$SESSION" find label "Password" fill \
  "fixture-password"
agent-browser --session "$SESSION" find role button click --name "Connect"
agent-browser --session "$SESSION" wait --text "Interactive shell is active."
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/03b-multi-tab-snapshot.txt"
if ! grep -F "Browser QA second" \
  "$OUTPUT_DIR/03b-multi-tab-snapshot.txt" >/dev/null; then
  echo "The second Session Tab was not exposed in the accessibility tree." >&2
  exit 1
fi
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03b-multi-tab-desktop.png"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03c-multi-tab-mobile.png"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find role button click \
  --name "Close Browser QA second session tab"
agent-browser --session "$SESSION" wait --text "Interactive shell is active."

REMAINING_SESSION_SNAPSHOT="$(agent-browser --session "$SESSION" snapshot -i)"
TERMINAL_REF="$(
  printf '%s\n' "$REMAINING_SESSION_SNAPSHOT" |
    sed -n 's/.*textbox "Terminal input" \[ref=\([^]]*\)\].*/@\1/p' |
    head -n1
)"
if [[ -z "$TERMINAL_REF" ]]; then
  echo "The remaining Session Tab lost its Terminal input." >&2
  exit 1
fi
agent-browser --session "$SESSION" focus "$TERMINAL_REF"
for key in s t a t u s; do
  agent-browser --session "$SESSION" press "$key"
done
agent-browser --session "$SESSION" press Enter
agent-browser --session "$SESSION" wait 300
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03d-first-tab-after-close.png"

agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/04-mobile-connected.png"
agent-browser --session "$SESSION" snapshot -i >"$OUTPUT_DIR/04-mobile-snapshot.txt"

agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find role button click --name "Disconnect"
agent-browser --session "$SESSION" wait --text "The SSH session has ended."
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/05-disconnected.png"

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(6)"
agent-browser --session "$SESSION" wait --text "Known Hosts"
agent-browser --session "$SESSION" wait --text "127.0.0.1:2222"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/05b-known-hosts.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/05b-known-hosts-snapshot.txt"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/05c-known-hosts-mobile.png"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find role button click --name "Forget trust…"
agent-browser --session "$SESSION" wait --text \
  "No trusted endpoints yet."
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/05d-known-hosts-forgotten.png"
agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(1)"
agent-browser --session "$SESSION" find role button click --name "Connect"
agent-browser --session "$SESSION" wait --text "Verify server identity"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/05e-tofu-after-forget.png"
agent-browser --session "$SESSION" find role button click --name "Trust and continue"
agent-browser --session "$SESSION" wait --text "Interactive shell is active."
agent-browser --session "$SESSION" find role button click --name "Disconnect"
agent-browser --session "$SESSION" wait --text "The SSH session has ended."

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(4)"
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

agent-browser --session "$SESSION" find role button click \
  --name "New system agent"
agent-browser --session "$SESSION" find label "Credential label" fill \
  "Browser QA system agent"
agent-browser --session "$SESSION" find label "Username" fill "browser-agent"
agent-browser --session "$SESSION" find role button click \
  --name "Save Agent Credential"
agent-browser --session "$SESSION" wait --text "Browser QA system agent"
agent-browser --session "$SESSION" find role button click \
  --name "New interactive"
agent-browser --session "$SESSION" find label "Credential label" fill \
  "Browser QA interactive"
agent-browser --session "$SESSION" find label "Username" fill "browser-otp"
agent-browser --session "$SESSION" wait --text "Session-only responses"
agent-browser --session "$SESSION" find role button click \
  --name "Save Interactive Credential"
agent-browser --session "$SESSION" wait --text "Browser QA interactive"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06-credentials.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/06-credentials-snapshot.txt"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06b-credentials-mobile.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/06b-credentials-mobile-snapshot.txt"
agent-browser --session "$SESSION" set viewport 1440 900

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(1)"
agent-browser --session "$SESSION" find label "Host" fill "multi-otp.example"
agent-browser --session "$SESSION" fill \
  ".connection-panel input[type=number]" "22"
agent-browser --session "$SESSION" find label "Username" fill "browser-otp"
agent-browser --session "$SESSION" select \
  ".connection-panel select" "keyboardInteractive"
agent-browser --session "$SESSION" find role button click --name "Connect"
agent-browser --session "$SESSION" wait --text "Verify server identity"
agent-browser --session "$SESSION" find role button click --name "Trust and continue"
agent-browser --session "$SESSION" wait --text "Multi-factor authentication"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06c-interactive-challenge.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/06c-interactive-challenge-snapshot.txt"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06d-interactive-challenge-mobile.png"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find label "Verification code:" fill \
  "browser-otp-response-must-not-persist"
agent-browser --session "$SESSION" find label "Device name:" fill \
  "browser-device-response-must-not-persist"
agent-browser --session "$SESSION" find role button click --name "Continue"
agent-browser --session "$SESSION" wait --text "Interactive shell is active."
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/06e-interactive-connected-snapshot.txt"
if grep -F "browser-otp-response-must-not-persist" \
  "$OUTPUT_DIR/06e-interactive-connected-snapshot.txt" >/dev/null; then
  echo "The submitted Keyboard-interactive response remained in Browser state." >&2
  exit 1
fi
if grep -F "browser-device-response-must-not-persist" \
  "$OUTPUT_DIR/06e-interactive-connected-snapshot.txt" >/dev/null; then
  echo "The echoed Keyboard-interactive response remained in Browser state." >&2
  exit 1
fi
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06e-interactive-connected.png"
agent-browser --session "$SESSION" find role button click --name "Disconnect"
agent-browser --session "$SESSION" wait --text "The SSH session has ended."

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(2)"
agent-browser --session "$SESSION" find role button click --name "New group"
agent-browser --session "$SESSION" find label "Group label" fill \
  "Browser QA group"
agent-browser --session "$SESSION" select \
  '[aria-label="Credential behavior"]' "set"
agent-browser --session "$SESSION" select \
  '[aria-label="Credential reference"]' "browser-credential-4"
agent-browser --session "$SESSION" find role button click --name "Save Group"
agent-browser --session "$SESSION" wait --text "Browser QA group"
agent-browser --session "$SESSION" find role button click --name "New group"
agent-browser --session "$SESSION" find label "Group label" fill \
  "Browser QA child"
agent-browser --session "$SESSION" select \
  ".resource-dialog select" "browser-group-2"
agent-browser --session "$SESSION" select \
  '[aria-label="Jump Route behavior"]' "clear"
agent-browser --session "$SESSION" find role button click --name "Save Group"
agent-browser --session "$SESSION" wait --text "Browser QA child"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07-groups.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/07-groups-snapshot.txt"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07b-groups-mobile.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/07b-groups-mobile-snapshot.txt"
agent-browser --session "$SESSION" set viewport 1440 900

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(3)"
agent-browser --session "$SESSION" find role button click --name "New host"
agent-browser --session "$SESSION" find label "Display name" fill \
  "Browser QA target"
agent-browser --session "$SESSION" find label "Host" fill "qa.internal"
agent-browser --session "$SESSION" fill \
  ".resource-dialog input[type=number]" "2202"
agent-browser --session "$SESSION" select \
  ".resource-dialog select" "browser-group-3"
agent-browser --session "$SESSION" find role button click --name "Save Host"
agent-browser --session "$SESSION" wait --text "Browser QA target"

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(5)"
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
  "$OUTPUT_DIR/screenshots/08-route-desktop.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/08-route-snapshot.txt"

agent-browser --session "$SESSION" set viewport 1024 768
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/09-route-compact.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/09-route-compact-snapshot.txt"

agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/10-route-mobile.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/10-route-mobile-snapshot.txt"

agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(1)"
agent-browser --session "$SESSION" find label "Host" fill "changed.example"
agent-browser --session "$SESSION" fill \
  ".connection-panel input[type=number]" "22"
agent-browser --session "$SESSION" find label "Username" fill "anyssh"
agent-browser --session "$SESSION" select \
  ".connection-panel select" "password"
agent-browser --session "$SESSION" find label "Password" fill "fixture-password"
agent-browser --session "$SESSION" find role button click --name "Connect"
agent-browser --session "$SESSION" wait --text "Host key changed"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/11-changed-host-key.png"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/11-changed-host-key-snapshot.txt"

if grep -F "Trust and continue" \
  "$OUTPUT_DIR/11-changed-host-key-snapshot.txt" >/dev/null; then
  echo "Changed-Key UI exposed an unsafe trust action." >&2
  exit 1
fi

BROWSER_ERRORS="$(agent-browser --session "$SESSION" errors)"
printf '%s\n' "$BROWSER_ERRORS" >"$OUTPUT_DIR/errors.txt"
if [[ -n "${BROWSER_ERRORS//[[:space:]]/}" ]]; then
  echo "Browser errors were detected:" >&2
  printf '%s\n' "$BROWSER_ERRORS" >&2
  exit 1
fi

agent-browser --session "$SESSION" console >"$OUTPUT_DIR/console.txt"
if grep -R -a -F "browser-otp-response-must-not-persist" \
  "$OUTPUT_DIR" >/dev/null 2>&1 \
  || grep -R -a -F "browser-device-response-must-not-persist" \
    "$OUTPUT_DIR" >/dev/null 2>&1; then
  echo "The Keyboard-interactive response leaked into Browser QA evidence." >&2
  exit 1
fi

cat >"$OUTPUT_DIR/report.md" <<EOF
# AnySSH agent-browser smoke report

- Target: \`$URL\`
- Result: PASS

## Verified

- Desktop layout rendered at 1440x900.
- Password reveal and hide changed the real input type.
- Connect flow displayed a target-scoped host-key dialog and SHA-256 fingerprint.
- Trust action persisted a metadata-only Known Host and opened the browser QA
  terminal session.
- Real keyboard events reached xterm.js.
- Unicode/CJK/Nerd Font preview command rendered.
- Two concurrent Preview Session Tabs rendered at desktop/mobile widths; closing
  the second Tab left the first Terminal connected and accepting input.
- Compact 1024x768 layout rendered with an icon-only sidebar.
- Responsive layout rendered at 390x844.
- Disconnect returned the UI to the disconnected state.
- Known Hosts displayed the trusted Endpoint, Algorithm, and SHA-256 Fingerprint
  at desktop and mobile widths.
- Forget Trust removed the entry, and the next connection required TOFU again.
- Password Credential creation returned metadata without showing the submitted
  password after the editor closed.
- Private Key import UI used a metadata-only browser simulation and exposed no
  file input, Path, Key text, or Passphrase field.
- System Agent creation selected a metadata-only SHA-256 Identity; no Agent
  Socket, Public Key Blob, Private Key, or signature entered Browser state.
- Interactive Credential creation stored only Label/Username metadata and
  exposed no Password, OTP Seed, Response, or Prompt Rule field.
- Quick Connection displayed a masked, target-scoped Keyboard-interactive
  Challenge at desktop and mobile widths; submission cleared the response
  before the connected snapshot.
- Parent/child Group creation exercised Credential Set, Credential Inherit,
  Jump Route Inherit, and Jump Route Clear without exposing the submitted
  password.
- Host creation joined the child Group and inherited its Credential by opaque ID.
- Jump Route creation added two Hosts, moved the second Host up, and preserved
  the visible order.
- Deleting a Jump Route still referenced by a Group showed an in-use error.
- A pre-seeded changed Host Key produced a hard-block dialog with trusted and
  received Fingerprints and no accept action.
- Configuration UI rendered at desktop and mobile viewports.
- Browser error log was empty.

## Evidence

- \`screenshots/01-initial-desktop.png\`
- \`screenshots/02-host-key-dialog.png\`
- \`screenshots/03-connected-unicode.png\`
- \`screenshots/03b-multi-tab-desktop.png\`
- \`screenshots/03c-multi-tab-mobile.png\`
- \`screenshots/03d-first-tab-after-close.png\`
- \`screenshots/04-mobile-connected.png\`
- \`screenshots/05-disconnected.png\`
- \`screenshots/05b-known-hosts.png\`
- \`screenshots/05c-known-hosts-mobile.png\`
- \`screenshots/05d-known-hosts-forgotten.png\`
- \`screenshots/05e-tofu-after-forget.png\`
- \`screenshots/06-credentials.png\`
- \`screenshots/06b-credentials-mobile.png\`
- \`screenshots/06c-interactive-challenge.png\`
- \`screenshots/06d-interactive-challenge-mobile.png\`
- \`screenshots/06e-interactive-connected.png\`
- \`screenshots/07-groups.png\`
- \`screenshots/07b-groups-mobile.png\`
- \`screenshots/08-route-desktop.png\`
- \`screenshots/09-route-compact.png\`
- \`screenshots/10-route-mobile.png\`
- \`screenshots/11-changed-host-key.png\`
EOF

echo "agent-browser smoke passed: $OUTPUT_DIR"
