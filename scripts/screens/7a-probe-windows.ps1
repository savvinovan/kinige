# The microphone probe of slice 7a on a GitHub Windows runner:
#   powershell -NoProfile -File scripts/screens/7a-probe-windows.ps1 -Installer <Kinige-setup.exe>
# Windows PowerShell 5.1. Run with `gh workflow run screens.yml --ref <branch> -f slice=7a-probe-windows -f os=windows`; the result goes
# to docs/screens/7a-probe-windows/. The installer is built with KINIGE_FAKE_MIC=1 and KINIGE_PROBE=1 (screens.yml). At load, and again
# on the first trusted key, the page records five seconds from the fake microphone with the real MediaRecorder (WebView2,
# http://tauri.localhost) and writes the facts as an entry in the Inbox of the synthetic profile (scripts/screens/probe.ts); this
# script reads those entries from the database into formats.txt. It never touches the microphone, and no device label is collected.
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [string]$Out = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'docs\screens\7a-probe-windows'),
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

. (Join-Path $PSScriptRoot 'lib-windows.ps1')

# All probe entries, oldest first, written as UTF-8 text by python (the database is read by another process).
function Read-ProbeEntries([string]$target) {
  # No double quotes inside the arguments: Windows PowerShell 5.1 mangles them when it starts a native program.
  $code = "import sqlite3,sys,pathlib; c=sqlite3.connect(pathlib.Path(sys.argv[1]).as_uri()+'?mode=ro',uri=True); rows=c.execute(sys.argv[3]).fetchall(); open(sys.argv[2],'w',encoding='utf-8').write(chr(10).join(r[0]+chr(10)+'----' for r in rows)+chr(10))"
  $sql = "select body_md from message where body_md like 'KINIGE PROBE 7a%' order by created_at"
  & python -I -c $code (Join-Path $Data 'kinige.db') $target $sql 2>&1 | Out-Host
}

[Win]::MakeDpiAware()
if ($Work -match '^[A-Za-z]:\\Users\\') { Fail "the work directory $Work is inside a user profile"; exit 1 }
Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Debug, $Data, $Mirror, $Out | Out-Null
Get-ChildItem $Out -Include *.png, *.txt -Recurse | Remove-Item -Force

Set-Dark
Say ("display: " + [Win]::SetBestResolution())
Start-Sleep -Seconds 3

# Install exactly what a user would get: silent NSIS install into a temporary directory.
$install = Start-Process -FilePath $Installer -ArgumentList '/S', "/D=$AppDir" -PassThru
$install.WaitForExit(600000) | Out-Null
if ($install.ExitCode -ne 0) { Fail "the installer exited with $($install.ExitCode)" }
$exe = Get-ChildItem -Path $AppDir -Filter *.exe -File | Where-Object { $_.Name -notmatch '^uninstall' } | Select-Object -First 1
if (-not $exe) { Fail "no application exe in the install directory"; exit 1 }

$main = Start-App $exe.FullName
if (-not $main) { exit 1 }
# The first run (no gesture) records five seconds and decodes them.
Start-Sleep -Seconds 25
$first = Join-Path $Debug 'first.txt'
Read-ProbeEntries $first

# One trusted key for the second run.
Send-Keys 'Kinige' '{TAB}' $true
Start-Sleep -Seconds 25
Shoot '01-window' 'Kinige'

$target = Join-Path $Out 'formats.txt'
@(
  '# Facts about audio recording and playback in the webview of the built app (slice 7a probe, Windows runner).',
  "# windows: $([Environment]::OSVersion.VersionString)",
  ''
) | Set-Content -Path $target -Encoding UTF8
$all = Join-Path $Debug 'all.txt'
Read-ProbeEntries $all
Get-Content -Path $all -Encoding UTF8 | Add-Content -Path $target -Encoding UTF8
Say (Get-Content $target -Raw)

$entries = @(Select-String -Path $target -Pattern '^KINIGE PROBE 7a').Count
if ((Test-Path $first) -and (Get-Content $first -Raw) -notmatch 'navigator.mediaDevices') { Fail 'the first probe run left no report' }
if ($entries -lt 2) { Fail "the probe left $entries report(s), expected 2 (without and with a gesture)" }

Stop-App
Say "scenario errors: $script:Fails"
if ($script:Fails -ne 0) { exit 1 }
