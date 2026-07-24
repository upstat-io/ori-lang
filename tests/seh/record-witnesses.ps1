# Compile and run each SEH witness with a given native ori.exe, recording one
# JSON object per witness (arm, commit, compiler, witness, args, build_ok,
# exit_signed, exit_hex, stdout, stderr, exe_path, recorded_at) to -ResultPath.
#
# Requires a native Windows toolchain. See docs/development/testing-seh-from-wsl.md.
#
#   .\record-witnesses.ps1 -OriExe <path\to\ori.exe> -Commit <sha> -Arm cured `
#       -ResultPath .\results.jsonl
param(
  [Parameter(Mandatory=$true)][string]$OriExe,
  [Parameter(Mandatory=$true)][string]$Commit,
  [Parameter(Mandatory=$true)][string]$Arm,
  [Parameter(Mandatory=$true)][string]$ResultPath,
  [string]$OutDir = "$PSScriptRoot\out",
  [string[]]$RunArgs = @("alpha","beta"),
  [switch]$AppendResults
)
$ErrorActionPreference = "Continue"
$witnessDir = "$PSScriptRoot\witnesses"
$out = Join-Path $OutDir $Arm
New-Item -ItemType Directory -Force -Path $out | Out-Null

# Result-file lifecycle is explicit: truncate per run unless -AppendResults is
# given, so one recording cannot silently inherit rows from an earlier one.
if (-not $AppendResults) { Set-Content -Path $ResultPath -Value $null -NoNewline }

foreach ($ori in Get-ChildItem -Path $witnessDir -Filter *.ori | Sort-Object Name) {
  $name = [IO.Path]::GetFileNameWithoutExtension($ori.Name)
  $exe  = Join-Path $out "$name.exe"
  $so   = Join-Path $out "$name.stdout"
  $se   = Join-Path $out "$name.stderr"

  # Fail closed against a stale executable: remove any prior artifact, then
  # require BOTH a zero build exit status AND a newly produced file. Testing
  # only for the file's existence would run a previous run's binary when this
  # compiler invocation fails, recording build_ok for a compiler never exercised.
  Remove-Item -Path $exe -Force -ErrorAction SilentlyContinue
  $build = & $OriExe build $ori.FullName -o $exe 2>&1 | Out-String
  $buildExit = $LASTEXITCODE
  if ($buildExit -ne 0 -or -not (Test-Path $exe)) {
    $rec = [ordered]@{
      arm=$Arm; commit=$Commit; compiler=$OriExe; witness=$name
      args=($RunArgs -join ' '); build_ok=$false; build_exit=$buildExit
      build_output=$build.Trim()
      exit_signed=$null; exit_hex=$null; stdout=$null; stderr=$null
      exe_path=$exe; recorded_at=(Get-Date -Format "o")
    }
    ($rec | ConvertTo-Json -Compress) | Add-Content -Path $ResultPath
    Write-Host "[$Arm] $name BUILD_FAIL (build_exit=$buildExit)"
    continue
  }

  $p = Start-Process -FilePath $exe -ArgumentList $RunArgs -NoNewWindow -PassThru -Wait `
       -RedirectStandardOutput $so -RedirectStandardError $se
  $code = $p.ExitCode
  $rec = [ordered]@{
    arm         = $Arm
    commit      = $Commit
    compiler    = $OriExe
    witness     = $name
    args        = ($RunArgs -join ' ')
    build_ok    = $true
    build_exit  = $buildExit
    exit_signed = $code
    exit_hex    = ('0x{0:X8}' -f ($code -band 0xFFFFFFFF))
    stdout      = [string](Get-Content $so -Raw -ErrorAction SilentlyContinue)
    stderr      = [string](Get-Content $se -Raw -ErrorAction SilentlyContinue)
    exe_path    = $exe
    recorded_at = (Get-Date -Format "o")
  }
  ($rec | ConvertTo-Json -Compress) | Add-Content -Path $ResultPath
  Write-Host "[$Arm] $name exit=$code ($($rec.exit_hex))"
}
Write-Host "RECORDED -> $ResultPath"
