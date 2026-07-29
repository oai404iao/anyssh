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
$GeneratedKeyMarkerPath = Join-Path `
  $env:TEMP `
  "anyssh-windows-generated-key-ok-$Timestamp-$PID.txt"
$ReimportedKeyMarkerPath = Join-Path `
  $env:TEMP `
  "anyssh-windows-reimported-key-ok-$Timestamp-$PID.txt"
$GeneratedExportPath = Join-Path `
  $env:TEMP `
  "anyssh-windows-generated-export-$Timestamp-$PID.key"
$InteractiveReadyPath = Join-Path `
  $env:TEMP `
  "anyssh-windows-interactive-ready-$Timestamp-$PID.txt"
$InteractiveMarkerPath = Join-Path `
  $env:TEMP `
  "anyssh-windows-interactive-ok-$Timestamp-$PID.txt"
$AppearanceFixtureRoot = Join-Path `
  $env:TEMP `
  "anyssh-windows-appearance-$Timestamp-$PID"
$ThemeFixturePath = Join-Path $AppearanceFixtureRoot "windows-aurora.json"
$FontFixturePath = Join-Path $AppearanceFixtureRoot "windows-terminal-font.ttf"
$SnippetMarkerPath = Join-Path `
  $env:TEMP `
  "anyssh-windows-snippet-ok-$Timestamp-$PID.txt"
$script:NativeProcess = $null
$script:NativeWindowHandle = [IntPtr]::Zero
$script:StageRecords = @()
$script:SshdProcess = $null
$script:InteractiveProcess = $null
$script:SshAgentWasRunning = $false
$script:AgentPublicKeyPath = ""
$script:SshPort = 0
$script:SshUsername = ""
$script:AgentFingerprint = ""
$script:AuthorizedKeysPath = ""
$script:EncryptedKeyPath = ""
$script:SshdExecutable = ""
$script:SshHostKeyPath = ""
$script:SshConfigPath = ""
$script:PrivateKeyPassphrase = "windows-key-passphrase"
$script:WrongPrivateKeyPassphrase = "windows-wrong-key-passphrase"
$script:ExportPassphrase = "windows-export-passphrase"
$script:WrongExportPassphrase = "windows-wrong-export-passphrase"
$script:InteractivePort = 0
$script:InteractiveResponse = "otp-$([Guid]::NewGuid().ToString('N'))"
$script:InteractiveUsername = "windows-interactive-user"
$script:LocalForwardMarker = "ANYSSH_WINDOWS_LOCAL_FORWARD_PAYLOAD"
$script:DynamicForwardMarker = "ANYSSH_WINDOWS_DYNAMIC_FORWARD_PAYLOAD"
$script:RemoteForwardMarker = "ANYSSH_WINDOWS_REMOTE_FORWARD_PAYLOAD"
$script:SnippetBodyMarker = "ANYSSH_WINDOWS_SNIPPET_BODY_$([Guid]::NewGuid().ToString('N'))"

New-Item -ItemType Directory -Force -Path $RunDirectory | Out-Null
Remove-Item -LiteralPath $VaultRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $WebViewDataRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $SshFixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $SshMarkerPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $PrivateKeyMarkerPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $GeneratedKeyMarkerPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $ReimportedKeyMarkerPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $GeneratedExportPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $InteractiveReadyPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $InteractiveMarkerPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $AppearanceFixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $SnippetMarkerPath -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $AppearanceFixtureRoot | Out-Null
$BundledFontPath = Join-Path `
  $RootDirectory `
  "apps\client\src\assets\fonts\JetBrainsMonoNerdFontMono-Regular.ttf"
Copy-Item -LiteralPath $BundledFontPath -Destination $FontFixturePath
$FontFixtureHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $FontFixturePath).Hash
$ThemeFixture = @"
{
  "version": 1,
  "label": "Windows Aurora",
  "palette": {
    "background": "#101426",
    "foreground": "#D8E2F0",
    "cursor": "#73F7D0",
    "selectionBackground": "#334466AA",
    "black": "#0B1020",
    "red": "#FF6B7A",
    "green": "#73F7D0",
    "yellow": "#FFD166",
    "blue": "#6CA0FF",
    "magenta": "#C792EA",
    "cyan": "#65D1FF",
    "white": "#D8E2F0",
    "brightBlack": "#5C6784",
    "brightRed": "#FF8A98",
    "brightGreen": "#96FFD9",
    "brightYellow": "#FFE29A",
    "brightBlue": "#91B8FF",
    "brightMagenta": "#DDB3F5",
    "brightCyan": "#8BE0FF",
    "brightWhite": "#FFFFFF"
  }
}
"@
[System.IO.File]::WriteAllText(
  $ThemeFixturePath,
  $ThemeFixture,
  [System.Text.UTF8Encoding]::new($false)
)

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

function Start-KeyboardInteractiveFixture {
  Push-Location $RootDirectory
  try {
    & cargo build --package anyssh-ssh --example keyboard_interactive_server
    if ($LASTEXITCODE -ne 0) {
      throw "Unable to build the controlled Keyboard-interactive fixture."
    }
  }
  finally {
    Pop-Location
  }

  $FixtureExecutable = (
    Resolve-Path (
      Join-Path `
        $RootDirectory `
        "target\debug\examples\keyboard_interactive_server.exe"
    )
  ).Path
  $script:InteractivePort = Get-FreeTcpPort
  $InteractiveStdout = Join-Path $RunDirectory "interactive-server.stdout.log"
  $InteractiveStderr = Join-Path $RunDirectory "interactive-server.stderr.log"
  Remove-Item -LiteralPath $InteractiveReadyPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $InteractiveMarkerPath -Force -ErrorAction SilentlyContinue

  $env:ANYSSH_KBDINT_PORT = [string]$script:InteractivePort
  $env:ANYSSH_KBDINT_RESPONSE = $script:InteractiveResponse
  $env:ANYSSH_KBDINT_MARKER = $InteractiveMarkerPath
  $env:ANYSSH_KBDINT_READY = $InteractiveReadyPath
  try {
    $script:InteractiveProcess = Start-Process `
      -FilePath $FixtureExecutable `
      -WorkingDirectory $RootDirectory `
      -RedirectStandardOutput $InteractiveStdout `
      -RedirectStandardError $InteractiveStderr `
      -PassThru
  }
  finally {
    Remove-Item Env:ANYSSH_KBDINT_PORT -ErrorAction SilentlyContinue
    Remove-Item Env:ANYSSH_KBDINT_RESPONSE -ErrorAction SilentlyContinue
    Remove-Item Env:ANYSSH_KBDINT_MARKER -ErrorAction SilentlyContinue
    Remove-Item Env:ANYSSH_KBDINT_READY -ErrorAction SilentlyContinue
  }

  $FixtureReady = $false
  for ($Attempt = 0; $Attempt -lt 80; $Attempt++) {
    if ($script:InteractiveProcess.HasExited) {
      $Details = Get-Content `
        -LiteralPath $InteractiveStderr `
        -Raw `
        -ErrorAction SilentlyContinue
      throw "The controlled Keyboard-interactive fixture exited early. $Details"
    }
    if (Test-Path -LiteralPath $InteractiveReadyPath -PathType Leaf) {
      $Client = [System.Net.Sockets.TcpClient]::new()
      try {
        $Connect = $Client.ConnectAsync(
          "127.0.0.1",
          $script:InteractivePort
        )
        if ($Connect.Wait(250) -and $Client.Connected) {
          $FixtureReady = $true
          break
        }
      }
      catch {
        # Retry until the controlled server is listening.
      }
      finally {
        $Client.Dispose()
      }
    }
    Start-Sleep -Milliseconds 250
  }
  if (-not $FixtureReady) {
    throw "The controlled Keyboard-interactive fixture did not become ready."
  }

  @"
interactive_host=127.0.0.1
interactive_port=$($script:InteractivePort)
interactive_username=$($script:InteractiveUsername)
"@ | Add-Content -Encoding UTF8 -Path (
    Join-Path $RunDirectory "ssh-fixture.txt"
  )
}

function Stop-KeyboardInteractiveFixture {
  if ($null -ne $script:InteractiveProcess) {
    try {
      if (-not $script:InteractiveProcess.HasExited) {
        Stop-Process -Id $script:InteractiveProcess.Id -Force
        $script:InteractiveProcess.WaitForExit(10000) | Out-Null
      }
    }
    catch {
      # The controlled fixture may already have exited during cleanup.
    }
    finally {
      $script:InteractiveProcess = $null
    }
  }
  Remove-Item -LiteralPath $InteractiveReadyPath -Force -ErrorAction SilentlyContinue
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
  Remove-Item -LiteralPath $GeneratedKeyMarkerPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $ReimportedKeyMarkerPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $GeneratedExportPath -Force -ErrorAction SilentlyContinue
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
  $script:AuthorizedKeysPath = $AuthorizedKeysPath

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
AllowTcpForwarding yes
AllowAgentForwarding no
X11Forwarding no
PrintMotd no
LogLevel ERROR
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
encrypted_key_fixture=ephemeral
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
  # Inputs used only by the external Playwright/native-dialog drivers must not
  # survive from a previous stage into the next AnySSH process environment.
  Remove-Item Env:ANYSSH_WINDOWS_ENCRYPTED_KEY_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_KEY_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_WRONG_KEY_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_GENERATED_EXPORT_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_EXPORT_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_WRONG_EXPORT_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_VAULT_PIN -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_WRONG_VAULT_PIN -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_AUTHORIZED_KEYS_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_GENERATED_KEY_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_REIMPORTED_KEY_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_PRIVATE_KEY_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_INTERACTIVE_RESPONSE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_INTERACTIVE_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_LOCAL_FORWARD_MARKER -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_DYNAMIC_FORWARD_MARKER -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_REMOTE_FORWARD_MARKER -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_THEME_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_FONT_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_SNIPPET_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_SNIPPET_BODY_MARKER -ErrorAction SilentlyContinue

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
  $env:ANYSSH_WINDOWS_GENERATED_EXPORT_PATH = $GeneratedExportPath
  $env:ANYSSH_WINDOWS_EXPORT_PASSPHRASE = $script:ExportPassphrase
  $env:ANYSSH_WINDOWS_WRONG_EXPORT_PASSPHRASE = $script:WrongExportPassphrase
  $env:ANYSSH_WINDOWS_VAULT_PIN = "246810"
  $env:ANYSSH_WINDOWS_WRONG_VAULT_PIN = "000000"
  $env:ANYSSH_WINDOWS_AUTHORIZED_KEYS_PATH = $script:AuthorizedKeysPath
  $env:ANYSSH_WINDOWS_GENERATED_KEY_MARKER_PATH = $GeneratedKeyMarkerPath
  $env:ANYSSH_WINDOWS_REIMPORTED_KEY_MARKER_PATH = $ReimportedKeyMarkerPath
  $env:ANYSSH_WINDOWS_PRIVATE_KEY_MARKER_PATH = $PrivateKeyMarkerPath
  $env:ANYSSH_WINDOWS_INTERACTIVE_HOST = "127.0.0.1"
  $env:ANYSSH_WINDOWS_INTERACTIVE_PORT = [string]$script:InteractivePort
  $env:ANYSSH_WINDOWS_INTERACTIVE_USERNAME = $script:InteractiveUsername
  $env:ANYSSH_WINDOWS_INTERACTIVE_RESPONSE = $script:InteractiveResponse
  $env:ANYSSH_WINDOWS_INTERACTIVE_MARKER_PATH = $InteractiveMarkerPath
  $env:ANYSSH_WINDOWS_LOCAL_FORWARD_MARKER = $script:LocalForwardMarker
  $env:ANYSSH_WINDOWS_DYNAMIC_FORWARD_MARKER = $script:DynamicForwardMarker
  $env:ANYSSH_WINDOWS_REMOTE_FORWARD_MARKER = $script:RemoteForwardMarker
  $env:ANYSSH_WINDOWS_THEME_PATH = $ThemeFixturePath
  $env:ANYSSH_WINDOWS_FONT_PATH = $FontFixturePath
  $env:ANYSSH_WINDOWS_SNIPPET_MARKER_PATH = $SnippetMarkerPath
  $env:ANYSSH_WINDOWS_SNIPPET_BODY_MARKER = $script:SnippetBodyMarker

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
    "Windows QA generated key",
    "Windows QA generated RSA",
    "Windows QA generated key host",
    "Windows QA reimported key",
    "Windows QA reimported key host",
    "Windows QA system agent",
    "Windows QA interactive",
    "Windows QA agent host",
    "Windows QA jump",
    "Windows QA target",
    "Windows QA group",
    "Windows QA route",
    "Windows QA snippet",
    "Windows Aurora",
    "target.internal",
    "windows-user",
    $script:InteractiveUsername,
    $script:InteractiveResponse,
    $script:EncryptedKeyPath,
    $script:ExportPassphrase,
    $script:WrongExportPassphrase,
    $GeneratedExportPath,
    $script:LocalForwardMarker,
    $script:DynamicForwardMarker,
    $script:RemoteForwardMarker,
    $script:SnippetBodyMarker,
    $SnippetMarkerPath,
    $ThemeFixturePath,
    $FontFixturePath,
    $script:SshUsername,
    $script:AgentFingerprint
  )
  $FontAssetsRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $VaultRoot "font-assets")
  )
  foreach ($File in Get-ChildItem -LiteralPath $VaultRoot -Recurse -File) {
    $FullName = [System.IO.Path]::GetFullPath($File.FullName)
    if ($FullName.StartsWith(
        "$FontAssetsRoot$([System.IO.Path]::DirectorySeparatorChar)",
        [System.StringComparison]::OrdinalIgnoreCase
      )) {
      continue
    }
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

function Assert-EvidenceContainsNoSensitiveFixtureValues {
  $Needles = @(
    $script:PrivateKeyPassphrase,
    $script:WrongPrivateKeyPassphrase,
    $script:ExportPassphrase,
    $script:WrongExportPassphrase,
    $script:EncryptedKeyPath,
    $GeneratedExportPath,
    $script:InteractiveResponse,
    $script:AgentFingerprint,
    $script:LocalForwardMarker,
    $script:DynamicForwardMarker,
    $script:RemoteForwardMarker,
    $ThemeFixturePath,
    $FontFixturePath,
    "BEGIN OPENSSH PRIVATE KEY"
  )
  $TextOnlyNeedles = @(
    "246810",
    "000000"
  )
  $TextExtensions = @(
    ".json",
    ".log",
    ".md",
    ".txt"
  )
  foreach ($File in Get-ChildItem -LiteralPath $RunDirectory -Recurse -File) {
    $Text = [System.Text.Encoding]::UTF8.GetString(
      [System.IO.File]::ReadAllBytes($File.FullName)
    )
    foreach ($Needle in $Needles) {
      if ($Text.Contains($Needle)) {
        throw "A Windows sensitive fixture value leaked into QA evidence file '$($File.Name)'."
      }
    }
    if ($TextExtensions -contains $File.Extension.ToLowerInvariant()) {
      foreach ($Needle in $TextOnlyNeedles) {
        if ($Text.Contains($Needle)) {
          throw "A Windows PIN fixture leaked into QA text evidence file '$($File.Name)'."
        }
      }
    }
  }
}

function Test-WindowsExportReparseGuard {
  $EvidencePath = Join-Path $RunDirectory "windows-export-reparse-test.txt"
  $StdoutPath = Join-Path $RunDirectory "windows-export-reparse-test.stdout.tmp"
  $StderrPath = Join-Path $RunDirectory "windows-export-reparse-test.stderr.tmp"
  try {
    $CargoProcess = Start-Process `
      -FilePath (Get-Command cargo.exe -ErrorAction Stop).Source `
      -WorkingDirectory $RootDirectory `
      -ArgumentList @(
        "test",
        "--package",
        "anyssh-app",
        "windows_private_key_export_rejects_reparse_points_and_alternate_streams",
        "--",
        "--nocapture"
      ) `
      -RedirectStandardOutput $StdoutPath `
      -RedirectStandardError $StderrPath `
      -NoNewWindow `
      -Wait `
      -PassThru
    @(
      Get-Content -LiteralPath $StdoutPath -ErrorAction SilentlyContinue
      Get-Content -LiteralPath $StderrPath -ErrorAction SilentlyContinue
    ) | Set-Content -Encoding UTF8 -Path $EvidencePath
    if ($CargoProcess.ExitCode -ne 0) {
      throw "The Windows Private Key export reparse-point guard test failed."
    }
  }
  finally {
    Remove-Item -LiteralPath $StdoutPath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $StderrPath -Force -ErrorAction SilentlyContinue
  }
}

try {
  Test-WindowsExportReparseGuard
  Start-SshFixture
  Start-KeyboardInteractiveFixture
  Start-NativeStage -Stage "create"
  Stop-NativeProcess
  if (-not (Test-Path -LiteralPath $SnippetMarkerPath -PathType Leaf)) {
    throw "The Windows Snippet did not create its remote marker."
  }
  $SnippetMarker = Get-Content -LiteralPath $SnippetMarkerPath -Raw
  if (-not $SnippetMarker.Contains("$($script:SnippetBodyMarker)_ONE") -or
    -not $SnippetMarker.Contains("$($script:SnippetBodyMarker)_TWO")) {
    throw "The Windows Snippet remote marker was invalid."
  }
  $ManagedFont = Get-ChildItem `
    -LiteralPath (Join-Path $VaultRoot "font-assets") `
    -Filter "font-*.ttf" `
    -File |
    Select-Object -First 1
  if ($null -eq $ManagedFont) {
    throw "The Windows imported Font managed asset was not created."
  }
  $ManagedFontHash = (
    Get-FileHash -Algorithm SHA256 -LiteralPath $ManagedFont.FullName
  ).Hash
  if ($ManagedFontHash -ne $FontFixtureHash) {
    throw "The Windows imported Font managed asset digest did not match its source."
  }
  Remove-Item -LiteralPath $AppearanceFixtureRoot -Recurse -Force
  if ((Test-Path -LiteralPath $ThemeFixturePath -PathType Leaf) -or
    (Test-Path -LiteralPath $FontFixturePath -PathType Leaf)) {
    throw "The Windows Appearance source fixtures still existed before restart."
  }
  if (-not (Test-Path -LiteralPath $SshMarkerPath -PathType Leaf)) {
    throw "The Windows System Agent session did not create its remote marker."
  }
  $SshMarker = Get-Content -LiteralPath $SshMarkerPath -Raw
  if (-not $SshMarker.Contains("ANYSSH_WINDOWS_AGENT_OK")) {
    throw "The Windows System Agent remote marker was invalid."
  }
  if (-not $SshMarker.Contains("ANYSSH_WINDOWS_AGENT_TAB_SURVIVED")) {
    throw "The Windows System Agent Session did not survive closing the second Tab."
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
  if (-not (Test-Path -LiteralPath $GeneratedKeyMarkerPath -PathType Leaf)) {
    throw "The Windows generated Private Key session did not create its remote marker."
  }
  if (-not (
      (Get-Content -LiteralPath $GeneratedKeyMarkerPath -Raw).Contains(
        "ANYSSH_WINDOWS_GENERATED_KEY_OK"
      )
    )) {
    throw "The Windows generated Private Key remote marker was invalid."
  }
  if (-not (Test-Path -LiteralPath $ReimportedKeyMarkerPath -PathType Leaf)) {
    throw "The Windows reimported Private Key session did not create its remote marker."
  }
  if (-not (
      (Get-Content -LiteralPath $ReimportedKeyMarkerPath -Raw).Contains(
        "ANYSSH_WINDOWS_REIMPORTED_KEY_OK"
      )
    )) {
    throw "The Windows reimported Private Key remote marker was invalid."
  }
  if (Test-Path -LiteralPath $GeneratedExportPath -PathType Leaf) {
    throw "The Windows generated Private Key export source was not removed after reimport."
  }
  if (-not (Test-Path -LiteralPath $InteractiveMarkerPath -PathType Leaf)) {
    throw "The Windows Keyboard-interactive session did not create its marker."
  }
  $InteractiveMarker = Get-Content -LiteralPath $InteractiveMarkerPath -Raw
  if (-not $InteractiveMarker.Contains("interactive-ok")) {
    throw "The Windows Keyboard-interactive marker was invalid."
  }
  Assert-VaultFilesAreEncrypted

  Start-NativeStage -Stage "restart"
  Stop-NativeProcess
  Assert-VaultFilesAreEncrypted

  Rotate-SshFixtureHostKey
  Start-NativeStage -Stage "changed"
  Stop-NativeProcess

  Stop-KeyboardInteractiveFixture
  Stop-SshFixture
  Assert-VaultFilesAreEncrypted
  Assert-EvidenceContainsNoSensitiveFixtureValues

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
- The real EXE generated Ed25519 and RSA 4096 Credentials inside Rust and
  exposed only Algorithm, SHA-256 Fingerprint, and OpenSSH Public Key metadata
  to WebView2.
- The generated Ed25519 Credential authenticated to the standalone OpenSSH
  Server and created ``$GeneratedKeyMarkerPath``.
- Windows Credential UI required one wrong/correct PIN Step-up and one
  mismatched/correct new Export Passphrase confirmation. The Native Save Dialog
  wrote an encrypted OpenSSH Key without sending PIN, Passphrase, or Path
  through WebView IPC.
- The exported file was owned by the current Windows user and used a protected
  owner-only DACL instead of inheriting broader permissions from its directory.
- Windows junction-parent and alternate-data-stream fixtures were rejected
  before Private Key export, leaving the real destination untouched.
- Reimporting that exported Key required the new Passphrase, after which the
  source file was deleted and the Credential authenticated to OpenSSH to create
  ``$ReimportedKeyMarkerPath``.
- Windows OpenSSH Authentication Agent enumerated the selected SHA-256 Identity
  through Rust; the temporary Private Key file was deleted before AnySSH launched.
- The real EXE used the Agent Named Pipe to authenticate to a standalone Windows
  OpenSSH Server and created the remote marker ``$SshMarkerPath``.
- Native Windows Theme and Font pickers imported a strict JSON Terminal Theme
  and TTF without adding source Path or Font bytes to WebView IPC. Picker Path
  inputs were provided only after AnySSH launched, the managed Font digest
  matched its source, and both source fixtures were deleted before restart.
- App Light Theme and the imported Terminal Theme/Font updated the already
  mounted xterm.js instance through the restricted ``anyssh-font`` protocol.
  The selected resources and loaded Font survived process restart.
- A Record-AEAD, variable-aware multi-line Snippet kept its Body out of list
  projections, required complete Preview/confirmation, wrote
  ``$SnippetMarkerPath`` only through the selected SSH PTY, and retained only
  its summary across process restart.
- The Agent Session started real Local, unauthenticated Dynamic SOCKS5, and
  Remote Loopback Forwards through the same russh transport. External TCP
  clients completed all three paths, Dynamic Stop closed its listener, and
  Session Disconnect removed the remaining Local and Remote endpoints.
- The real EXE used Quick Connection against a controlled russh Server,
  displayed a masked Keyboard-interactive Challenge in a second Session Tab,
  and created the marker ``$InteractiveMarkerPath`` while the Agent Session
  remained connected.
- Closing the Keyboard-interactive Tab removed its Session-scoped Local Forward,
  left the Agent Tab connected, and allowed it to append
  ``ANYSSH_WINDOWS_AGENT_TAB_SURVIVED`` to its remote marker.
- The Interactive Credential persisted only Label/Username metadata, while the
  session response remained absent from Vault files and QA evidence.
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
- Appearance, Theme, imported Font, Snippet summary, Password/System Agent
  Credentials, Group, inherited/direct Hosts, and Jump Route metadata persisted
  across process restart.
- Vault Lock removed an active Session Forward listener before returning to the
  PIN gate.
- PIN, Password, Private Key, Passphrases, Agent Fingerprint, Group, Host,
  Username, Route, Theme source Path, and Snippet Body markers were absent from
  encrypted Vault files. Private Key material, Passphrases, the Agent
  Fingerprint, Theme/Font source Paths, and the Keyboard-interactive Response
  were also absent from QA text evidence.
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
- ``02g-generated-public-key.png``
- ``02g2-generated-key-connected.png``
- ``02g3-generated-key-exported.png``
- ``generated-export-acl.txt``
- ``windows-export-reparse-test.txt``
- ``02g4-reimported-key-connected.png``
- ``02a6-private-key-export-picker.png``
- ``02a7-private-key-export-pin.png``
- ``02a8-private-key-export-pin-retry.png``
- ``02a9-private-key-export-passphrase.png``
- ``02a10-private-key-export-passphrase-retry.png``
- ``02a11-generated-key-reimport-picker.png``
- ``02a12-generated-key-reimport-passphrase.png``
- ``02a13-generated-key-reimport-passphrase-retry.png``
- ``02b-system-agent-connected.png``
- ``02b3-terminal-theme-picker.png``
- ``02b4-terminal-font-picker.png``
- ``02b5-appearance-imported.png``
- ``02b6-terminal-imported-font.png``
- ``02b7-snippet-confirmation.png``
- ``02b8-snippet-terminal-output.png``
- ``02b2-port-forwarding.png``
- ``02c-known-hosts.png``
- ``02c-known-host-forget-confirmation.png``
- ``02c2-known-host-forgotten.png``
- ``02c3-tofu-after-forget.png``
- ``02d-interactive-challenge.png``
- ``02e-interactive-connected.png``
- ``02f-agent-tab-after-close.png``
- ``03-repository-created.png``
- ``03a-vault-lock-forwarding.png``
- ``04-vault-wrong-pin.png``
- ``05-vault-reunlocked.png``
- ``06-restart-locked.png``
- ``07-restart-recovered.png``
- ``07a-restart-trusted-connection.png``
- ``07b-restart-appearance.png``
- ``07c-restart-snippet-summary.png``
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
- ``interactive-server.stdout.log``
- ``interactive-server.stderr.log``
- ``native-dialog-driver.txt``
- ``key-export-driver.txt``
- ``generated-key-reimport-driver.txt``
- ``known-host-forget-driver.txt``
- ``appearance-import-driver.txt``
"@ | Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "report.md")

  Assert-EvidenceContainsNoSensitiveFixtureValues
  Write-Host "Native Windows WebView2 smoke passed: $RunDirectory"
}
catch {
  $_.Exception.ToString() |
    Set-Content -Encoding UTF8 -Path (Join-Path $RunDirectory "failure.txt")
  throw
}
finally {
  Stop-NativeProcess
  Stop-KeyboardInteractiveFixture
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
  Remove-Item Env:ANYSSH_WINDOWS_GENERATED_EXPORT_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_EXPORT_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_WRONG_EXPORT_PASSPHRASE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_VAULT_PIN -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_WRONG_VAULT_PIN -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_AUTHORIZED_KEYS_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_GENERATED_KEY_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_REIMPORTED_KEY_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_PRIVATE_KEY_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_INTERACTIVE_HOST -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_INTERACTIVE_PORT -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_INTERACTIVE_USERNAME -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_INTERACTIVE_RESPONSE -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_INTERACTIVE_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_THEME_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_FONT_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_SNIPPET_MARKER_PATH -ErrorAction SilentlyContinue
  Remove-Item Env:ANYSSH_WINDOWS_SNIPPET_BODY_MARKER -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $VaultRoot -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $WebViewDataRoot -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $PrivateKeyMarkerPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $GeneratedKeyMarkerPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $ReimportedKeyMarkerPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $GeneratedExportPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $InteractiveReadyPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $InteractiveMarkerPath -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $AppearanceFixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $SnippetMarkerPath -Force -ErrorAction SilentlyContinue
}
