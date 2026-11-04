# Shared helpers of the Windows screens scenarios of slice 7a (7a-probe-windows.ps1, 7a-windows.ps1): dot-sourced, not run. The
# code is the helper block of 7-windows.ps1 (plain Win32 window finding, SendKeys input, a screen copy of the window rectangle).
# The caller defines $Out, $Work, $Debug, $Data, $Mirror, $AppDir, $script:Fails and $script:AppProcess before the first call.
# Do not use UI Automation or a DevTools port here: both stalled the WebView2 page on the runner (slice 10a).

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
  [DllImport("user32.dll")] static extern bool AttachThreadInput(uint attach, uint to, bool on);
  [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
  [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
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
    // A background process may not take the foreground: attach to the input queue of the current foreground window for the
    // call. (An Alt tap, the other known trick, puts the window into menu mode and the first typed keys are lost.)
    uint ignored;
    uint foregroundThread = GetWindowThreadProcessId(GetForegroundWindow(), out ignored);
    uint thisThread = GetCurrentThreadId();
    AttachThreadInput(thisThread, foregroundThread, true);
    BringWindowToTop(h);
    SetForegroundWindow(h);
    AttachThreadInput(thisThread, foregroundThread, false);
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
    // One pixel in from every side: the outermost row and column of the frame bounds belong to whatever is behind the window.
    Rectangle full = Bounds(h);
    Rectangle b = new Rectangle(full.Left + 1, full.Top + 1, full.Width - 2, full.Height - 2);
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

function Shoot([string]$name, [string]$title) {
  $window = Get-AppWindow $title
  if (-not $window) { Fail "no window '$title' for $name"; return }
  [Win]::Raise($window)
  Start-Sleep -Seconds 2
  Say ("shot ${name}: " + [Win]::Capture($window, (Join-Path $Out "$name.png")))
}


function Start-App([string]$exe) {
  $env:KINIGE_DATA_DIR = $Data
  $env:KINIGE_MIRROR_DIR = $Mirror
  # Debug level: the app log then holds the line with the first weekday and the locale API that gave it.
  $env:KINIGE_LOG = 'debug'
  $script:AppProcess = Start-Process -FilePath $exe -PassThru
  $window = Wait-For { Get-AppWindow 'Kinige' } 90 'the main window'
  if ($window) {
    [Win]::MoveTo($window, 20, 20, 1280, 800)
    # There is no signal that the page has finished loading without touching it: wait for the first paint generously.
    Start-Sleep -Seconds 14
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

# Writes an `app_setting` row while the app is stopped (the view state of the forest).
function Set-Setting([string]$key, [string]$value) {
  $code = "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute('insert or replace into app_setting(key,value) values(?,?)',(sys.argv[2],sys.argv[3])); c.commit(); c.close()"
  & python -I -c $code (Join-Path $Data 'kinige.db') $key $value 2>&1 | Out-Host
  if ($LASTEXITCODE -ne 0) { Fail "could not write the setting $key" }
}

function Set-Dark {
  $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
  New-Item -Path $key -Force | Out-Null
  Set-ItemProperty -Path $key -Name AppsUseLightTheme -Value 0 -Type DWord
  Set-ItemProperty -Path $key -Name SystemUsesLightTheme -Value 0 -Type DWord
}



# Keys go to the foreground window: bring the Kinige window up first. `$keys` is SendKeys syntax.
function Send-Keys([string]$title, [string]$keys, [bool]$raise = $true) {
  if ($raise) { Focus-Window $title; Start-Sleep -Milliseconds 1500 }
  [System.Windows.Forms.SendKeys]::SendWait($keys)
  Start-Sleep -Milliseconds 400
}
