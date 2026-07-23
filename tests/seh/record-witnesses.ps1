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
  [string[]]$RunArgs = @("alpha","beta")
)
$ErrorActionPreference = "Continue"
$witnessDir = "$PSScriptRoot\witnesses"
$out = Join-Path $OutDir $Arm
New-Item -ItemType Directory -Force -Path $out | Out-Null

foreach ($ori in Get-ChildItem -Path $witnessDir -Filter *.ori | Sort-Object Name) {
  $name = [IO.Path]::GetFileNameWithoutExtension($ori.Name)
  $exe  = Join-Path $out "$name.exe"
  $so   = Join-Path $out "$name.stdout"
  $se   = Join-Path $out "$name.stderr"

  $build = & $OriExe build $ori.FullName -o $exe 2>&1 | Out-String
  if (-not (Test-Path $exe)) {
    $rec = [ordered]@{
      arm=$Arm; commit=$Commit; compiler=$OriExe; witness=$name
      args=($RunArgs -join ' '); build_ok=$false; build_output=$build.Trim()
      exit_signed=$null; exit_hex=$null; stdout=$null; stderr=$null
      exe_path=$exe; recorded_at=(Get-Date -Format "o")
    }
    ($rec | ConvertTo-Json -Compress) | Add-Content -Path $ResultPath
    Write-Host "[$Arm] $name BUILD_FAIL"
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
