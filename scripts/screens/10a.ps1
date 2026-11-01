# Screens of slice 10a (the Windows build) in the real window on a GitHub Windows runner:
#   powershell -NoProfile -File scripts/screens/10a.ps1 -Installer <Kinige-setup.exe>
# Windows PowerShell 5.1 on purpose (the runner has it): the UI Automation client assemblies exist there and not in pwsh 7.
#
# Data are synthetic (`seed_demo`) in a temporary directory; the real profile is not touched, except in the last phase, which
# starts the app once WITHOUT the data overrides on the fresh runner to prove the documented default locations.
# Input is UI Automation patterns only (Invoke, Toggle, SelectionItem, Value): no mouse, no keys. Each screenshot is the
# rectangle of one Kinige window; before it, the text of that window is checked for personal paths and names (see Test-Private).
# The result goes to -Out (docs/screens/10a); debug dumps of the UI tree go to $Work\debug and never into the repository.
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

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing, System.Windows.Forms

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
$PatternInvoke = [System.Windows.Automation.InvokePattern]::Pattern
$PatternToggle = [System.Windows.Automation.TogglePattern]::Pattern
$PatternSelect = [System.Windows.Automation.SelectionItemPattern]::Pattern
$PatternValue = [System.Windows.Automation.ValuePattern]::Pattern
$PatternExpand = [System.Windows.Automation.ExpandCollapsePattern]::Pattern

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

# Top-level windows of the app process that have a title (the helper windows of the event loop have none).
function Get-AppWindows {
  if (-not $script:AppProcess) { return @() }
  $condition = New-Object System.Windows.Automation.PropertyCondition($AE::ProcessIdProperty, $script:AppProcess.Id)
  $found = $AE::RootElement.FindAll($Scope::Children, $condition)
  $result = @()
  foreach ($w in $found) { if ($w.Current.Name) { $result += $w } }
  return $result
}

function Get-AppWindow([string]$title) {
  foreach ($w in (Get-AppWindows)) { if ($w.Current.Name -eq $title) { return $w } }
  return $null
}

# All descendants with their names and control types (cached: one round trip, not one per property).
function Get-Elements($root) {
  $cache = New-Object System.Windows.Automation.CacheRequest
  $cache.Add($AE::NameProperty)
  $cache.Add($AE::ControlTypeProperty)
  $cache.TreeScope = $Scope::Element
  # The first queries to a freshly started web view can time out (the page is still building its accessibility tree): retry.
  for ($attempt = 1; $attempt -le 2; $attempt++) {
    $activation = $cache.Activate()
    try { return $root.FindAll($Scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition) }
    catch { Say "UI Automation query failed (attempt $attempt): $($_.Exception.Message)"; Start-Sleep -Seconds 3 }
    finally { $activation.Dispose() }
  }
  return @()
}

# One line per element: depth is not available through FindAll, so the dump is flat: "ControlType | name".
function Dump-Tree($root, [string]$file) {
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $file) | Out-Null
  $lines = @()
  foreach ($e in (Get-Elements $root)) {
    $type = $e.Cached.ControlType.ProgrammaticName -replace '^ControlType\.', ''
    $lines += ("{0} | {1}" -f $type, $e.Cached.Name)
  }
  Set-Content -Path $file -Value $lines -Encoding UTF8
  return $lines
}

function Find-Elements($root, [string]$name) {
  $all = Get-Elements $root
  $exact = @(); $part = @()
  foreach ($e in $all) {
    $n = $e.Cached.Name
    if (-not $n) { continue }
    if ($n -eq $name) { $exact += $e } elseif ($n.Contains($name)) { $part += $e }
  }
  return $exact + $part
}

function Test-Has($root, [string]$text) { return @(Find-Elements $root $text).Count -gt 0 }

# Uses one pattern a web control can offer. Returns the pattern used or $null.
function Invoke-Element($element, [string]$kind) {
  $pattern = $null
  switch ($kind) {
    'invoke' { if ($element.TryGetCurrentPattern($PatternInvoke, [ref]$pattern)) { $pattern.Invoke(); return 'invoke' } }
    'toggle' { if ($element.TryGetCurrentPattern($PatternToggle, [ref]$pattern)) { $pattern.Toggle(); return 'toggle' } }
    'select' { if ($element.TryGetCurrentPattern($PatternSelect, [ref]$pattern)) { $pattern.Select(); return 'select' } }
    'expand' { if ($element.TryGetCurrentPattern($PatternExpand, [ref]$pattern)) { $pattern.Expand(); return 'expand' } }
  }
  return $null
}

# Presses the first element whose name is (or contains) `name` and supports a pattern. Waits for it to appear.
function Press([string]$windowTitle, [string]$name) {
  Focus-Window $windowTitle
  $done = Wait-For {
    $window = Get-AppWindow $windowTitle
    if (-not $window) { return $false }
    # The most direct pattern over all matches first: a list item that merely wraps the button can expose "expand" too.
    $candidates = @(Find-Elements $window $name)
    foreach ($kind in 'invoke', 'toggle', 'select', 'expand') {
      foreach ($candidate in $candidates) {
        try {
          $used = Invoke-Element $candidate $kind
          if ($used) { Say "press '$name' in '$windowTitle': $used"; return $true }
        } catch { }
      }
    }
    return $false
  } 12 "press '$name' in '$windowTitle'"
  if (-not $done) { Dump-Failure "press-$name" $windowTitle }
  Start-Sleep -Milliseconds 1500
}

function Dump-Failure([string]$label, [string]$windowTitle) {
  $safe = $label -replace '[^A-Za-z0-9-]', '_'
  $window = Get-AppWindow $windowTitle
  if ($window) { Dump-Tree $window (Join-Path $Debug "fail-$safe.txt") | Out-Null }
  else { Set-Content -Path (Join-Path $Debug "fail-$safe.txt") -Value "no window '$windowTitle'; windows: $((Get-AppWindows | ForEach-Object { $_.Current.Name }) -join ', ')" }
}

# Personal data must not reach a screenshot: no profile paths, no real names (the identifier me.savvinov.kinige is fine).
function Test-Private([string[]]$lines, [string]$label) {
  $hits = @($lines | Where-Object { $_ -match 'C:\\Users\\|runneradmin|savvinovan|alexandr' })
  if ($hits.Count -gt 0) { Fail "personal paths or names in the text of $label : $($hits[0])" }
}

function Focus-Window([string]$title) {
  $window = Get-AppWindow $title
  if ($window) { [Win]::Raise((New-Object System.IntPtr($window.Current.NativeWindowHandle))) }
}

# The accessibility tree of a web window fills in a moment after the window appears: wait until it has real content.
function Shoot([string]$name, [string]$title, [int]$minLines = 30) {
  $window = Get-AppWindow $title
  if (-not $window) { Fail "no window '$title' for $name"; return }
  $handle = New-Object System.IntPtr($window.Current.NativeWindowHandle)
  [Win]::Raise($handle)
  $lines = @()
  $deadline = (Get-Date).AddSeconds(25)
  do {
    Start-Sleep -Seconds 1
    $window = Get-AppWindow $title
    $lines = Dump-Tree $window (Join-Path $Debug "shot-$name.txt")
  } while ($lines.Count -lt $minLines -and (Get-Date) -lt $deadline)
  $copy = Join-Path $Out "$name.png"
  Say ("shot ${name}: " + [Win]::Capture($handle, $copy))
  if ($lines.Count -lt $minLines) {
    # The picture is kept for diagnosis, but a screenshot whose text could not be checked must not be accepted.
    Fail "the UI tree of $name has $($lines.Count) lines (< $minLines), so the private-path check could not run"
    return
  }
  Test-Private $lines $name
}

function Start-App([string]$exe, [bool]$overrides) {
  # Chromium builds its accessibility tree lazily, on the first UI Automation query, and a query that arrives while it is
  # being built can hang for minutes. With this flag the tree is built from the start of the page.
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--force-renderer-accessibility'
  if ($overrides) {
    $env:KINIGE_DATA_DIR = $Data
    $env:KINIGE_MIRROR_DIR = $Mirror
  } else {
    Remove-Item Env:KINIGE_DATA_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:KINIGE_MIRROR_DIR -ErrorAction SilentlyContinue
  }
  $script:AppProcess = Start-Process -FilePath $exe -PassThru
  $window = Wait-For { Get-AppWindow 'Kinige' } 90 'the main window'
  Start-Sleep -Seconds 4
  return $window
}

function Stop-App {
  if ($script:AppProcess -and -not $script:AppProcess.HasExited) {
    # TerminateProcess: the hardest kill there is on Windows (the equivalent of kill -9).
    Stop-Process -Id $script:AppProcess.Id -Force
    $script:AppProcess.WaitForExit(15000) | Out-Null
  }
  Start-Sleep -Seconds 2
}

# Number of rows in a table of the database, read by another process (python is on the runner image).
function Get-Count([string]$table, [string]$db = (Join-Path $Data 'kinige.db')) {
  $code = "import sqlite3,sys,pathlib; c=sqlite3.connect(pathlib.Path(sys.argv[1]).as_uri()+'?mode=ro',uri=True); print(c.execute('select count(*) from ' + sys.argv[2]).fetchone()[0])"
  try { return ((& python -I -c $code $db $table 2>&1) | Out-String).Trim() }
  catch { return "n/a: $($_.Exception.Message)" }
}

# Types a note with real key events into the composer (the runner has an interactive desktop and nobody else on it) and
# presses "Send" through UI Automation. Value patterns do not work here: the page does not see a value set from outside.
function Send-Note([string]$text) {
  Focus-Window 'Kinige'
  $composer = Wait-For {
    $found = Find-Elements (Get-AppWindow 'Kinige') 'New note' | Where-Object { $_.Current.ControlType.ProgrammaticName -match 'Edit' }
    $found | Select-Object -First 1
  } 20 'the composer'
  if (-not $composer) { return $false }
  $composer.SetFocus()
  Start-Sleep -Milliseconds 800
  $window = Get-AppWindow 'Kinige'
  Say ("composer: foreground=" + [Win]::IsForeground((New-Object System.IntPtr($window.Current.NativeWindowHandle))) + ' focused=' + $composer.Current.HasKeyboardFocus)
  [System.Windows.Forms.SendKeys]::SendWait($text)
  Start-Sleep -Milliseconds 800
  $value = $null
  if ($composer.TryGetCurrentPattern($PatternValue, [ref]$value)) { Say ("composer value length after typing: " + $value.Current.Value.Length) }
  Press 'Kinige' 'Send'
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

# Synthetic data (the example refuses nothing and writes only where it is told).
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
$handle = New-Object System.IntPtr($main.Current.NativeWindowHandle)
[Win]::MoveTo($handle, 20, 20, 1280, 800)
Start-Sleep -Seconds 2
Dump-Tree $main (Join-Path $Debug 'main-start.txt') | Out-Null
if (-not (Wait-For { Test-Has (Get-AppWindow 'Kinige') 'On this day' } 30 'the "On this day" card')) { Fail 'the "On this day" card is not in the Inbox' }
Shoot '01-inbox' 'Kinige'

Press 'Kinige' $WorkChat
Shoot '02-chat' 'Kinige'

# ---- 2. Settings: the AI agent access page says it is not available ------------------------------------------------------

Press 'Kinige' 'Profile'
Press 'Kinige' 'Settings'
$settings = Wait-For { Get-AppWindow 'Settings' } 30 'the settings window'
if ($settings) {
  Start-Sleep -Seconds 2
  Shoot '03-settings-appearance' 'Settings'
  Press 'Settings' 'AI agent access'
  $page = Get-AppWindow 'Settings'
  if (-not (Test-Has $page 'Not available on Windows yet.')) { Fail 'the access page does not say "Not available on Windows yet."'; Dump-Failure 'access' 'Settings' }
  if (Test-Has $page 'Allow agents to access the journal') { Fail 'the access switch is on the page' }
  Shoot '04-settings-access-unavailable' 'Settings'

  # ---- 3. Large text -----------------------------------------------------------------------------------------------
  Press 'Settings' 'Appearance'
  Press 'Settings' 'Large'
  Start-Sleep -Seconds 2
  Shoot '05-large-text-settings' 'Settings'
  Shoot '06-large-text-inbox' 'Kinige'
  Press 'Settings' 'Normal'
} else {
  Dump-Failure 'settings' 'Kinige'
}

# ---- 4. Kill the process, start again: the entries are in place ---------------------------------------------------------

$before = Get-Count 'message'
$typed = Send-Note 'A synthetic note written just before the process is killed'
$written = Get-Count 'message'
if ($typed -and $written -eq $before) { Fail "the note typed into the composer was not saved ($before entries before, $written after)" }
Stop-App
$main = Start-App $exe.FullName $true
if ($main) { [Win]::MoveTo((New-Object System.IntPtr($main.Current.NativeWindowHandle)), 20, 20, 1280, 800) }
$after = Get-Count 'message'
@(
  "entries before the new note: $before",
  "a note was typed into the composer and sent: $typed",
  "entries before the kill: $written",
  "entries after the kill and the restart: $after"
) | Set-Content -Path (Join-Path $Out 'survival.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'survival.txt') -Raw)
if ($after -ne $written) { Fail "entries before the kill ($written) and after ($after) differ" }
if ($main) { Shoot '07-after-kill' 'Kinige' 100 }
Stop-App

# ---- 5. Default locations (no overrides) ----------------------------------------------------------------------------------

$main = Start-App $exe.FullName $false
# The Markdown folder appears with the first saved entry, so write one into this fresh default profile.
$sent = Send-Note 'A synthetic note for the default location check'
$appData = Join-Path $env:APPDATA 'me.savvinov.kinige'
$documents = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Kinige'
Start-Sleep -Seconds 3
@(
  '# Names only, no contents. Data directory: %APPDATA%\me.savvinov.kinige',
  ((Get-ChildItem $appData -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', '),
  '# Markdown copy: Documents\Kinige',
  "exists: $(Test-Path $documents)",
  ((Get-ChildItem $documents -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', ')
) | Set-Content -Path (Join-Path $Out 'data-location.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'data-location.txt') -Raw)
if (-not (Test-Path (Join-Path $appData 'kinige.db'))) { Fail 'kinige.db is not in %APPDATA%\me.savvinov.kinige' }
if ($sent -and -not (Test-Path $documents)) { Fail 'the Markdown copy folder Documents\Kinige does not exist after an entry was saved' }
Stop-App

Say "scenario errors: $script:Fails"
if ($script:Fails -ne 0) { exit 1 }
