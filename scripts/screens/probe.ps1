param([Parameter(Mandatory = $true)][string]$Installer)
# TEMPORARY probe of UI Automation strategies; see .github/workflows/win-ui-probe.yml
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  public static void Raise(IntPtr h, bool alt) {
    ShowWindow(h, 9);
    SetWindowPos(h, new IntPtr(-1), 0, 0, 0, 0, 0x43); SetWindowPos(h, new IntPtr(-2), 0, 0, 0, 0, 0x43);
    if (alt) { keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero); }
    SetForegroundWindow(h);
  }
}
'@
$AE = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]
$out = Join-Path $env:RUNNER_TEMP 'probe'
New-Item -ItemType Directory -Force -Path $out, (Join-Path $env:RUNNER_TEMP 'pd'), (Join-Path $env:RUNNER_TEMP 'pm'), (Join-Path $env:RUNNER_TEMP 'app') | Out-Null
$log = Join-Path $out 'log.txt'
function L($t) { $line = (Get-Date).ToString('HH:mm:ss.fff') + ' ' + $t; Write-Host $line; Add-Content -Path $log -Value $line }

$app = Join-Path $env:RUNNER_TEMP 'app'
$p = Start-Process -FilePath $Installer -ArgumentList '/S', "/D=$app" -PassThru; $p.WaitForExit(600000) | Out-Null
$exe = (Get-ChildItem $app -Filter kinige.exe).FullName
& cargo --version | Out-Null
# empty data dir: the app creates its own database
$env:KINIGE_DATA_DIR = Join-Path $env:RUNNER_TEMP 'pd'
$env:KINIGE_MIRROR_DIR = Join-Path $env:RUNNER_TEMP 'pm'
$proc = Start-Process -FilePath $exe -PassThru
function Win { $c = New-Object System.Windows.Automation.PropertyCondition($AE::ProcessIdProperty, $proc.Id); foreach ($w in $AE::RootElement.FindAll($Scope::Children, $c)) { if ($w.Current.Name -eq 'Kinige') { return $w } } }
$w = $null
for ($i = 0; $i -lt 60 -and -not $w; $i++) { Start-Sleep -Seconds 1; $w = Win }
L "window: $($w -ne $null)"
$h = New-Object System.IntPtr($w.Current.NativeWindowHandle)

function Time($label, [scriptblock]$code) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  try { $r = & $code; L "$label : $r in $($sw.ElapsedMilliseconds) ms" } catch { L "$label : EXCEPTION $($_.Exception.Message) after $($sw.ElapsedMilliseconds) ms" }
}
function Walk($el, $depth, $max) {
  $n = 0
  $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
  $child = $walker.GetFirstChild($el)
  while ($child -ne $null -and $n -lt $max) {
    $n++
    if ($depth -lt 12) { $n += Walk $child ($depth + 1) ($max - $n) }
    $child = $walker.GetNextSibling($child)
  }
  return $n
}
for ($round = 1; $round -le 3; $round++) {
  L "---- round $round (before any raise: $($round -eq 1))"
  if ($round -eq 2) { [W]::Raise($h, $false); L 'raised (no alt)'; Start-Sleep -Seconds 3 }
  if ($round -eq 3) { [W]::Raise($h, $true); L 'raised (alt)'; Start-Sleep -Seconds 3 }
  Time 'children of window (Children)' { $w.FindAll($Scope::Children, [System.Windows.Automation.Condition]::TrueCondition).Count }
  Time 'FindAll Descendants True' { $w.FindAll($Scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition).Count }
  Time 'FindFirst Name=Search' { ($w.FindFirst($Scope::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, 'Search'))) -ne $null) }
  Time 'RawViewWalker walk (max 2000)' { Walk $w 0 2000 }
  Start-Sleep -Seconds 5
}
Stop-Process -Id $proc.Id -Force
