# Screens of slice 7a (voice notes) in the real window on a GitHub Windows runner:
#   powershell -NoProfile -File scripts/screens/7a-windows.ps1 -Installer <Kinige-setup.exe>
# Windows PowerShell 5.1. Run with `gh workflow run screens.yml --ref <branch> -f slice=7a-windows -f os=windows`; the result goes to
# docs/screens/7a-windows/. The installer is built with KINIGE_FAKE_MIC=1 (screens.yml): an oscillator replaces getUserMedia, so the
# real MediaRecorder of WebView2 records with no device and no privacy prompt. What it checks that the macOS run cannot: recording and
# the `http://kinige-media.localhost` audio route (206 ranges) on WebView2. Keys are SendKeys after SetForegroundWindow; there is no UI
# Automation and no DevTools port (they stalled WebView2 on the runner). That the player played is read from the app log (the route
# logs its status and size at debug level, no ids), not from the window.
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [string]$Out = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'docs\screens\7a-windows'),
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

function Get-AppLogLines {
  $logs = @(Get-ChildItem -Path (Join-Path $env:LOCALAPPDATA 'me.savvinov.kinige'), (Join-Path $env:APPDATA 'me.savvinov.kinige') -Recurse -Filter *.log -ErrorAction SilentlyContinue)
  return @($logs | ForEach-Object { Get-Content -LiteralPath $_.FullName -Encoding UTF8 -ErrorAction SilentlyContinue })
}

[Win]::MakeDpiAware()
if ($Work -match '^[A-Za-z]:\\Users\\') { Fail "the work directory $Work is inside a user profile"; exit 1 }
Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Debug, $Data, $Mirror, $Out | Out-Null
Get-ChildItem $Out -Include *.png, *.txt -Recurse | Remove-Item -Force

Set-Dark
Say ("display: " + [Win]::SetBestResolution())
Start-Sleep -Seconds 3

# Synthetic data, without voice notes: the only voice note of this run is the one the scenario records, so a request of the audio route can
# only be that note's (the first run also seeded a Telegram-style stub, and Play landed on it).
Push-Location $Repo
$ErrorActionPreference = 'Continue'
Remove-Item Env:KINIGE_SEED_VOICE -ErrorAction SilentlyContinue
try { & cargo run --release -q -p kinige-store --example seed_demo -- $Data $Mirror 2>&1 | Out-Host; if ($LASTEXITCODE -ne 0) { throw "seed_demo failed" } }
finally { Pop-Location; $ErrorActionPreference = 'Stop' }

# Positive control of the fake microphone guard.
& bash (Join-Path $Repo 'scripts/check-fake-mic.sh') (Join-Path $Repo 'web/dist') --expect
if ($LASTEXITCODE -ne 0) { Fail 'the fake microphone is not in web/dist' }

$install = Start-Process -FilePath $Installer -ArgumentList '/S', "/D=$AppDir" -PassThru
$install.WaitForExit(600000) | Out-Null
if ($install.ExitCode -ne 0) { Fail "the installer exited with $($install.ExitCode)" }
$exe = Get-ChildItem -Path $AppDir -Filter *.exe -File | Where-Object { $_.Name -notmatch '^uninstall' } | Select-Object -First 1
if (-not $exe) { Fail "no application exe in the install directory"; exit 1 }

# ---- 1. Recording: Tab from the field reaches the emoji button, then the microphone; Enter starts ---------------------------------

$main = Start-App $exe.FullName
if (-not $main) { exit 1 }
Start-Sleep -Seconds 6
Send-Keys 'Kinige' '{TAB}{TAB}{ENTER}' $true
Start-Sleep -Seconds 5
Shoot '01-recording' 'Kinige'

# ---- 2. Enter on Send (the focus moved there) saves the note ---------------------------------------------------------------------

Send-Keys 'Kinige' '{ENTER}' $false
Start-Sleep -Seconds 5
$rows = Get-Scalar "select count(*) from attachment where filename like 'voice-%'"
if ($rows -ne '1') { Fail "expected one recorded note after Enter on Send, found $rows" }
$mime = Get-Scalar "select mime from attachment where filename like 'voice-%' limit 1"
$duration = Get-Scalar "select duration_ms from attachment where filename like 'voice-%' limit 1"
@(
  "windows: $([Environment]::OSVersion.VersionString)",
  "recorded notes: $rows",
  "container: $mime",
  "duration_ms: $duration"
) | Set-Content -Path (Join-Path $Out 'files.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'files.txt') -Raw)
if ($mime -notmatch '^audio/(mp4|webm)$') { Fail "unexpected container: $mime" }
if ($duration -notmatch '^\d+$' -or [int64]$duration -lt 3000) { Fail "the saved duration looks wrong: $duration" }
Shoot '02-sent' 'Kinige'

# ---- 3. The note survives a kill, and Play reaches the audio route ------------------------------------------------------------------

Stop-App
$before = @(Get-AppLogLines | Where-Object { $_ -match 'audio route: ' }).Count
$main = Start-App $exe.FullName
if (-not $main) { exit 1 }
Start-Sleep -Seconds 6
$rowsAfter = Get-Scalar "select count(*) from attachment where filename like 'voice-%'"
if ($rowsAfter -ne '1') { Fail "the note did not survive the kill ($rowsAfter)" }
# Shift+Tab from the field walks back through the newest entry: its five action buttons (copy, edit, task, favorite, more), the speed
# chip, the seek bar and Play, so Play is the eighth stop (the first run, with six, ended on the Trash button of the sidebar). The
# neighbouring counts are tried after it; each attempt returns to the field with the same number of Tab presses and checks the log for a
# request of the audio route.
$played = $false
foreach ($stops in 8, 7, 9) {
  Focus-Window 'Kinige'
  Start-Sleep -Milliseconds 1500
  for ($i = 0; $i -lt $stops; $i++) { Send-Keys 'Kinige' '+{TAB}' $false }
  Send-Keys 'Kinige' '{ENTER}' $false
  Start-Sleep -Seconds 4
  $now = @(Get-AppLogLines | Where-Object { $_ -match 'audio route: ' })
  if ($now.Count -gt $before) { $played = $true; Say "Play reached the route with $stops Shift+Tab stops"; break }
  Send-Keys 'Kinige' '{ESC}{ESC}' $false
  for ($i = 0; $i -lt $stops; $i++) { Send-Keys 'Kinige' '{TAB}' $false }
}
Shoot '03-playing' 'Kinige'
$lines = @(Get-AppLogLines | Where-Object { $_ -match 'audio route: ' })
@("audio route requests in the log: $($lines.Count)") + @($lines | Select-Object -First 8 | ForEach-Object { ($_ -replace '^\[[^\]]*\]\[[^\]]*\]', '').Trim() }) |
  Set-Content -Path (Join-Path $Out 'playback.txt') -Encoding UTF8
Say (Get-Content (Join-Path $Out 'playback.txt') -Raw)
if (-not $played) { Fail 'Play did not reach the audio route (a finding if the focus order differs)' }
elseif (-not ($lines | Where-Object { $_ -match 'audio route: (200|206) ' })) { Fail 'the audio route answered with something other than 200 or 206' }
else {
  # The answer's total length is the recorded file's size: the route served the note this run recorded.
  $size = Get-Scalar "select byte_size from attachment where filename like 'voice-%' limit 1"
  if (-not ($lines | Where-Object { $_ -match "answered `"bytes \d+-\d+/$size`"" })) { Fail "no answer of the audio route had the recorded file's size ($size bytes)" }
  else { Add-Content -Path (Join-Path $Out 'playback.txt') -Value "the route served the recorded note: its size is $size bytes" -Encoding UTF8 }
}

# ---- Formats from the probe, and the log holds no file names or device names ---------------------------------------------------------

$probe = Join-Path $Repo 'docs\screens\7a-probe-windows\formats.txt'
if (Test-Path $probe) { Copy-Item $probe (Join-Path $Out 'formats.txt') } else { 'no probe output was committed' | Set-Content (Join-Path $Out 'formats.txt') }
Stop-App
$private = @(Get-AppLogLines | Where-Object { $_ -match 'voice-[0-9]|audio_1|seed-voice|AirPods|Microphone \(' })
if ($private.Count -gt 0) { Fail "the app log holds file names or device names ($($private.Count) lines)" }

Say "scenario errors: $script:Fails"
if ($script:Fails -ne 0) { exit 1 }
