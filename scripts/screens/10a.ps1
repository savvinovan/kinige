# Screens of slice 10a (the Windows build) in the real window on a GitHub Windows runner:
#   powershell -NoProfile -File scripts/screens/10a.ps1 -Installer <Kinige-setup.exe>
# Windows PowerShell 5.1 (the runner has it).
#
# Data are synthetic (`seed_demo`) in a temporary directory; the real profile is not touched, except in the last phase, which
# starts the app once WITHOUT the data overrides on the fresh runner to prove the documented default locations.
# The page is driven over the WebView2 DevTools channel (scripts/screens/cdp.mjs, Playwright connectOverCDP): the scenario
# starts the app with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port> (localhost only, set only here;
# the app has no code for it). UI Automation was tried first and hangs on the WebView2 tree. Each screenshot is the
# rectangle of one Kinige window (the real title bar included), taken with CopyFromScreen after the window is raised; before
# it, the text and accessible names of that page are checked for personal paths and names (see Test-Private).
# The result goes to -Out (docs/screens/10a); debug dumps go to $Work\debug and never into the repository.
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [string]$Out = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'docs\screens\10a'),
  [string]$Work = (Join-Path $(if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }) 'kinige-screens')
)

$ErrorActionPreference = 'Stop'
$Repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Debug = Join-Path $Work 'debug'
$Data = Join-Path $Work 'data'
$Mirror = Join-Path $Work 'mirror'
$AppDir = Join-Path $Work 'app'
$script:Fails = 0
# The name of the "Work" chat in the synthetic data (Russian), spelled with code points: Windows PowerShell 5.1 reads a
# UTF-8 file without a BOM as ANSI.
$WorkChat = -join ([char[]](0x0420, 0x0430, 0x0431, 0x043E, 0x0442, 0x0430))
$script:AppProcess = $null
$Port = 9222

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing

# C# 5 only (Windows PowerShell compiles with the .NET Framework compiler).
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Win {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
  public struct DEVMODE {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
    public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra;
    public int dmFields;
    public int dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
    public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
    public short dmLogPixels;
    public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
    public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
  }

  [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
  [DllImport("user32.dll", CharSet = CharSet.Ansi)] static extern bool EnumDisplaySettings(string device, int mode, ref DEVMODE dm);
  [DllImport("user32.dll", CharSet = CharSet.Ansi)] static extern int ChangeDisplaySettings(ref DEVMODE dm, int flags);

  public static void MakeDpiAware() { SetProcessDPIAware(); }

  // Picks 1920x1080 (or the largest 32-bit mode) and returns "WxH" of the mode that was applied, or "unchanged ...".
  public static string SetBestResolution() {
    DEVMODE best = new DEVMODE(); bool have = false; long bestArea = 0;
    DEVMODE dm = new DEVMODE(); dm.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE));
    for (int i = 0; EnumDisplaySettings(null, i, ref dm); i++) {
      if (dm.dmBitsPerPel != 32) continue;
      long area = (long)dm.dmPelsWidth * dm.dmPelsHeight;
      if (dm.dmPelsWidth == 1920 && dm.dmPelsHeight == 1080) { best = dm; have = true; break; }
      if (area > bestArea) { best = dm; have = true; bestArea = area; }
    }
    if (!have) return "unchanged: no 32-bit mode";
    int rc = ChangeDisplaySettings(ref best, 0);
    return (rc == 0 ? "" : "failed(" + rc + ") ") + best.dmPelsWidth + "x" + best.dmPelsHeight;
  }

  // Puts the window on top of everything (without keeping it topmost) so the screen copy shows only it.
  public static void Raise(IntPtr h) {
    ShowWindow(h, 9); // SW_RESTORE
    SetWindowPos(h, new IntPtr(-1), 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0040); // HWND_TOPMOST, NOSIZE|NOMOVE|SHOWWINDOW
    SetWindowPos(h, new IntPtr(-2), 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0040); // HWND_NOTOPMOST
    // A background process may not take the foreground; a tap on Alt lifts that restriction for the next call.
    keybd_event(0x12, 0, 0, UIntPtr.Zero);
    keybd_event(0x12, 0, 2, UIntPtr.Zero);
    SetForegroundWindow(h);
  }

  public static bool IsForeground(IntPtr h) { return GetForegroundWindow() == h; }

  // Fits the window into the screen (the runner screen may be smaller than the window).
  public static void MoveTo(IntPtr h, int x, int y, int w, int hgt) { SetWindowPos(h, IntPtr.Zero, x, y, w, hgt, 0x0004 | 0x0040); }

  // The visible frame of the window (without the invisible resize border and the shadow).
  public static Rectangle Bounds(IntPtr h) {
    RECT r;
    if (DwmGetWindowAttribute(h, 9, out r, Marshal.SizeOf(typeof(RECT))) != 0) GetWindowRect(h, out r);
    return new Rectangle(r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top);
  }

  public static string Capture(IntPtr h, string path) {
    Rectangle b = Bounds(h);
    using (Bitmap bmp = new Bitmap(b.Width, b.Height, PixelFormat.Format32bppArgb)) {
      using (Graphics g = Graphics.FromImage(bmp)) { g.CopyFromScreen(b.Left, b.Top, 0, 0, b.Size, CopyPixelOperation.SourceCopy); }
      bmp.Save(path, ImageFormat.Png);
    }
    return b.Width + "x" + b.Height + " at " + b.Left + "," + b.Top;
  }
}
'@

$AE = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]

# ---- Helpers ----------------------------------------------------------------------------------------------------------

function Say([string]$text) { Write-Host ((Get-Date).ToString('HH:mm:ss') + ' ' + $text) }

function Fail([string]$text) {
  Write-Host ((Get-Date).ToString('HH:mm:ss') + " ERROR: $text")
  $script:Fails++
}

function Wait-For([scriptblock]$condition, [int]$seconds, [string]$what) {
  $deadline = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $deadline) {
    $value = & $condition
    if ($value) { return $value }
    Start-Sleep -Milliseconds 500
  }
  Fail "timeout: $what"
  return $null
}

# Top-level windows of the app process that have a title (UI Automation, children of the desktop only: cheap and reliable;
# the helper windows of the event loop have no title).
function Get-AppWindow([string]$title) {
  if (-not $script:AppProcess) { return $null }
  $condition = New-Object System.Windows.Automation.PropertyCondition($AE::ProcessIdProperty, $script:AppProcess.Id)
  foreach ($w in $AE::RootElement.FindAll($Scope::Children, $condition)) { if ($w.Current.Name -eq $title) { return $w } }
  return $null
}

function Focus-Window([string]$title) {
  $window = Get-AppWindow $title
  if ($window) { [Win]::Raise((New-Object System.IntPtr($window.Current.NativeWindowHandle))) }
}

# One action on a page over the DevTools protocol; returns the exit code and the output of cdp.mjs.
function Invoke-Cdp([string]$which, [string]$action, [string[]]$arguments = @()) {
  $ErrorActionPreference = 'Continue'
  $output = & node (Join-Path $Repo 'scripts\screens\cdp.mjs') $Port $which $action @arguments 2>&1 | Out-String
  return [pscustomobject]@{ Code = $LASTEXITCODE; Output = $output.Trim() }
}

function Cdp-Ok([string]$which, [string]$action, [string[]]$arguments = @()) {
  $result = Invoke-Cdp $which $action $arguments
  if ($result.Code -ne 0) { Fail "cdp $which $action $($arguments -join ' '): $($result.Output)"; return $false }
  Say "cdp $which ${action}: $($arguments -join ' ')"
  return $true
}

# Visible text and accessible names of a page as one string.
function Get-PageText([string]$which) {
  $result = Invoke-Cdp $which 'text'
  if ($result.Code -ne 0) { Fail "cdp $which text: $($result.Output)"; return '' }
  return ($result.Output | ConvertFrom-Json).all
}

function Test-Has([string]$which, [string]$text) { return (Invoke-Cdp $which 'has' @($text)).Code -eq 0 }

# Presses a control and gives the page a moment to react.
function Press([string]$which, [string]$name) {
  if (-not (Cdp-Ok $which 'click' @($name))) { Dump-Failure "press-$name" $which }
  Start-Sleep -Milliseconds 1200
}

function Dump-Failure([string]$label, [string]$which) {
  $safe = $label -replace '[^A-Za-z0-9-]', '_'
  Set-Content -Path (Join-Path $Debug "fail-$safe.txt") -Value (Get-PageText $which) -Encoding UTF8
}

# Personal data must not reach a screenshot: no profile paths, no real names (the identifier me.savvinov.kinige is fine).
function Test-Private([string]$text, [string]$label) {
  $hit = [regex]::Match($text, 'C:\\Users\\|runneradmin|savvinovan|alexandr')
  if ($hit.Success) { Fail "personal paths or names in the text of $label ($($hit.Value))" }
}

# The window title (what UI Automation calls the name) of each page: the main window is "Kinige", the other one "Settings".
function Title-Of([string]$which) { if ($which -eq 'settings') { 'Settings' } else { 'Kinige' } }

function Shoot([string]$name, [string]$which) {
  $title = Title-Of $which
  $window = Get-AppWindow $title
  if (-not $window) { Fail "no window '$title' for $name"; return }
  Invoke-Cdp $which 'ready' | Out-Null
  $text = Get-PageText $which
  Set-Content -Path (Join-Path $Debug "shot-$name.txt") -Value $text -Encoding UTF8
  if ($text.Length -lt 50) { Fail "the text of $name is empty, so the private-path check could not run" }
  Test-Private $text $name
  $handle = New-Object System.IntPtr($window.Current.NativeWindowHandle)
  [Win]::Raise($handle)
  Start-Sleep -Seconds 1
  Say ("shot ${name}: " + [Win]::Capture($handle, (Join-Path $Out "$name.png")))
}

function Start-App([string]$exe, [bool]$overrides) {
  # The DevTools port of the web view: localhost only (Chromium's default), exists only while the scenario runs the app.
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
  if ($overrides) {
    $env:KINIGE_DATA_DIR = $Data
    $env:KINIGE_MIRROR_DIR = $Mirror
  } else {
    Remove-Item Env:KINIGE_DATA_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:KINIGE_MIRROR_DIR -ErrorAction SilentlyContinue
  }
  $script:AppProcess = Start-Process -FilePath $exe -PassThru
  $window = Wait-For { Get-AppWindow 'Kinige' } 90 'the main window'
  if ($window) {
    Wait-For { (Invoke-Cdp 'main' 'ready').Code -eq 0 } 90 'the main page over DevTools' | Out-Null
    [Win]::MoveTo((New-Object System.IntPtr($window.Current.NativeWindowHandle)), 20, 20, 1280, 800)
    Focus-Window 'Kinige'
    Start-Sleep -Seconds 2
  }
  Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
  return $window
}

function Stop-App {
  if ($script:AppProcess -and -not $script:AppProcess.HasExited) {
    # TerminateProcess: the hardest kill there is on Windows (the equivalent of kill -9).
    Stop-Process -Id $script:AppProcess.Id -Force
    $script:AppProcess.WaitForExit(15000) | Out-Null
  }
  # The web view processes of the killed app go with it; give them a moment to release the port and the profile.
  Start-Sleep -Seconds 4
}

# Number of rows in a table of the database, read by another process (python is on the runner image).
function Get-Count([string]$table, [string]$db = (Join-Path $Data 'kinige.db')) {
  $code = "import sqlite3,sys,pathlib; c=sqlite3.connect(pathlib.Path(sys.argv[1]).as_uri()+'?mode=ro',uri=True); print(c.execute('select count(*) from ' + sys.argv[2]).fetchone()[0])"
  try { return ((& python -I -c $code $db $table 2>&1) | Out-String).Trim() }
  catch { return "n/a: $($_.Exception.Message)" }
}

# Types a note into the composer and presses "Send".
function Send-Note([string]$text) {
  if (-not (Cdp-Ok 'main' 'fill' @('New note', $text))) { return $false }
  Press 'main' 'Send'
  Start-Sleep -Seconds 2
  return $true
}

function Set-Dark {
  $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
  New-Item -Path $key -Force | Out-Null
  Set-ItemProperty -Path $key -Name AppsUseLightTheme -Value 0 -Type DWord
  Set-ItemProperty -Path $key -Name SystemUsesLightTheme -Value 0 -Type DWord
}

# ---- Setup ------------------------------------------------------------------------------------------------------------

[Win]::MakeDpiAware()
Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Debug, $Data, $Mirror, $Out | Out-Null
Get-ChildItem $Out -Include *.png, *.txt -Recurse | Remove-Item -Force

Set-Dark
Say ("display: " + [Win]::SetBestResolution())
Start-Sleep -Seconds 3

# Synthetic data.
Push-Location $Repo
# Cargo writes progress to stderr, which Windows PowerShell 5.1 turns into errors under 'Stop'.
$ErrorActionPreference = 'Continue'
try { & cargo run --release -q -p kinige-store --example seed_demo -- $Data $Mirror 2>&1 | Out-Host; if ($LASTEXITCODE -ne 0) { throw "seed_demo failed" } }
finally { Pop-Location; $ErrorActionPreference = 'Stop' }

# Install exactly what a user would get: silent NSIS install into a temporary directory.
$installerFile = Get-Item -LiteralPath $Installer
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Installer).Hash.ToLower()
# WaitForExit, not Start-Process -Wait: -Wait would also wait for processes the installer starts.
$install = Start-Process -FilePath $Installer -ArgumentList '/S', "/D=$AppDir" -PassThru
$install.WaitForExit(600000) | Out-Null
if ($install.ExitCode -ne 0) { Fail "the installer exited with $($install.ExitCode)" }
$exe = Get-ChildItem -Path $AppDir -Filter *.exe -File | Where-Object { $_.Name -notmatch '^uninstall' } | Select-Object -First 1
if (-not $exe) { Fail "no application exe in the install directory"; Get-ChildItem $AppDir -Recurse | Out-String | Write-Host; exit 1 }
$version = $exe.VersionInfo.ProductVersion
@(
  "installer: $($installerFile.Name)",
  "installer size: $($installerFile.Length) bytes ($([math]::Round($installerFile.Length / 1MB, 2)) MiB)",
  "installer sha256: $hash",
  "installed exe: $($exe.Name)",
  "installed exe ProductVersion: $version",
  "installed files: " + ((Get-ChildItem $AppDir -Recurse -File | ForEach-Object { $_.FullName.Substring($AppDir.Length + 1) }) -join ', '),
  "installs per-user without administrator rights: $((Test-Path (Join-Path $AppDir 'uninstall.exe')))"
) | Set-Content -Path (Join-Path $Out 'install.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'install.txt') -Raw)
if (Test-Path (Join-Path $AppDir 'kinige-mcp.exe')) { Fail 'the MCP bridge is part of the Windows install' }

# ---- 1. Main window ---------------------------------------------------------------------------------------------------

$main = Start-App $exe.FullName $true
if (-not $main) { exit 1 }
if (-not (Wait-For { Test-Has 'main' 'On this day' } 30 'the "On this day" card')) { Fail 'the "On this day" card is not in the Inbox' }
Shoot '01-inbox' 'main'

Press 'main' $WorkChat
Shoot '02-chat' 'main'

# ---- 2. Settings: the AI agent access page says it is not available ------------------------------------------------------

Press 'main' 'Profile'
Press 'main' 'Settings'
$settings = Wait-For { Get-AppWindow 'Settings' } 30 'the settings window'
if ($settings) {
  Wait-For { (Invoke-Cdp 'settings' 'ready').Code -eq 0 } 30 'the settings page over DevTools' | Out-Null
  Shoot '03-settings-appearance' 'settings'
  Press 'settings' 'AI agent access'
  if (-not (Test-Has 'settings' 'Not available on Windows yet.')) { Fail 'the access page does not say "Not available on Windows yet."'; Dump-Failure 'access' 'settings' }
  if (Test-Has 'settings' 'Allow agents to access the journal') { Fail 'the access switch is on the page' }
  Shoot '04-settings-access-unavailable' 'settings'

  # ---- 3. Large text -----------------------------------------------------------------------------------------------
  Press 'settings' 'Appearance'
  Press 'settings' 'Large'
  Start-Sleep -Seconds 2
  Shoot '05-large-text-settings' 'settings'
  Shoot '06-large-text-inbox' 'main'
  Press 'settings' 'Normal'
} else {
  Dump-Failure 'settings' 'main'
}

# ---- 4. Kill the process, start again: the entries are in place ---------------------------------------------------------

$before = Get-Count 'message'
$typed = Send-Note 'A synthetic note written just before the process is killed'
$written = Get-Count 'message'
if ($typed -and $written -eq $before) { Fail "the note typed into the composer was not saved ($before entries before, $written after)" }
Stop-App
$main = Start-App $exe.FullName $true
$after = Get-Count 'message'
@(
  "entries before the new note: $before",
  "a note was typed into the composer and sent: $typed",
  "entries before the kill: $written",
  "entries after the kill and the restart: $after"
) | Set-Content -Path (Join-Path $Out 'survival.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'survival.txt') -Raw)
if ($after -ne $written) { Fail "entries before the kill ($written) and after ($after) differ" }
if ($main) { Shoot '07-after-kill' 'main' }
Stop-App

# ---- 5. Default locations (no overrides) ----------------------------------------------------------------------------------

$main = Start-App $exe.FullName $false
$appData = Join-Path $env:APPDATA 'me.savvinov.kinige'
$documents = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Kinige'
# The Markdown folder appears with the first saved entry, so write one into this fresh default profile.
$sent = $false
if ($main) { $sent = Send-Note 'A synthetic note for the default location check' }
if ($sent) { Wait-For { Test-Path $documents } 30 'the Markdown copy folder Documents\Kinige' | Out-Null }
@(
  '# Names only, no contents. Data directory: %APPDATA%\me.savvinov.kinige',
  ((Get-ChildItem $appData -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', '),
  '# Markdown copy: Documents\Kinige (created with the first saved entry)',
  "exists: $(Test-Path $documents)",
  ((Get-ChildItem $documents -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', ')
) | Set-Content -Path (Join-Path $Out 'data-location.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'data-location.txt') -Raw)
if (-not (Test-Path (Join-Path $appData 'kinige.db'))) { Fail 'kinige.db is not in %APPDATA%\me.savvinov.kinige' }
if ($sent -and -not (Test-Path $documents)) { Fail 'the Markdown copy folder Documents\Kinige does not exist after an entry was saved' }
Stop-App

Say "scenario errors: $script:Fails"
if ($script:Fails -ne 0) { exit 1 }
