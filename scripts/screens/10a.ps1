# Screens of slice 10a (the Windows build) in the real window on a GitHub Windows runner:
#   powershell -NoProfile -File scripts/screens/10a.ps1 -Installer <Kinige-setup.exe>
# Windows PowerShell 5.1 (the runner has it).
#
# Input is keyboard keys injected on the runner (SendKeys after the Kinige window is brought to the foreground): the runner is
# a disposable VM with nobody on it. No UI Automation and no DevTools: UI Automation stalled the WebView2 tree, and the
# DevTools port could not be used reliably (ADR-0012 and the slice spec, "Follow-ups"). Windows are found with plain Win32.
# Each screenshot is the rectangle of one Kinige window (the real title bar included), taken with CopyFromScreen.
#
# Data are synthetic (`seed_demo`) in a temporary directory; the real profile is not touched, except in the last phase, which
# starts the app once WITHOUT the data overrides on the fresh runner to prove the documented default locations.
# The data checks do not use the UI: the database is read by another process, the Markdown copy is read from disk.
# The personal-path check of the macOS scenarios is replaced here by a check that the scenario works only in the runner's
# temporary directory (no user profile path) on synthetic data.
# The result goes to -Out (docs/screens/10a).
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
$script:AppProcess = $null

Add-Type -AssemblyName System.Drawing, System.Windows.Forms

# C# 5 only (Windows PowerShell compiles with the .NET Framework compiler).
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;

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

  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder text, int max);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);

  // The visible top-level window of a process with this exact title, or IntPtr.Zero. Plain Win32: no accessibility
  // client touches the web view (a UI Automation client makes WebView2 build its accessibility tree, which stalled it).
  public static IntPtr FindTitled(uint pid, string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows(delegate(IntPtr h, IntPtr l) {
      uint owner;
      GetWindowThreadProcessId(h, out owner);
      if (owner != pid || !IsWindowVisible(h)) return true;
      StringBuilder text = new StringBuilder(256);
      GetWindowText(h, text, 256);
      if (text.ToString() == title) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }

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

# The handle of a top-level window of the app process by its title (plain Win32), or $null.
function Get-AppWindow([string]$title) {
  if (-not $script:AppProcess) { return $null }
  $handle = [Win]::FindTitled([uint32]$script:AppProcess.Id, $title)
  if ($handle -eq [System.IntPtr]::Zero) { return $null }
  return $handle
}

function Focus-Window([string]$title) {
  $window = Get-AppWindow $title
  if ($window) { [Win]::Raise($window); Start-Sleep -Milliseconds 500 }
}

# Keys go to the foreground window: bring the Kinige window up first. `$keys` is SendKeys syntax.
function Send-Keys([string]$title, [string]$keys) {
  Focus-Window $title
  [System.Windows.Forms.SendKeys]::SendWait($keys)
  Start-Sleep -Milliseconds 400
}

function Press-Tab([string]$title, [int]$times) {
  for ($i = 0; $i -lt $times; $i++) { Send-Keys $title '{TAB}' }
}

function Shoot([string]$name, [string]$title) {
  $window = Get-AppWindow $title
  if (-not $window) { Fail "no window '$title' for $name"; return }
  [Win]::Raise($window)
  Start-Sleep -Seconds 2
  Say ("shot ${name}: " + [Win]::Capture($window, (Join-Path $Out "$name.png")))
}

function Start-App([string]$exe, [bool]$overrides) {
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
    [Win]::MoveTo($window, 20, 20, 1280, 800)
    # There is no signal that the page has finished loading without touching it: wait for the first paint generously.
    Start-Sleep -Seconds 12
    Focus-Window 'Kinige'
  }
  return $window
}

function Stop-App {
  if ($script:AppProcess -and -not $script:AppProcess.HasExited) {
    # TerminateProcess: the hardest kill there is on Windows (the equivalent of kill -9).
    Stop-Process -Id $script:AppProcess.Id -Force
    $script:AppProcess.WaitForExit(15000) | Out-Null
  }
  Start-Sleep -Seconds 4
}

# A scalar from the database, read by another process (python is on the runner image).
function Get-Scalar([string]$sql, [string]$db = (Join-Path $Data 'kinige.db')) {
  $code = "import sqlite3,sys,pathlib; c=sqlite3.connect(pathlib.Path(sys.argv[1]).as_uri()+'?mode=ro',uri=True); print(c.execute(sys.argv[2]).fetchone()[0])"
  try { return ((& python -I -c $code $db $sql 2>&1) | Out-String).Trim() }
  catch { return "n/a: $($_.Exception.Message)" }
}

# Types a note into the composer (it has the focus when the window opens) and sends it with Enter.
function Send-Note([string]$text) {
  # The first keys sent right after the window is raised were lost ("A " of the first note): wait for the focus to settle.
  Focus-Window 'Kinige'
  Start-Sleep -Seconds 2
  Send-Keys 'Kinige' ($text + '{ENTER}')
  Start-Sleep -Seconds 3
}

# Opens the profile menu and "Settings" by keys: Ctrl+K focuses the search field (the first stop of the sidebar), then Tab
# walks Collapse, the chats of the synthetic data, Archive, Trash and Profile. `$stops` is the number of Tab presses; the
# count depends on the sidebar of the synthetic data, so a few neighbours are tried when the window does not appear.
function Open-Settings {
  foreach ($stops in 11, 12, 10, 13, 9, 14, 8, 15, 16) {
    Send-Keys 'Kinige' '{ESC}'
    Send-Keys 'Kinige' '^k'
    Press-Tab 'Kinige' $stops
    # Where the focus is after the Tab stops (kept for diagnosis, never committed).
    [Win]::Capture((Get-AppWindow 'Kinige'), (Join-Path $Debug "tabs-$stops.png")) | Out-Null
    Send-Keys 'Kinige' '{ENTER}'
    Start-Sleep -Milliseconds 800
    Send-Keys 'Kinige' '{ENTER}'
    $window = $null
    for ($i = 0; $i -lt 10 -and -not $window; $i++) { Start-Sleep -Milliseconds 700; $window = Get-AppWindow 'Settings' }
    if ($window) { Say "settings opened with $stops Tab stops"; return $window }
    Say "no settings window after $stops Tab stops"
  }
  Fail 'the settings window did not open'
  return $null
}

function Set-Dark {
  $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
  New-Item -Path $key -Force | Out-Null
  Set-ItemProperty -Path $key -Name AppsUseLightTheme -Value 0 -Type DWord
  Set-ItemProperty -Path $key -Name SystemUsesLightTheme -Value 0 -Type DWord
}

# ---- Setup ------------------------------------------------------------------------------------------------------------

[Win]::MakeDpiAware()
# No user profile path anywhere: the scenario works only in the runner's temporary directory.
if ($Work -match '^[A-Za-z]:\\Users\\') { Fail "the work directory $Work is inside a user profile"; exit 1 }
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
$seeded = Get-Scalar 'select count(*) from message'
if ($seeded -notmatch '^\d+$' -or [int]$seeded -lt 20) { Fail "the seeded database has $seeded messages" }

# Install exactly what a user would get: silent NSIS install into a temporary directory.
$installerFile = Get-Item -LiteralPath $Installer
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Installer).Hash.ToLower()
# WaitForExit, not Start-Process -Wait: -Wait would also wait for processes the installer starts.
$install = Start-Process -FilePath $Installer -ArgumentList '/S', "/D=$AppDir" -PassThru
$install.WaitForExit(600000) | Out-Null
if ($install.ExitCode -ne 0) { Fail "the installer exited with $($install.ExitCode)" }
$exe = Get-ChildItem -Path $AppDir -Filter *.exe -File | Where-Object { $_.Name -notmatch '^uninstall' } | Select-Object -First 1
if (-not $exe) { Fail "no application exe in the install directory"; Get-ChildItem $AppDir -Recurse | Out-String | Write-Host; exit 1 }
@(
  "installer: $($installerFile.Name)",
  "installer size: $($installerFile.Length) bytes ($([math]::Round($installerFile.Length / 1MB, 2)) MiB)",
  "installer sha256: $hash",
  "installed exe: $($exe.Name)",
  "installed exe ProductVersion: $($exe.VersionInfo.ProductVersion)",
  "installed files: " + ((Get-ChildItem $AppDir -Recurse -File | ForEach-Object { $_.FullName.Substring($AppDir.Length + 1) }) -join ', '),
  "installs per-user without administrator rights: $((Test-Path (Join-Path $AppDir 'uninstall.exe')))",
  "seeded messages: $seeded"
) | Set-Content -Path (Join-Path $Out 'install.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'install.txt') -Raw)
if (Test-Path (Join-Path $AppDir 'kinige-mcp.exe')) { Fail 'the MCP bridge is part of the Windows install' }

# ---- 1. Main window with the seeded data -------------------------------------------------------------------------------

$main = Start-App $exe.FullName $true
if (-not $main) { exit 1 }
Shoot '01-inbox' 'Kinige'

# ---- 2. A note typed with keys and sent --------------------------------------------------------------------------------

$noteText = 'A synthetic note written just before the process is killed'
$before = Get-Scalar 'select count(*) from message'
Send-Note $noteText
$written = Get-Scalar 'select count(*) from message'
$noteSaved = ($before -match '^\d+$') -and ($written -match '^\d+$') -and ([int]$written -eq [int]$before + 1)
if (-not $noteSaved) { Fail "the typed note was not saved ($before entries before, $written after)" }
Shoot '02-note-sent' 'Kinige'

# ---- 3. Settings by keys: the AI agent access page says it is not available --------------------------------------------

$settings = Open-Settings
if ($settings) {
  Start-Sleep -Seconds 4
  Shoot '03-settings' 'Settings'
  # The tree is one tab stop (the open section); Down moves along Appearance, Keyboard shortcuts, Copies to AI agent access.
  Send-Keys 'Settings' '{TAB}'
  for ($i = 0; $i -lt 3; $i++) { Send-Keys 'Settings' '{DOWN}' }
  Send-Keys 'Settings' '{ENTER}'
  Start-Sleep -Seconds 2
  Shoot '04-settings-access-unavailable' 'Settings'
}

# ---- 4. Kill the process, start again: the entries are in place --------------------------------------------------------

Stop-App
$main = Start-App $exe.FullName $true
$after = Get-Scalar 'select count(*) from message'
@(
  "seeded messages: $seeded",
  "messages before the typed note: $before",
  "messages after it, before the kill: $written",
  "messages after the kill and the restart: $after"
) | Set-Content -Path (Join-Path $Out 'survival.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'survival.txt') -Raw)
if ($after -ne $written) { Fail "messages before the kill ($written) and after ($after) differ" }
if ($main) { Shoot '05-after-kill' 'Kinige' }

# ---- 5. The Markdown copy of the scenario (KINIGE_MIRROR_DIR) ------------------------------------------------------------

# The copy is written by a background writer; after a kill the pending days are written at the next start.
$withNote = @()
for ($i = 0; $i -lt 40 -and $withNote.Count -eq 0; $i++) {
  $mdFiles = @(Get-ChildItem -Path $Mirror -Recurse -Filter *.md -ErrorAction SilentlyContinue)
  $withNote = @($mdFiles | ForEach-Object { if ((Get-Content -LiteralPath $_.FullName -Raw -Encoding UTF8) -match 'note written just before the process is killed') { $_ } })
  if ($withNote.Count -eq 0) { Start-Sleep -Seconds 1 }
}
@(
  "markdown files in the copy folder: $($mdFiles.Count)",
  "files that contain the typed note: $($withNote.Count)",
  "days still waiting for the copy (mirror_dirty): " + (Get-Scalar 'select count(*) from mirror_dirty'),
  "folders: " + ((Get-ChildItem $Mirror -Directory -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', ')
) | Set-Content -Path (Join-Path $Out 'mirror.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'mirror.txt') -Raw)
if ($mdFiles.Count -eq 0) { Fail 'no Markdown files in KINIGE_MIRROR_DIR' }
if ($noteSaved -and $withNote.Count -eq 0) { Fail 'the typed note is not in the Markdown copy' }
Stop-App

# ---- 6. Default locations (no overrides) ----------------------------------------------------------------------------------

$main = Start-App $exe.FullName $false
$appData = Join-Path $env:APPDATA 'me.savvinov.kinige'
$documents = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Kinige'
# The Markdown folder appears with the first saved entry, so write one into this fresh default profile.
if ($main) { Send-Note 'A synthetic note for the default location check' }
Wait-For { Test-Path $documents } 30 'the Markdown copy folder Documents\Kinige' | Out-Null
@(
  '# Names only, no contents. Data directory: %APPDATA%\me.savvinov.kinige',
  ((Get-ChildItem $appData -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', '),
  '# Markdown copy: Documents\Kinige (created with the first saved entry; Documents is the shell known folder)',
  "exists: $(Test-Path $documents)",
  ((Get-ChildItem $documents -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', '),
  "default database rows: " + (Get-Scalar 'select count(*) from message' (Join-Path $appData 'kinige.db'))
) | Set-Content -Path (Join-Path $Out 'data-location.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'data-location.txt') -Raw)
if (-not (Test-Path (Join-Path $appData 'kinige.db'))) { Fail 'kinige.db is not in %APPDATA%\me.savvinov.kinige' }
if (-not (Test-Path $documents)) { Fail 'the Markdown copy folder Documents\Kinige does not exist after an entry was sent' }
Stop-App

Say "scenario errors: $script:Fails"
if ($script:Fails -ne 0) { exit 1 }
