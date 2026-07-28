param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$AppProcessId = [int]$env:ANYSSH_WINDOWS_APP_PID
$PrivateKeyPath = $env:ANYSSH_WINDOWS_ENCRYPTED_KEY_PATH
$Passphrase = $env:ANYSSH_WINDOWS_KEY_PASSPHRASE
$WrongPassphrase = $env:ANYSSH_WINDOWS_WRONG_KEY_PASSPHRASE
$RunDirectory = $env:ANYSSH_WINDOWS_RUN_DIR

foreach ($RequiredValue in @(
  $AppProcessId,
  $PrivateKeyPath,
  $Passphrase,
  $WrongPassphrase,
  $RunDirectory
)) {
  if ($null -eq $RequiredValue -or [string]::IsNullOrWhiteSpace([string]$RequiredValue)) {
    throw "Windows native dialog automation is missing a required input."
  }
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -ReferencedAssemblies @(
  "System.Drawing.dll",
  "System.Windows.Forms.dll",
  "UIAutomationClient.dll",
  "UIAutomationTypes.dll"
) -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Automation;
using System.Windows.Forms;

public static class AnySshNativeDialogDriver
{
    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindow(IntPtr window);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr window, StringBuilder title, int capacity);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr window, out RECT rectangle);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr window);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr window, int command);

    public static IntPtr WaitForWindow(
        int expectedProcessId,
        string titleFragment,
        int timeoutMilliseconds)
    {
        DateTime deadline = DateTime.UtcNow.AddMilliseconds(timeoutMilliseconds);
        while (DateTime.UtcNow < deadline)
        {
            IntPtr window = FindVisibleWindow(expectedProcessId, titleFragment);
            if (window == IntPtr.Zero && expectedProcessId != 0)
            {
                window = FindVisibleWindow(0, titleFragment);
            }
            if (window != IntPtr.Zero)
            {
                return window;
            }
            Thread.Sleep(100);
        }
        return IntPtr.Zero;
    }

    public static bool WaitForWindowToClose(IntPtr window, int timeoutMilliseconds)
    {
        DateTime deadline = DateTime.UtcNow.AddMilliseconds(timeoutMilliseconds);
        while (DateTime.UtcNow < deadline)
        {
            if (!IsWindow(window) || !IsWindowVisible(window))
            {
                return true;
            }
            Thread.Sleep(50);
        }
        return false;
    }

    public static void Activate(IntPtr window)
    {
        const int SW_RESTORE = 9;
        ShowWindow(window, SW_RESTORE);
        SetForegroundWindow(window);
        Thread.Sleep(250);
    }

    public static void ChooseFile(IntPtr window, string path)
    {
        Activate(window);
        SendKeys.SendWait("%n");
        Thread.Sleep(200);
        SendKeys.SendWait(EscapeSendKeys(path));
        Thread.Sleep(200);
        SendKeys.SendWait("{ENTER}");
    }

    public static void EnterPassphrase(IntPtr window, string passphrase)
    {
        Activate(window);
        AutomationElement root = AutomationElement.FromHandle(window);
        AutomationElementCollection edits = root.FindAll(
            TreeScope.Descendants,
            new PropertyCondition(
                AutomationElement.ControlTypeProperty,
                ControlType.Edit));
        if (edits.Count == 0)
        {
            throw new InvalidOperationException(
                "The native Credential UI exposed no password field.");
        }

        AutomationElement password = edits[edits.Count - 1];
        password.SetFocus();
        Thread.Sleep(200);
        SendKeys.SendWait(EscapeSendKeys(passphrase));
        Thread.Sleep(200);
    }

    public static void Submit(IntPtr window)
    {
        Activate(window);
        SendKeys.SendWait("{ENTER}");
    }

    public static void CaptureWindow(IntPtr window, string path)
    {
        RECT rectangle;
        if (!GetWindowRect(window, out rectangle))
        {
            throw new InvalidOperationException(
                "Unable to resolve the native dialog bounds.");
        }
        int width = Math.Max(1, rectangle.Right - rectangle.Left);
        int height = Math.Max(1, rectangle.Bottom - rectangle.Top);
        using (Bitmap bitmap = new Bitmap(width, height))
        using (Graphics graphics = Graphics.FromImage(bitmap))
        {
            graphics.CopyFromScreen(
                rectangle.Left,
                rectangle.Top,
                0,
                0,
                new Size(width, height));
            bitmap.Save(path, ImageFormat.Png);
        }
    }

    private static IntPtr FindVisibleWindow(int expectedProcessId, string titleFragment)
    {
        IntPtr matched = IntPtr.Zero;
        EnumWindows((window, parameter) =>
        {
            uint processId;
            GetWindowThreadProcessId(window, out processId);
            if ((expectedProcessId != 0 && processId != expectedProcessId)
                || !IsWindowVisible(window))
            {
                return true;
            }

            StringBuilder title = new StringBuilder(512);
            GetWindowText(window, title, title.Capacity);
            if (title.ToString().IndexOf(
                    titleFragment,
                    StringComparison.OrdinalIgnoreCase) >= 0)
            {
                matched = window;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return matched;
    }

    private static string EscapeSendKeys(string value)
    {
        StringBuilder escaped = new StringBuilder(value.Length);
        foreach (char character in value)
        {
            if ("+^%~(){}[]".IndexOf(character) >= 0)
            {
                escaped.Append('{').Append(character).Append('}');
            }
            else
            {
                escaped.Append(character);
            }
        }
        return escaped.ToString();
    }
}
"@

$FileDialog = [AnySshNativeDialogDriver]::WaitForWindow(
  $AppProcessId,
  "Open SSH private key",
  60000
)
if ($FileDialog -eq [IntPtr]::Zero) {
  throw "The native Windows Private Key picker did not appear."
}
[AnySshNativeDialogDriver]::CaptureWindow(
  $FileDialog,
  (Join-Path $RunDirectory "02a-private-key-picker.png")
)
[AnySshNativeDialogDriver]::ChooseFile($FileDialog, $PrivateKeyPath)
if (-not [AnySshNativeDialogDriver]::WaitForWindowToClose($FileDialog, 30000)) {
  throw "The native Windows Private Key picker did not close."
}

$FirstPrompt = [AnySshNativeDialogDriver]::WaitForWindow(
  $AppProcessId,
  "Unlock SSH private key",
  60000
)
if ($FirstPrompt -eq [IntPtr]::Zero) {
  throw "The native Windows Private Key Passphrase prompt did not appear."
}
[AnySshNativeDialogDriver]::EnterPassphrase($FirstPrompt, $WrongPassphrase)
[AnySshNativeDialogDriver]::CaptureWindow(
  $FirstPrompt,
  (Join-Path $RunDirectory "02a2-private-key-passphrase.png")
)
[AnySshNativeDialogDriver]::Submit($FirstPrompt)
if (-not [AnySshNativeDialogDriver]::WaitForWindowToClose($FirstPrompt, 30000)) {
  throw "The first native Windows Passphrase prompt did not close."
}

$RetryPrompt = [AnySshNativeDialogDriver]::WaitForWindow(
  $AppProcessId,
  "Unlock SSH private key",
  60000
)
if ($RetryPrompt -eq [IntPtr]::Zero) {
  throw "The native Windows Passphrase retry prompt did not appear."
}
[AnySshNativeDialogDriver]::EnterPassphrase($RetryPrompt, $Passphrase)
[AnySshNativeDialogDriver]::CaptureWindow(
  $RetryPrompt,
  (Join-Path $RunDirectory "02a3-private-key-passphrase-retry.png")
)
[AnySshNativeDialogDriver]::Submit($RetryPrompt)
if (-not [AnySshNativeDialogDriver]::WaitForWindowToClose($RetryPrompt, 30000)) {
  throw "The native Windows Passphrase retry prompt did not close."
}

"PASS" | Set-Content -Encoding ASCII -Path (
  Join-Path $RunDirectory "native-dialog-driver.txt"
)
