# Falsify the recorder's fail-closed contract: prove it reports a build failure
# instead of running a stale executable left by an earlier recording.
#
# Prepopulates a witness output with a supplied known-good executable, then runs
# the recorder with a shim compiler that exits nonzero without producing its -o
# target. A conforming recorder deletes the stale file and records build_ok=false;
# a regressed one runs the stale binary and records a passing row.
#
#   .\selftest-recorder.ps1 -StaleExe <path\to\any\working\witness.exe>
#
# Exit 0 = the recorder failed closed (contract holds). Exit 1 = regression.
param(
  [Parameter(Mandatory=$true)][string]$StaleExe,
  [string]$WorkDir = "$env:TEMP\ori-seh-selftest"
)
$ErrorActionPreference = "Continue"

if (-not (Test-Path $StaleExe)) { Write-Host "SELFTEST ERROR: -StaleExe not found: $StaleExe"; exit 2 }

Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
$armDir = Join-Path $WorkDir "out\selftest"
New-Item -ItemType Directory -Force -Path $armDir | Out-Null

# A compiler shim that always fails and never writes its -o target.
$shim = Join-Path $WorkDir "failing-ori.cmd"
Set-Content -Path $shim -Value "@echo off`r`nexit /b 3"

# Seed the destination with a known-good executable. If the recorder regressed,
# this is what it would run, and the row would look like a successful build.
$witness = (Get-ChildItem -Path "$PSScriptRoot\witnesses" -Filter *.ori | Sort-Object Name | Select-Object -First 1)
$staleDest = Join-Path $armDir "$([IO.Path]::GetFileNameWithoutExtension($witness.Name)).exe"
Copy-Item -Path $StaleExe -Destination $staleDest -Force
if (-not (Test-Path $staleDest)) { Write-Host "SELFTEST ERROR: could not seed stale executable"; exit 2 }

$results = Join-Path $WorkDir "selftest.jsonl"
& "$PSScriptRoot\record-witnesses.ps1" -OriExe $shim -Commit "SELFTEST" -Arm "selftest" `
  -ResultPath $results -OutDir (Join-Path $WorkDir "out") | Out-Null

if (-not (Test-Path $results)) { Write-Host "SELFTEST FAIL: recorder produced no results"; exit 1 }

$rows = Get-Content $results | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json }
$bad = $rows | Where-Object { $_.build_ok -ne $false }
if ($bad) {
  Write-Host "SELFTEST FAIL: recorder reported build_ok=$($bad[0].build_ok) after a failing compile"
  Write-Host "  a stale executable was treated as a successful build"
  exit 1
}
if (Test-Path $staleDest) {
  Write-Host "SELFTEST FAIL: stale executable still present; recorder did not clear it before building"
  exit 1
}
Write-Host "SELFTEST PASS: failing compile recorded build_ok=false; stale executable removed unexecuted"
exit 0
