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
    exec pnpm exec vite --host 0.0.0.0
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

wait_for_interactive_text() {
  local expected="$1"
  local snapshot=""
  for _ in $(seq 1 60); do
    snapshot="$(agent-browser --session "$SESSION" snapshot -i)"
    if grep -F "$expected" <<<"$snapshot" >/dev/null; then
      return 0
    fi
    sleep 0.1
  done
  echo "Timed out waiting for interactive text: $expected" >&2
  return 1
}

select_ui_option() {
  local label="$1"
  local value="$2"
  agent-browser --session "$SESSION" find role combobox click \
    --name "$label" --exact
  agent-browser --session "$SESSION" wait 100
  agent-browser --session "$SESSION" click \
    "[role='option'][data-value='$value']"
}

set_ui_switch() {
  local label="$1"
  local expected="$2"
  local current
  current="$(
    agent-browser --session "$SESSION" get attr \
      '[data-ui-control="switch"]' aria-checked
  )"
  if [[ "$current" != "$expected" ]]; then
    agent-browser --session "$SESSION" click '[data-ui-control="switch"]'
  fi
}

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

agent-browser --session "$SESSION" scrollintoview \
  ".forwarding-form button[type=submit]"
agent-browser --session "$SESSION" find role button click \
  --name "Start forward"
wait_for_interactive_text "Stop local forward on port"
agent-browser --session "$SESSION" select \
  '[aria-label="Port forward type"]' "dynamic"
agent-browser --session "$SESSION" scrollintoview \
  ".forwarding-form button[type=submit]"
agent-browser --session "$SESSION" find role button click \
  --name "Start forward"
wait_for_interactive_text "Stop dynamic forward on port"
agent-browser --session "$SESSION" select \
  '[aria-label="Port forward type"]' "remote"
agent-browser --session "$SESSION" scrollintoview \
  ".forwarding-form button[type=submit]"
agent-browser --session "$SESSION" find role button click \
  --name "Start forward"
wait_for_interactive_text "Stop remote forward on port"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/03a-forwarding-snapshot.txt"
for kind in local dynamic remote; do
  if ! grep -F "Stop $kind forward on port" \
    "$OUTPUT_DIR/03a-forwarding-snapshot.txt" >/dev/null; then
    echo "The $kind Browser Preview Forward was not active." >&2
    exit 1
  fi
done
agent-browser --session "$SESSION" scrollintoview \
  ".forwarding-list li:last-child"
agent-browser --session "$SESSION" screenshot \
  "$OUTPUT_DIR/screenshots/03a-forwarding-desktop.png"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" wait 150
agent-browser --session "$SESSION" find role button click \
  --name "Forwarding" --exact
agent-browser --session "$SESSION" scrollintoview \
  ".forwarding-list li:last-child"
agent-browser --session "$SESSION" screenshot \
  "$OUTPUT_DIR/screenshots/03a-forwarding-mobile.png"
agent-browser --session "$SESSION" find role button click \
  --name "Sessions" --exact
agent-browser --session "$SESSION" set viewport 1440 900

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
if grep -F "Stop local forward on port" \
  "$OUTPUT_DIR/03b-multi-tab-snapshot.txt" >/dev/null \
  || grep -F "Stop remote forward on port" \
    "$OUTPUT_DIR/03b-multi-tab-snapshot.txt" >/dev/null; then
  echo "The first Tab's Forward metadata leaked into the second Tab." >&2
  exit 1
fi
agent-browser --session "$SESSION" select \
  '[aria-label="Port forward type"]' "dynamic"
agent-browser --session "$SESSION" find role button click \
  --name "Start forward"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/03b-second-tab-forwarding-snapshot.txt"
if ! grep -F "Stop dynamic forward on port" \
  "$OUTPUT_DIR/03b-second-tab-forwarding-snapshot.txt" >/dev/null; then
  echo "The second Tab did not own its Dynamic Forward metadata." >&2
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
for kind in local dynamic remote; do
  if ! grep -F "Stop $kind forward on port" \
    <<<"$REMAINING_SESSION_SNAPSHOT" >/dev/null; then
    echo "Closing the second Tab affected the first Tab's $kind Forward." >&2
    exit 1
  fi
done
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

agent-browser --session "$SESSION" eval \
  'window.__anysshTerminalMount = document.querySelector(".terminal-mount"); true' \
  >"$OUTPUT_DIR/03e-terminal-mount-before.txt"
agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(8)"
agent-browser --session "$SESSION" wait --text "Apply appearance"
agent-browser --session "$SESSION" find role button click --name "Import Theme"
agent-browser --session "$SESSION" wait --text "Browser QA Midnight"
agent-browser --session "$SESSION" find role button click --name "Import Font"
agent-browser --session "$SESSION" wait --text "Browser QA Mono"
agent-browser --session "$SESSION" find role combobox click \
  --name "App theme" --exact
agent-browser --session "$SESSION" wait 100
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03e0-component-select-open.png"
agent-browser --session "$SESSION" click \
  "[role='option'][data-value='light']"
select_ui_option "Terminal theme" "theme-browser-1"
select_ui_option "Terminal font" "imported:font-browser-1"
agent-browser --session "$SESSION" fill \
  '[aria-label="Terminal font size"]' "16"
agent-browser --session "$SESSION" click \
  '[aria-label="Decrease Terminal font size"]'
agent-browser --session "$SESSION" click \
  '[aria-label="Increase Terminal font size"]'
select_ui_option "Terminal line height" "1600"
set_ui_switch "Programming ligatures" "true"
select_ui_option "East Asian ambiguous width" "wide"
agent-browser --session "$SESSION" find role button click \
  --name "Apply appearance"
agent-browser --session "$SESSION" wait --fn \
  "document.documentElement.dataset.appTheme === 'light'"
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/03e-appearance-snapshot.txt"
if grep -F 'input type="file"' \
  "$OUTPUT_DIR/03e-appearance-snapshot.txt" >/dev/null; then
  echo "Browser Appearance import exposed a file input." >&2
  exit 1
fi
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03e-appearance-light.png"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03f-appearance-light-mobile.png"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(1)"
if [[ "$(agent-browser --session "$SESSION" eval \
  'window.__anysshTerminalMount === document.querySelector(".terminal-mount")')" != "true" ]]; then
  echo "Appearance update recreated the mounted Terminal." >&2
  exit 1
fi
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03g-terminal-custom-appearance.png"

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(7)"
agent-browser --session "$SESSION" wait --text "Snippets"
agent-browser --session "$SESSION" find role button click --name "New Snippet"
agent-browser --session "$SESSION" find label "Label" fill \
  "Browser QA multi-line"
agent-browser --session "$SESSION" fill \
  '[aria-label="Snippet command template"]' \
  $'echo {{target}}\nprintf browser-qa-finished'
agent-browser --session "$SESSION" find role button click \
  --name "Save Snippet"
agent-browser --session "$SESSION" wait --text "Browser QA multi-line"
agent-browser --session "$SESSION" snapshot \
  >"$OUTPUT_DIR/03h-snippet-summary-snapshot.txt"
if grep -F "printf browser-qa-finished" \
  "$OUTPUT_DIR/03h-snippet-summary-snapshot.txt" >/dev/null; then
  echo "Snippet list exposed the encrypted Body outside explicit Edit." >&2
  exit 1
fi
agent-browser --session "$SESSION" click \
  ".snippet-card:last-child .snippet-actions button:nth-child(2)"
agent-browser --session "$SESSION" find label "target" fill "browser-qa-marker"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03h-snippet-confirmation.png"
agent-browser --session "$SESSION" click \
  '.multiline-confirmation [data-ui-control="checkbox"]'
agent-browser --session "$SESSION" find role button click \
  --name "Run in Session"
agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(1)"
agent-browser --session "$SESSION" wait 300
SNIPPET_TERMINAL_TEXT="$(
  agent-browser --session "$SESSION" get text ".xterm-rows"
)"
if [[ "$SNIPPET_TERMINAL_TEXT" != *"echo browser-qa-marker"* ]] \
  || [[ "$SNIPPET_TERMINAL_TEXT" != *"printf browser-qa-finished"* ]]; then
  echo "The variable-aware multi-line Snippet did not reach the selected Terminal." >&2
  exit 1
fi
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/03i-snippet-terminal-output.png"

agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(8)"
select_ui_option "App theme" "dark"
select_ui_option "Terminal theme" "builtin:obsidian"
select_ui_option "Terminal font" "bundled:anyssh-nerd-mono"
agent-browser --session "$SESSION" fill \
  '[aria-label="Terminal font size"]' "13"
select_ui_option "Terminal line height" "1420"
set_ui_switch "Programming ligatures" "false"
select_ui_option "East Asian ambiguous width" "narrow"
agent-browser --session "$SESSION" find role button click \
  --name "Apply appearance"
agent-browser --session "$SESSION" click ".primary-nav .nav-item:nth-child(1)"
agent-browser --session "$SESSION" wait --fn \
  "document.documentElement.dataset.appTheme === 'dark'"

agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/04-mobile-connected.png"
agent-browser --session "$SESSION" snapshot -i >"$OUTPUT_DIR/04-mobile-snapshot.txt"
if [[ "$(agent-browser --session "$SESSION" get count \
  '[aria-label="SSH auxiliary keyboard"]')" != "1" ]]; then
  echo "The Android Terminal shell did not expose its auxiliary keyboard." >&2
  exit 1
fi
for control in \
  "Toggle Control modifier" \
  "Toggle Alt modifier" \
  "Send Escape" \
  "Send Tab" \
  "Send Arrow Up" \
  "Terminal actions"; do
  if ! grep -F "$control" "$OUTPUT_DIR/04-mobile-snapshot.txt" >/dev/null; then
    echo "The Android Terminal shell did not expose $control." >&2
    exit 1
  fi
done
agent-browser --session "$SESSION" find role button click \
  --name "Toggle Control modifier"
if [[ "$(agent-browser --session "$SESSION" get attr \
  '[aria-label="Toggle Control modifier"]' aria-pressed)" != "true" ]]; then
  echo "The Android Control modifier did not latch." >&2
  exit 1
fi
if [[ "$(agent-browser --session "$SESSION" eval \
  'document.activeElement?.classList.contains("xterm-helper-textarea")')" != "true" ]]; then
  echo "The Android Control modifier did not return focus to xterm." >&2
  exit 1
fi
agent-browser --session "$SESSION" find role button click \
  --name "Toggle Control modifier"
agent-browser --session "$SESSION" find role button click \
  --name "Keyboard" --exact
if [[ "$(agent-browser --session "$SESSION" eval \
  'document.activeElement?.classList.contains("xterm-helper-textarea")')" != "true" ]]; then
  echo "The Android Keyboard action did not focus xterm." >&2
  exit 1
fi

agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find role button click --name "Disconnect"
agent-browser --session "$SESSION" wait --text "The SSH session has ended."
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/05-disconnected-snapshot.txt"
if grep -F "Stop local forward on port" \
  "$OUTPUT_DIR/05-disconnected-snapshot.txt" >/dev/null \
  || grep -F "Stop dynamic forward on port" \
    "$OUTPUT_DIR/05-disconnected-snapshot.txt" >/dev/null \
  || grep -F "Stop remote forward on port" \
    "$OUTPUT_DIR/05-disconnected-snapshot.txt" >/dev/null; then
  echo "Disconnect left Browser Preview Forward metadata active." >&2
  exit 1
fi
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
agent-browser --session "$SESSION" find role button click --name "More" --exact
agent-browser --session "$SESSION" wait --text "Jump routes"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/05c2-mobile-more-navigation.png"
agent-browser --session "$SESSION" click \
  ".mobile-navigation-sheet header > button"
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
  --name "Generate key"
agent-browser --session "$SESSION" find label "Credential label" fill \
  "Browser QA generated key"
agent-browser --session "$SESSION" find label "Username" fill \
  "browser-generated"
agent-browser --session "$SESSION" select ".resource-dialog select" "rsa4096"
agent-browser --session "$SESSION" click \
  ".resource-dialog button.connect-button"
agent-browser --session "$SESSION" wait --text "Browser QA generated key"
agent-browser --session "$SESSION" click \
  ".resource-list .resource-card:last-child .resource-actions button:first-child"
agent-browser --session "$SESSION" wait --text "SHA-256 fingerprint"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06a-generated-public-key.png"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/06a-generated-public-key-mobile.png"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" snapshot -i \
  >"$OUTPUT_DIR/06a-generated-public-key-snapshot.txt"
if grep -F "BEGIN OPENSSH PRIVATE KEY" \
  "$OUTPUT_DIR/06a-generated-public-key-snapshot.txt" >/dev/null; then
  echo "Generated Private Key material entered the Browser snapshot." >&2
  exit 1
fi
agent-browser --session "$SESSION" click \
  ".resource-dialog > header > button"
agent-browser --session "$SESSION" click \
  ".resource-list .resource-card:last-child .resource-actions button:nth-child(2)"
agent-browser --session "$SESSION" wait --text \
  "Browser QA writes no file."

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
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07c-host-editor-desktop.png"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07c-host-editor-mobile.png"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find role button click --name "Save Host"
agent-browser --session "$SESSION" wait --text "Browser QA target"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07c-hosts-desktop.png"
agent-browser --session "$SESSION" fill ".host-search input" \
  "Browser QA target"
if [[ "$(agent-browser --session "$SESSION" eval \
  'document.querySelectorAll(".host-resource-card").length')" != "1" ]]; then
  echo "Host search did not reduce the product grid to one matching Host." >&2
  exit 1
fi
agent-browser --session "$SESSION" click \
  ".host-resource-card .resource-actions button:first-child"
agent-browser --session "$SESSION" wait --text "Connect directly"
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07d-host-detail-desktop.png"
agent-browser --session "$SESSION" set viewport 390 844
agent-browser --session "$SESSION" screenshot --full \
  "$OUTPUT_DIR/screenshots/07e-host-detail-mobile.png"
agent-browser --session "$SESSION" set viewport 1440 900
agent-browser --session "$SESSION" find role button click --name "Close details"
agent-browser --session "$SESSION" fill ".host-search input" ""

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
- Local, Dynamic SOCKS5, and Remote Forward metadata started with assigned
  ports at desktop/mobile widths without opening Browser network listeners.
- Forward metadata stayed isolated by Session Tab; closing the second Tab left
  the first Tab's three Forwards active, and Disconnect cleared them.
- Two concurrent Preview Session Tabs rendered at desktop/mobile widths; closing
  the second Tab left the first Terminal connected and accepting input.
- App Light/Dark, Custom Terminal Theme, imported Font metadata, Font Size,
  Line Height, Ligature, and Ambiguous Width settings updated the existing
  mounted xterm.js instance without recreating it.
- Browser Theme/Font import exposed metadata only and no file input or Path.
- Snippet list kept Body hidden; a variable-aware multi-line Snippet required
  full Preview/Confirmation and reached only the selected connected Terminal.
- Compact 1024x768 layout rendered with an icon-only sidebar.
- Responsive layout rendered at 390x844.
- The Android Product Shell exposed Bottom Navigation, a More management
  sheet, full-height Terminal, SSH auxiliary keys, latched Ctrl/Alt controls,
  Forwarding switcher, and an xterm keyboard-focus action.
- Disconnect returned the UI to the disconnected state.
- Known Hosts displayed the trusted Endpoint, Algorithm, and SHA-256 Fingerprint
  at desktop and mobile widths.
- Forget Trust removed the entry, and the next connection required TOFU again.
- Password Credential creation returned metadata without showing the submitted
  password after the editor closed.
- Private Key import UI used a metadata-only browser simulation and exposed no
  file input, Path, Key text, or Passphrase field.
- Private Key Generation simulated metadata only, exposed RSA Public Key and
  SHA-256 Fingerprint, and kept Private Key/PIN/Passphrase/Path out of Browser
  state. Encrypted Export explicitly wrote no Browser file.
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
- Host Editor grouped target, authentication, and advanced route fields into
  three bounded sections at desktop/mobile widths.
- Host search reduced the Material 3 grid to the matching saved Host; the
  desktop/mobile Detail view exposed only connection-plan metadata and a
  deliberate session action without assembling a saved connection plan in
  Browser QA.
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
- \`screenshots/03a-forwarding-desktop.png\`
- \`screenshots/03a-forwarding-mobile.png\`
- \`screenshots/03b-multi-tab-desktop.png\`
- \`screenshots/03c-multi-tab-mobile.png\`
- \`screenshots/03d-first-tab-after-close.png\`
- \`screenshots/03e0-component-select-open.png\`
- \`screenshots/03e-appearance-light.png\`
- \`screenshots/03f-appearance-light-mobile.png\`
- \`screenshots/03g-terminal-custom-appearance.png\`
- \`screenshots/03h-snippet-confirmation.png\`
- \`screenshots/03i-snippet-terminal-output.png\`
- \`screenshots/04-mobile-connected.png\`
- \`screenshots/05-disconnected.png\`
- \`screenshots/05b-known-hosts.png\`
- \`screenshots/05c-known-hosts-mobile.png\`
- \`screenshots/05c2-mobile-more-navigation.png\`
- \`screenshots/05d-known-hosts-forgotten.png\`
- \`screenshots/05e-tofu-after-forget.png\`
- \`screenshots/06-credentials.png\`
- \`screenshots/06a-generated-public-key.png\`
- \`screenshots/06a-generated-public-key-mobile.png\`
- \`screenshots/06b-credentials-mobile.png\`
- \`screenshots/06c-interactive-challenge.png\`
- \`screenshots/06d-interactive-challenge-mobile.png\`
- \`screenshots/06e-interactive-connected.png\`
- \`screenshots/07-groups.png\`
- \`screenshots/07b-groups-mobile.png\`
- \`screenshots/07c-host-editor-desktop.png\`
- \`screenshots/07c-host-editor-mobile.png\`
- \`screenshots/07c-hosts-desktop.png\`
- \`screenshots/07d-host-detail-desktop.png\`
- \`screenshots/07e-host-detail-mobile.png\`
- \`screenshots/08-route-desktop.png\`
- \`screenshots/09-route-compact.png\`
- \`screenshots/10-route-mobile.png\`
- \`screenshots/11-changed-host-key.png\`
EOF

echo "agent-browser smoke passed: $OUTPUT_DIR"
