param(
  [string]$ExecutablePath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDirectory = (Resolve-Path (Join-Path $ScriptDirectory "..\..")).Path
if ([string]::IsNullOrWhiteSpace($ExecutablePath)) {
  $ExecutablePath = Join-Path $RootDirectory "target\debug\anyssh-client.exe"
}
$ExecutablePath = (Resolve-Path $ExecutablePath).Path

$Timestamp = (Get-Date).ToUniversalTime().ToString("yyyyMMdd-HHmmss")
$RunDirectory = Join-Path $RootDirectory "artifacts\native-windows\smoke-$Timestamp-$PID"
$VaultRoot = Join-Path $env:TEMP "anyssh-windows-vault-$Timestamp-$PID"
$WebViewDataRoot = Join-Path `
  ([Environment]::GetFolderPath("LocalApplicationData")) `
  "main\anyssh-qa-webview2"
$CdpPort = 9222
$SshFixtureRoot = Join-Path $env:TEMP "anyssh-windows-ssh-$Timestamp-$PID"
$SshMarkerPath = Join-Path $env:TEMP "anyssh-windows-agent-ok-$Timestamp-$PID.txt"
$PrivateKeyMarkerPath = Join-Path `
  $env:TEMP `
  "anyssh-windows-encrypted-key-ok-$Timestamp-$PID.txt"
$script:NativeProcess = $null
$script:NativeWindowHandle = [IntPtr]::Zero
$script:StageRecords = @()
$script:SshdProcess = $null
$script:SshAgentWasRunning = $false
$script:AgentPublicKeyPath = ""
$script:SshPort = 0
$script:SshUsername = ""
$script:AgentFingerprint = ""
$script:EncryptedKeyPath = ""
$script:SshdExecutable = ""
$script:SshHostKeyPath = ""
$script:SshConfigPath = ""
$script:PrivateKeyPassphrase = "windows-key-passphrase"
$script:WrongPrivateKeyPassphrase = "windows-wrong-key-passphrase"

New-Item -ItemType Directory -Force -Path $RunDirectory | Out-Null
Remove-Item -LiteralPath $VaultRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $WebViewDataRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $SshFixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $SshMarkerPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $PrivateKeyMarkerPath -Force -ErrorAction SilentlyContinue

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class AnySshWindowProbe
{
    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr window, StringBuilder title, int capacity);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    public static IntPtr FindVisibleWindow(int expectedProcessId, string titleFragment)
    {
        IntPtr matched = IntPtr.Zero;
        EnumWindows((window, parameter) =>
        {
            uint processId;
            GetWindowThreadProcessId(window, out processId);
            if (processId != expectedProcessId || !IsWindowVisible(window))
            {
                return true;
            }

            var title = new StringBuilder(512);
            GetWindowText(window, title, title.Capacity);
            if (title.ToString().IndexOf(titleFragment, StringComparison.OrdinalIgnoreCase) >= 0)
            {
                matched = window;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return matched;
    }

    public static string GetTitle(IntPtr window)
    {
        var title = new StringBuilder(512);
        GetWindowText(window, title, title.Capacity);
        return title.ToString();
    }

    public static void CloseWindow(IntPtr window)
    {
        const uint WM_CLOSE = 0x0010;
        PostMessage(window, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
    }
}
"@

function Wait-CdpPortAvailable {
  for ($Attempt = 0; $Attempt -lt 80; $Attempt++) {
    $Listener = [System.Net.Sockets.TcpListener]::new(
      [System.Net.IPAddress]::Loopback,
      $CdpPort
    )
    try {
      $Listener.Start()
      return
    }
    catch {
      Start-Sleep -Milliseconds 250
    }
    finally {
      $Listener.Stop()
    }
  }
  throw "The Windows QA WebView2 CDP port $CdpPort is unavailable."
}

function Get-FreeTcpPort {
  $Listener = [System.Net.Sockets.TcpListener]::new(
    [System.Net.IPAddress]::Loopback,
    0
  )
  try {
    $Listener.Start()
    return ([System.Net.IPEndPoint]$Listener.LocalEndpoint).Port
  }
  finally {
    $Listener.Stop()
  }
}

function ConvertTo-OpenSshPath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )
  return $Path.Replace("\", "/")
}

function Stop-SshFixture {
  if ($null -ne $script:SshdProcess) {
    try {
      if (-not $script:SshdProcess.HasExited) {
        Stop-Process -Id $script:SshdProcess.Id -Force
        $script:SshdProcess.WaitForExit(10000) | Out-Null
      }
    }
    catch {
      # The standalone sshd may already have exited during cleanup.
    }
    finally {
      $script:SshdProcess = $null
    }
  }

  if (-not [string]::IsNullOrWhiteSpace($script:AgentPublicKeyPath)) {
    try {
      & ssh-add.exe -d $script:AgentPublicKeyPath *> $null
    }
    catch {
      # The ephemeral key may already be absent.
    }
  }

  if (-not $script:SshAgentWasRunning) {
    Stop-Service -Name ssh-agent -Force -ErrorAction SilentlyContinue
  }

  Remove-Item -LiteralPath $SshFixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $SshMarkerPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $PrivateKeyMarkerPath -Force -ErrorAction SilentlyContinue
}

function Start-SshFixture {
  $SshKeygen = (Get-Command ssh-keygen.exe -ErrorAction Stop).Source
  $Sshd = (Get-Command sshd.exe -ErrorAction Stop).Source
  Get-Command ssh-add.exe -ErrorAction Stop | Out-Null

  New-Item -ItemType Directory -Force -Path $SshFixtureRoot | Out-Null
  $AgentKeyPath = Join-Path $SshFixtureRoot "id_ed25519_agent"
  $AgentPublicKeyPath = "$AgentKeyPath.pub"
  $EncryptedKeyPath = Join-Path $SshFixtureRoot "id_ed25519_encrypted"
  $EncryptedPublicKeyPath = "$EncryptedKeyPath.pub"
  $HostKeyPath = Join-Path $SshFixtureRoot "ssh_host_ed25519_key"
  $AuthorizedKeysPath = Join-Path $SshFixtureRoot "authorized_keys"
  $SshConfigPath = Join-Path $SshFixtureRoot "sshd_config"
  $SshPidPath = Join-Path $SshFixtureRoot "sshd.pid"
  $SshStdout = Join-Path $RunDirectory "sshd.stdout.log"
  $SshStderr = Join-Path $RunDirectory "sshd.stderr.log"
  $script:SshdExecutable = $Sshd
  $script:SshHostKeyPath = $HostKeyPath
  $script:SshConfigPath = $SshConfigPath

  & $SshKeygen -q -t ed25519 -N '""' -C "anyssh-windows-agent" -f $AgentKeyPath
  if ($LASTEXITCODE -ne 0) {
    throw "Unable to generate the Windows SSH Agent fixture key."
  }
  & $SshKeygen `
    -q `
    -t ed25519 `
    -N $script:PrivateKeyPassphrase `
    -C "anyssh-windows-encrypted-key" `
    -f $EncryptedKeyPath
  if ($LASTEXITCODE -ne 0) {
    throw "Unable to generate the Windows encrypted Private Key fixture."
  }
  & $SshKeygen -q -t ed25519 -N '""' -f $HostKeyPath
  if ($LASTEXITCODE -ne 0) {
    throw "Unable to generate the Windows OpenSSH host key."
  }

  $AgentService = Get-Service -Name ssh-agent -ErrorAction Stop
  $script:SshAgentWasRunning = $AgentService.Status -eq "Running"
  if (-not $script:SshAgentWasRunning) {
    Set-Service -Name ssh-agent -StartupType Manual
    Start-Service -Name ssh-agent
    (Get-Service -Name ssh-agent).WaitForStatus(
      [System.ServiceProcess.ServiceControllerStatus]::Running,
      [TimeSpan]::FromSeconds(20)
    )
  }

  $PreviousErrorActionPreference = $ErrorActionPreference
  try {
    # Windows OpenSSH writes the successful "Identity added" message to stderr.
    # Windows PowerShell promotes native stderr to NativeCommandError when the
    # script-level preference is Stop, so decide success from the exit code.
    $ErrorActionPreference = "Continue"
    & ssh-add.exe $AgentKeyPath 2>&1 | Out-Null
    $SshAddExitCode = $LASTEXITCODE
  }
  finally {
    $ErrorActionPreference = $PreviousErrorActionPreference
  }
  if ($SshAddExitCode -ne 0) {
    throw "Unable to load the Windows SSH Agent fixture key."
  }

  $FingerprintLine = (& $SshKeygen -lf $AgentPublicKeyPath -E sha256 | Select-Object -First 1)
  $FingerprintParts = @($FingerprintLine -split "\s+" | Where-Object { $_ })
  if ($FingerprintParts.Count -lt 2 -or -not $FingerprintParts[1].StartsWith("SHA256:")) {
    throw "Unable to resolve the Windows SSH Agent fixture fingerprint."
  }
  $script:AgentFingerprint = $FingerprintParts[1]
  $script:AgentPublicKeyPath = $AgentPublicKeyPath

  @(
    Get-Content -LiteralPath $AgentPublicKeyPath
    Get-Content -LiteralPath $EncryptedPublicKeyPath
  ) | Set-Content -Encoding ASCII -Path $AuthorizedKeysPath
  Remove-Item -LiteralPath $AgentKeyPath -Force
  $script:EncryptedKeyPath = $EncryptedKeyPath

  $script:SshPort = Get-FreeTcpPort
  $script:SshUsername = $env:USERNAME.ToLowerInvariant()
  $OpenSshHostKeyPath = ConvertTo-OpenSshPath $HostKeyPath
  $OpenSshAuthorizedKeysPath = ConvertTo-OpenSshPath $AuthorizedKeysPath
  $OpenSshPidPath = ConvertTo-OpenSshPath $SshPidPath
  @"
Port $($script:SshPort)
ListenAddress 127.0.0.1
HostKey $OpenSshHostKeyPath
PidFile $OpenSshPidPath
AuthorizedKeysFile $OpenSshAuthorizedKeysPath
PubkeyAuthentication yes
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitEmptyPasswords no
StrictModes no
AllowUsers $($script:SshUsername)
AllowTcpForwarding no
AllowAgentForwarding no
X11Forwarding no
PrintMotd no
LogLevel VERBOSE
"@ | Set-Content -Encoding ASCII -Path $SshConfigPath

  $script:SshdProcess = Start-Process `
    -FilePath $Sshd `
    -ArgumentList @("-D", "-e", "-f", "`"$SshConfigPath`"") `
    -RedirectStandardOutput $SshStdout `
    -RedirectStandardError $SshStderr `
    -PassThru

  $SshReady = $false
  for ($Attempt = 0; $Attempt -lt 80; $Attempt++) {
    if ($script:SshdProcess.HasExited) {
      $Details = Get-Content -LiteralPath $SshStderr -Raw -ErrorAction SilentlyContinue
      throw "The Windows OpenSSH fixture exited before listening. $Details"
    }
    $Client = [System.Net.Sockets.TcpClient]::new()
    try {
      $Connect = $Client.ConnectAsync("127.0.0.1", $script:SshPort)
      if ($Connect.Wait(250) -and $Client.Connected) {
        $SshReady = $true
        break
      }
    }
    catch {
      # Retry until the standalone server is listening.
    }
    finally {
      $Client.Dispose()
    }
    Start-Sleep -Milliseconds 250
  }
  if (-not $SshReady) {
    throw "The Windows OpenSSH fixture did not become ready."
  }

  $env:ANYSSH_WINDOWS_SSH_HOST = "127.0.0.1"
  $env:ANYSSH_WINDOWS_SSH_PORT = [string]$script:SshPort
  $env:ANYSSH_WINDOWS_SSH_USERNAME = $script:SshUsername
  $env:ANYSSH_WINDOWS_AGENT_FINGERPRINT = $script:AgentFingerprint
  $env:ANYSSH_WINDOWS_AGENT_MARKER_PATH = $SshMarkerPath

  @"
host=127.0.0.1
port=$($script:SshPort)
username=$($script:SshUsername)
agent_fingerprint=$($script:AgentFingerprint)
encrypted_key_source=$($script:EncryptedKeyPath)
"@ | Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "ssh-fixture.txt")
}

function Rotate-SshFixtureHostKey {
  if ($null -ne $script:SshdProcess -and -not $script:SshdProcess.HasExited) {
    Stop-Process -Id $script:SshdProcess.Id -Force
    $script:SshdProcess.WaitForExit(10000) | Out-Null
  }
  $script:SshdProcess = $null
  Remove-Item `
    -LiteralPath (Join-Path $SshFixtureRoot "sshd.pid") `
    -Force `
    -ErrorAction SilentlyContinue

  Remove-Item -LiteralPath $script:SshHostKeyPath -Force
  Remove-Item -LiteralPath "$($script:SshHostKeyPath).pub" -Force
  & (Get-Command ssh-keygen.exe -ErrorAction Stop).Source `
    -q `
    -t ed25519 `
    -N '""' `
    -f $script:SshHostKeyPath
  if ($LASTEXITCODE -ne 0) {
    throw "Unable to rotate the Windows OpenSSH host key."
  }

  $RotatedStdout = Join-Path $RunDirectory "sshd-rotated.stdout.log"
  $RotatedStderr = Join-Path $RunDirectory "sshd-rotated.stderr.log"
  $script:SshdProcess = Start-Process `
    -FilePath $script:SshdExecutable `
    -ArgumentList @("-D", "-e", "-f", "`"$($script:SshConfigPath)`"") `
    -RedirectStandardOutput $RotatedStdout `
    -RedirectStandardError $RotatedStderr `
    -PassThru

  $SshReady = $false
  for ($Attempt = 0; $Attempt -lt 80; $Attempt++) {
    if ($script:SshdProcess.HasExited) {
      $Details = Get-Content -LiteralPath $RotatedStderr -Raw -ErrorAction SilentlyContinue
      throw "The rotated Windows OpenSSH fixture exited before listening. $Details"
    }
    $Client = [System.Net.Sockets.TcpClient]::new()
    try {
      $Connect = $Client.ConnectAsync("127.0.0.1", $script:SshPort)
      if ($Connect.Wait(250) -and $Client.Connected) {
        $SshReady = $true
        break
      }
    }
    catch {
      # Retry until the rotated server is listening.
    }
    finally {
      $Client.Dispose()
    }
    Start-Sleep -Milliseconds 250
  }
  if (-not $SshReady) {
    throw "The rotated Windows OpenSSH fixture did not become ready."
  }

  & (Get-Command ssh-keygen.exe -ErrorAction Stop).Source `
    -lf "$($script:SshHostKeyPath).pub" `
    -E sha256 |
    Set-Content -Encoding ASCII -Path (
      Join-Path $RunDirectory "rotated-host-key-fingerprint.txt"
    )
}

function Stop-NativeProcess {
  if ($null -eq $script:NativeProcess) {
    return
  }

  try {
    if (-not $script:NativeProcess.HasExited) {
      if ($script:NativeWindowHandle -ne [IntPtr]::Zero) {
        [AnySshWindowProbe]::CloseWindow($script:NativeWindowHandle)
        $script:NativeProcess.WaitForExit(10000) | Out-Null
      }
      if (-not $script:NativeProcess.HasExited) {
        Stop-Process -Id $script:NativeProcess.Id -Force
        $script:NativeProcess.WaitForExit(10000) | Out-Null
      }
    }
  }
  catch {
    # The WebView2 CDP client may already have closed the host process.
  }
  finally {
    $script:NativeProcess = $null
    $script:NativeWindowHandle = [IntPtr]::Zero
  }
}

function Start-NativeStage {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Stage
  )

  Wait-CdpPortAvailable

  $env:ANYSSH_QA_VAULT_ROOT = $VaultRoot

  $StandardOutput = Join-Path $RunDirectory "app-$Stage.stdout.log"
  $StandardError = Join-Path $RunDirectory "app-$Stage.stderr.log"
  $script:NativeProcess = Start-Process `
    -FilePath $ExecutablePath `
    -WorkingDirectory $RootDirectory `
    -RedirectStandardOutput $StandardOutput `
    -RedirectStandardError $StandardError `
    -PassThru
  $env:ANYSSH_WINDOWS_APP_PID = [string]$script:NativeProcess.Id
  # Set native-dialog QA inputs only after AnySSH starts so the application
  # process cannot inherit the fixture Path or Passphrases through its
  # environment. These values are consumed by the external QA driver only.
  $env:ANYSSH_WINDOWS_ENCRYPTED_KEY_PATH = $script:EncryptedKeyPath
  $env:ANYSSH_WINDOWS_KEY_PASSPHRASE = $script:PrivateKeyPassphrase
  $env:ANYSSH_WINDOWS_WRONG_KEY_PASSPHRASE = $script:WrongPrivateKeyPassphrase
  $env:ANYSSH_WINDOWS_PRIVATE_KEY_MARKER_PATH = $PrivateKeyMarkerPath

  $Targets = $null
  $Version = $null
  $WindowHandle = [IntPtr]::Zero
  $WindowReady = $false
  $CdpReady = $false
  for ($Attempt = 0; $Attempt -lt 120; $Attempt++) {
    if ($script:NativeProcess.HasExited) {
      throw "AnySSH exited before the Windows runtime became ready."
    }

    $WindowHandle = [AnySshWindowProbe]::FindVisibleWindow(
      $script:NativeProcess.Id,
      "AnySSH"
    )
    if ($WindowHandle -ne [IntPtr]::Zero) {
      $WindowReady = $true
    }

    try {
      $Targets = Invoke-RestMethod `
        -Uri "http://127.0.0.1:$CdpPort/json/list" `
        -TimeoutSec 1
      $Version = Invoke-RestMethod `
        -Uri "http://127.0.0.1:$CdpPort/json/version" `
        -TimeoutSec 1
      if ($null -ne $Targets -and @($Targets).Count -gt 0) {
        $CdpReady = $true
      }
    }
    catch {
      $CdpReady = $false
    }

    if ($WindowReady -and $CdpReady) {
      break
    }
    Start-Sleep -Milliseconds 250
  }

  if ($WindowReady) {
    $WindowTitle = [AnySshWindowProbe]::GetTitle($WindowHandle)
  }
  else {
    $WindowTitle = ""
  }
  $ProbeRecord = [PSCustomObject]@{
    stage = $Stage
    pid = $script:NativeProcess.Id
    sessionId = $script:NativeProcess.SessionId
    windowReady = $WindowReady
    mainWindowHandle = ("0x{0:X}" -f $WindowHandle.ToInt64())
    mainWindowTitle = $WindowTitle
    cdpReady = $CdpReady
  }
  $ProbeRecord |
    ConvertTo-Json |
    Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "probe-$Stage.json")

  if (-not $WindowReady) {
    throw "AnySSH did not expose a non-zero native Windows window handle."
  }
  if (-not $CdpReady) {
    throw "The AnySSH WebView2 runtime did not expose its loopback CDP endpoint."
  }

  $script:NativeProcess.Refresh()
  $script:NativeWindowHandle = $WindowHandle
  $ProcessRecord = [PSCustomObject]@{
    stage = $Stage
    pid = $script:NativeProcess.Id
    sessionId = $script:NativeProcess.SessionId
    mainWindowHandle = ("0x{0:X}" -f $WindowHandle.ToInt64())
    mainWindowTitle = [AnySshWindowProbe]::GetTitle($WindowHandle)
    webViewBrowser = $Version.Browser
    protocolVersion = $Version."Protocol-Version"
  }
  $script:StageRecords += $ProcessRecord
  $ProcessRecord |
    ConvertTo-Json |
    Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "process-$Stage.json")

  @($Targets) |
    ForEach-Object {
      [PSCustomObject]@{
        type = $_.type
        title = $_.title
        url = $_.url
      }
    } |
    ConvertTo-Json |
    Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "cdp-targets-$Stage.json")

  $env:ANYSSH_WINDOWS_CDP_URL = "http://127.0.0.1:$CdpPort"
  $env:ANYSSH_WINDOWS_RUN_DIR = $RunDirectory
  $env:ANYSSH_WINDOWS_STAGE = $Stage

  Push-Location $RootDirectory
  try {
    & pnpm --filter "@anyssh/client" exec node "e2e/windows-native-smoke.mjs"
    if ($LASTEXITCODE -ne 0) {
      throw "The Windows WebView2 interaction stage '$Stage' failed."
    }
  }
  finally {
    Pop-Location
  }
}

function Assert-VaultFilesAreEncrypted {
  $Bootstrap = Join-Path $VaultRoot "vault.bootstrap.json"
  $Database = Join-Path $VaultRoot "vault.db"
  if (-not (Test-Path -LiteralPath $Bootstrap -PathType Leaf)) {
    throw "The Windows runtime did not create the Vault bootstrap."
  }
  if (-not (Test-Path -LiteralPath $Database -PathType Leaf)) {
    throw "The Windows runtime did not create the SQLCipher database."
  }

  $Needles = @(
    "246810",
    "000000",
    "windows-fixture-password",
    "windows-key-passphrase",
    "windows-wrong-key-passphrase",
    "BEGIN OPENSSH PRIVATE KEY",
    "Windows QA password",
    "Windows QA encrypted key",
    "Windows QA encrypted key host",
    "Windows QA system agent",
    "Windows QA agent host",
    "Windows QA jump",
    "Windows QA target",
    "Windows QA group",
    "Windows QA route",
    "target.internal",
    "windows-user",
    $script:SshUsername,
    $script:AgentFingerprint
  )
  foreach ($File in Get-ChildItem -LiteralPath $VaultRoot -Recurse -File) {
    $Text = [System.Text.Encoding]::UTF8.GetString(
      [System.IO.File]::ReadAllBytes($File.FullName)
    )
    foreach ($Needle in $Needles) {
      if ($Text.Contains($Needle)) {
        throw "A Windows QA plaintext marker leaked into a Vault file."
      }
    }
  }

  $DatabaseBytes = [System.IO.File]::ReadAllBytes($Database)
  $HeaderLength = [Math]::Min(16, $DatabaseBytes.Length)
  $Header = [System.Text.Encoding]::ASCII.GetString(
    $DatabaseBytes,
    0,
    $HeaderLength
  )
  if ($Header.Contains("SQLite format 3")) {
    throw "The Windows Vault database exposed a plaintext SQLite header."
  }
}

function Assert-EvidenceContainsNoPrivateKeySecrets {
  $Needles = @(
    $script:PrivateKeyPassphrase,
    $script:WrongPrivateKeyPassphrase,
    "BEGIN OPENSSH PRIVATE KEY"
  )
  foreach ($File in Get-ChildItem -LiteralPath $RunDirectory -Recurse -File) {
    $Text = [System.Text.Encoding]::UTF8.GetString(
      [System.IO.File]::ReadAllBytes($File.FullName)
    )
    foreach ($Needle in $Needles) {
      if ($Text.Contains($Needle)) {
        throw "A Windows encrypted Private Key Secret leaked into QA evidence."
      }
    }
  }
}

try {
  Start-SshFixture
  Start-NativeStage -Stage "create"
  Stop-NativeProcess
  if (-not (Test-Path -LiteralPath $SshMarkerPath -PathType Leaf)) {
    throw "The Windows System Agent session did not create its remote marker."
  }
  $SshMarker = Get-Content -LiteralPath $SshMarkerPath -Raw
  if (-not $SshMarker.Contains("ANYSSH_WINDOWS_AGENT_OK")) {
    throw "The Windows System Agent remote marker was invalid."
  }
  if (Test-Path -LiteralPath $script:EncryptedKeyPath -PathType Leaf) {
    throw "The Windows encrypted Private Key source still existed before SSH validation."
  }
  if (-not (Test-Path -LiteralPath $PrivateKeyMarkerPath -PathType Leaf)) {
    throw "The Windows encrypted Private Key session did not create its remote marker."
  }
  $PrivateKeyMarker = Get-Content -LiteralPath $PrivateKeyMarkerPath -Raw
  if (-not $PrivateKeyMarker.Contains("ANYSSH_WINDOWS_ENCRYPTED_KEY_OK")) {
    throw "The Windows encrypted Private Key remote marker was invalid."
  }
  Assert-VaultFilesAreEncrypted

  Start-NativeStage -Stage "restart"
  Stop-NativeProcess
  Assert-VaultFilesAreEncrypted

  Rotate-SshFixtureHostKey
  Start-NativeStage -Stage "changed"
  Stop-NativeProcess

  Stop-SshFixture
  Assert-VaultFilesAreEncrypted
  Assert-EvidenceContainsNoPrivateKeySecrets

  $CreateRecord = $script:StageRecords |
    Where-Object { $_.stage -eq "create" } |
    Select-Object -First 1
  $RestartRecord = $script:StageRecords |
    Where-Object { $_.stage -eq "restart" } |
    Select-Object -First 1
  $ChangedRecord = $script:StageRecords |
    Where-Object { $_.stage -eq "changed" } |
    Select-Object -First 1

  @"
# AnySSH native Windows and WebView2 smoke report

- Result: PASS
- Identifier: ``com.spiredive.anyssh``
- Executable: ``target/debug/anyssh-client.exe``
- Create window handle: ``$($CreateRecord.mainWindowHandle)``
- Restart window handle: ``$($RestartRecord.mainWindowHandle)``
- Changed-key window handle: ``$($ChangedRecord.mainWindowHandle)``
- WebView2 browser: ``$($CreateRecord.webViewBrowser)``
- CDP protocol: ``$($CreateRecord.protocolVersion)``

## Verified

- The built Windows EXE launched and exposed a non-zero top-level window handle.
- The existing WebView2 instance rendered the Tauri application and accepted
  Playwright input through a QA-only loopback CDP port.
- CDP was enabled only by ``tauri.windows-qa.conf.json`` for this ephemeral
  Debug build; the canonical Tauri config and Release builds do not expose it.
- Native Tauri IPC created a PIN Slot and SQLCipher Vault.
- Wrong PIN, Lock, Unlock, process termination, relaunch, and restart recovery passed.
- The native Windows file picker selected an encrypted OpenSSH Private Key
  without adding Path, Key, or Passphrase fields to WebView IPC.
- Windows Credential UI displayed masked, non-persistent Passphrase input,
  rejected one incorrect Passphrase, and accepted the second bounded attempt.
- The original encrypted Key and accepted Passphrase were stored under separate
  Record AEAD fields; the source Key file was deleted before SSH connection.
- The real EXE authenticated with the imported encrypted Key and created the
  remote marker ``$PrivateKeyMarkerPath``.
- Windows OpenSSH Authentication Agent enumerated the selected SHA-256 Identity
  through Rust; the temporary Private Key file was deleted before AnySSH launched.
- The real EXE used the Agent Named Pipe to authenticate to a standalone Windows
  OpenSSH Server and created the remote marker ``$SshMarkerPath``.
- The first connection durably persisted TOFU before authentication; a second
  Credential using the same Endpoint connected without another prompt.
- Known Hosts exposed only Endpoint, Algorithm, and SHA-256 Fingerprint
  metadata. Forget Trust required a native Windows confirmation with explicit
  Forget/Cancel actions, and the next connection required TOFU again.
- Durable Trust survived Vault lock/unlock and process restart without another
  Host Key prompt.
- Rotating the standalone OpenSSH Host Key at the same Endpoint produced a
  typed hard-block dialog with Trusted and Received Fingerprints and no Accept
  or Replace action.
- Password/System Agent Credentials, Group, inherited/direct Hosts, and Jump Route
  metadata persisted across process restart.
- PIN, Password, Private Key, Passphrases, Agent Fingerprint, Group, Host,
  Username, and Route markers were absent from Vault files and QA text evidence.
- The SQLCipher database did not expose the plaintext SQLite header.
- Browser error logs were empty.

## Evidence

- ``01-vault-create.png``
- ``02-native-ready.png``
- ``02a-private-key-picker.png``
- ``02a2-private-key-passphrase.png``
- ``02a3-private-key-passphrase-retry.png``
- ``02a4-private-key-imported.png``
- ``02a5-private-key-connected.png``
- ``02b-system-agent-connected.png``
- ``02c-known-hosts.png``
- ``02c-known-host-forget-confirmation.png``
- ``02c2-known-host-forgotten.png``
- ``02c3-tofu-after-forget.png``
- ``03-repository-created.png``
- ``04-vault-wrong-pin.png``
- ``05-vault-reunlocked.png``
- ``06-restart-locked.png``
- ``07-restart-recovered.png``
- ``07a-restart-trusted-connection.png``
- ``08-changed-host-key.png``
- ``process-create.json``
- ``process-restart.json``
- ``process-changed.json``
- ``probe-create.json``
- ``probe-restart.json``
- ``probe-changed.json``
- ``cdp-targets-create.json``
- ``cdp-targets-restart.json``
- ``cdp-targets-changed.json``
- ``console-create.txt``
- ``console-restart.txt``
- ``console-changed.txt``
- ``errors-create.txt``
- ``errors-restart.txt``
- ``errors-changed.txt``
- ``app-create.stdout.log``
- ``app-create.stderr.log``
- ``app-restart.stdout.log``
- ``app-restart.stderr.log``
- ``app-changed.stdout.log``
- ``app-changed.stderr.log``
- ``ssh-fixture.txt``
- ``sshd.stdout.log``
- ``sshd.stderr.log``
- ``sshd-rotated.stdout.log``
- ``sshd-rotated.stderr.log``
- ``rotated-host-key-fingerprint.txt``
- ``native-dialog-driver.txt``
- ``known-host-forget-driver.txt``
"@ | Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "report.md")

  Assert-EvidenceContainsNoPrivateKeySecrets
  Write-Host "Native Windows WebView2 smoke passed: $RunDirectory"
}
catch {
  $_.Exception.ToString() |
    Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "failure.txt")
  throw
}
finally {
  Stop-NativeProcess
  Stop-SshFixture
  Remove-Item Env:ANYSSH_QA_VAULT_ROOT -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_CDP_URL -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_RUN_DIR -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_STAGE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_APP_PID -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_SSH_HOST -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_SSH_PORT -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_SSH_USERNAME -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_AGENT_FINGERPRINT -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_AGENT_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_ENCRYPTED_KEY_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_KEY_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_WRONG_KEY_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_PRIVATE_KEY_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $VaultRoot -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $WebViewDataRoot -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $PrivateKeyMarkerPath -Force -ErrorAction SilentlyContinue
}
