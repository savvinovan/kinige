# The screens driver probe on a GitHub Windows runner (KINIG-92, ADR-0016):
#   powershell -NoProfile -File scripts/screens/screens-driver-probe-windows.ps1 -Installer <Kinige-setup.exe>
# Windows PowerShell 5.1. Run with `gh workflow run screens.yml --ref <branch> -f slice=screens-driver-probe-windows -f os=windows`; the result
# goes to docs/screens/screens-driver-probe-windows/. The installer is built with the fake microphone and the screens driver (screens.yml).
# It answers on WebView2 what the macOS probe answers on WKWebView, and writes facts.txt:
#   1. Does `press` on a voice note's Play start playback although its click is not trusted (no user activation)?
#   2. Does `press` on the microphone start a recording from the fake microphone?
#   3. Does the driver answer while a recording runs?
#   4. How long does one step take, request file to reply file?
# Playback is read from the window (the Pause button, the slider's "m:ss of m:ss") and from the app log (the audio route). No keys, no
# UI Automation. Synthetic data only.
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [string]$Out = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'docs\screens\screens-driver-probe-windows'),
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

$Facts = Join-Path $Out 'facts.txt'
function Fact([string]$text) { Say $text; Add-Content -Path $Facts -Value $text -Encoding UTF8 }
function Elapsed-Text { $text = Get-WindowText 'Kinige'; $m = [regex]::Match([string]$text, '\d+:\d{2} of [\d:]+'); if ($m.Success) { return $m.Value } else { return 'none' } }

[Win]::MakeDpiAware()
if ($Work -match '^[A-Za-z]:\\Users\\') { Fail "the work directory $Work is inside a user profile"; exit 1 }
Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Debug, $Data, $Mirror, $Out | Out-Null
Get-ChildItem $Out -Include *.png, *.txt -Recurse | Remove-Item -Force
Set-Content -Path $Facts -Value '' -Encoding UTF8

Set-Dark
Say ("display: " + [Win]::SetBestResolution())
Start-Sleep -Seconds 3

# Synthetic data without voice notes: the only note of this run is the one the probe records.
Push-Location $Repo
$ErrorActionPreference = 'Continue'
Remove-Item Env:KINIGE_SEED_VOICE -ErrorAction SilentlyContinue
try { & cargo run --release -q -p kinige-store --example seed_demo -- $Data $Mirror 2>&1 | Out-Host; if ($LASTEXITCODE -ne 0) { throw "seed_demo failed" } }
finally { Pop-Location; $ErrorActionPreference = 'Stop' }

& bash (Join-Path $Repo 'scripts/check-test-hooks.sh') (Join-Path $Repo 'web/dist') --expect fake-mic
if ($LASTEXITCODE -ne 0) { Fail 'the fake microphone is not in web/dist' }
& bash (Join-Path $Repo 'scripts/check-test-hooks.sh') (Join-Path $Repo 'web/dist') --expect screens-driver
if ($LASTEXITCODE -ne 0) { Fail 'the screens driver is not in web/dist' }

$install = Start-Process -FilePath $Installer -ArgumentList '/S', "/D=$AppDir" -PassThru
$install.WaitForExit(600000) | Out-Null
if ($install.ExitCode -ne 0) { Fail "the installer exited with $($install.ExitCode)" }
$exe = Get-ChildItem -Path $AppDir -Filter *.exe -File | Where-Object { $_.Name -notmatch '^uninstall' } | Select-Object -First 1
if (-not $exe) { Fail "no application exe in the install directory"; exit 1 }
& bash (Join-Path $Repo 'scripts/check-test-hooks.sh') (Join-Path $Repo 'web/dist') --binary $exe.FullName --expect screens-driver
if ($LASTEXITCODE -ne 0) { Fail 'the screens driver is not in the installed executable' }

Enable-Driver (Join-Path $Work 'driver')
Fact "# Facts about the screens driver on a Windows runner (KINIG-92 probe). $([Environment]::OSVersion.VersionString)"
$main = Start-App $exe.FullName
if (-not $main) { exit 1 }

# The first step waits for the page to say it listens.
$clock = [System.Diagnostics.Stopwatch]::StartNew()
$first = Drive 'main' @{ kind = 'has'; name = 'Inbox' } 60
Fact "first step (waits for the page to listen): ok=$($first.Ok), $($clock.ElapsedMilliseconds) ms"

# ---- 4. Round trip of one step ----
$times = @()
for ($i = 0; $i -lt 20; $i++) {
  $clock.Restart()
  Drive 'main' @{ kind = 'has'; name = 'Inbox' } 10 | Out-Null
  $times += $clock.ElapsedMilliseconds
}
$sorted = @($times | Sort-Object)
Fact "4. round trip of one step (20 'has' steps, ms): median $($sorted[9]), min $($sorted[0]), max $($sorted[19])"
$clock.Restart()
$idleText = Get-WindowText 'Kinige'
Fact "4. one 'text' step of the idle window: $($clock.ElapsedMilliseconds) ms, $(([string]$idleText).Length) characters"

# ---- 2 and 3. Recording from the fake microphone, and the driver while it runs ----
$before = Get-Scalar "select count(*) from attachment where filename like 'voice-%'"
$pressed = Drive 'main' @{ kind = 'press'; name = 'Record voice note' } 20
Fact "2. press Record voice note: ok=$($pressed.Ok) $($pressed.Error)"
Start-Sleep -Seconds 2
$recording = Wait-Name 'Recording' 5000
Fact "2. the recording bar shows 'Recording' 2 s later: $recording"
$ok = 0; $bad = 0; $max = 0
for ($i = 0; $i -lt 8; $i++) {
  $clock.Restart()
  $r = Drive 'main' @{ kind = 'has'; name = 'Recording' } 10
  if ($r.Ok) { $ok++ } else { $bad++ }
  if ($clock.ElapsedMilliseconds -gt $max) { $max = $clock.ElapsedMilliseconds }
  Start-Sleep -Seconds 1
}
Fact "3. driver steps during the recording: $ok answered, $bad not, slowest $max ms"
Shoot '01-recording' 'Kinige'
# The fake microphone's recording ends by itself at 14 s and sends the note.
for ($i = 0; $i -lt 30; $i++) {
  if ((Get-Scalar "select count(*) from attachment where filename like 'voice-%'") -ne $before) { break }
  Start-Sleep -Seconds 2
}
$after = Get-Scalar "select count(*) from attachment where filename like 'voice-%'"
Fact "2. voice notes before / after: $before / $after; newest: $(Get-Scalar "select mime || ' ' || duration_ms || ' ms' from attachment where filename like 'voice-%' order by rowid desc limit 1")"
if ($after -eq $before) { Fail 'no recording was saved after the press' }
Shoot '02-sent' 'Kinige'

# ---- 1. Play without user activation, on the note this run recorded ----
$routeBefore = @(Get-AppLogLines | Where-Object { $_ -match 'audio route: ' }).Count
$played = Drive 'main' @{ kind = 'press'; name = 'Play' } 10
Fact "1. press Play: ok=$($played.Ok) $($played.Error)"
Start-Sleep -Seconds 3
Fact "1. 3 s after the press: Pause button shown: $(Has 'Pause'), 'Can''t play here': $(Has 'Can''t play here')"
Fact "1. slider position after 3 s: $(Elapsed-Text)"
Start-Sleep -Seconds 3
Fact "1. slider position after 6 s: $(Elapsed-Text)"
Fact "1. audio route requests in the app log before / after: $routeBefore / $(@(Get-AppLogLines | Where-Object { $_ -match 'audio route: ' }).Count)"
Shoot '03-playing' 'Kinige'

Stop-App
Fact "scenario errors: $script:Fails"
if ($script:Fails -ne 0) { exit 1 }
