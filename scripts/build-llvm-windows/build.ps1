# Ori LLVM Windows Build Script
# Run this in PowerShell on Windows to build LLVM with all targets
#
# Usage: .\build.ps1
# Output: LLVM-17.0.6-win64.7z (upload to GitHub releases)

$ErrorActionPreference = "Stop"
$LLVM_VERSION = "17.0.6"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Ori LLVM Windows Builder" -ForegroundColor Cyan
Write-Host "  Building LLVM $LLVM_VERSION" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Install scoop if not present
if (!(Get-Command "scoop" -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Scoop package manager..." -ForegroundColor Yellow
    Set-ExecutionPolicy RemoteSigned -Scope CurrentUser -Force
    Invoke-RestMethod get.scoop.sh | Invoke-Expression
}

# Install build tools
$tools = @("git", "cmake", "ninja", "7zip")
foreach ($tool in $tools) {
    if (!(Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Host "Installing $tool..." -ForegroundColor Yellow
        scoop install $tool
    }
}

# Check for Visual Studio
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (!(Test-Path $vsWhere)) {
    Write-Host ""
    Write-Host "ERROR: Visual Studio not found!" -ForegroundColor Red
    Write-Host "Install Visual Studio 2022 with 'Desktop development with C++' workload" -ForegroundColor Red
    Write-Host "Download: https://visualstudio.microsoft.com/downloads/" -ForegroundColor Yellow
    exit 1
}

$vsPath = & $vsWhere -latest -property installationPath
$vcvarsall = "$vsPath\VC\Auxiliary\Build\vcvarsall.bat"
Write-Host "Found Visual Studio: $vsPath" -ForegroundColor Green

# Setup directories
$workDir = "$env:USERPROFILE\ori-llvm-build"
$sourceDir = "$workDir\llvm-project"
$buildDir = "$workDir\build"
$installDir = "$workDir\LLVM-$LLVM_VERSION-win64"
$outputFile = "$PSScriptRoot\LLVM-$LLVM_VERSION-win64.7z"

Write-Host ""
Write-Host "Work directory: $workDir"
Write-Host "Output file: $outputFile"
Write-Host ""

# Create work directory
New-Item -ItemType Directory -Force -Path $workDir | Out-Null
Set-Location $workDir

# Clone LLVM
if (!(Test-Path $sourceDir)) {
    Write-Host "Cloning LLVM $LLVM_VERSION (this takes a few minutes)..." -ForegroundColor Yellow
    git clone --single-branch --branch "llvmorg-$LLVM_VERSION" --depth 1 `
        "https://github.com/llvm/llvm-project.git" $sourceDir
} else {
    Write-Host "LLVM source already exists, skipping clone" -ForegroundColor Green
}

# Import Visual Studio environment
Write-Host ""
Write-Host "Setting up Visual Studio environment..." -ForegroundColor Yellow
$envBefore = @{}
Get-ChildItem env: | ForEach-Object { $envBefore[$_.Name] = $_.Value }

cmd /c "`"$vcvarsall`" x64 && set" | ForEach-Object {
    if ($_ -match "^([^=]+)=(.*)$") {
        [System.Environment]::SetEnvironmentVariable($matches[1], $matches[2])
    }
}

# Configure
Write-Host ""
Write-Host "Configuring LLVM (this takes a few minutes)..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $buildDir | Out-Null

cmake `
    -S "$sourceDir\llvm" `
    -B $buildDir `
    -G "Ninja" `
    -DCMAKE_BUILD_TYPE=Release `
    -DCMAKE_INSTALL_PREFIX="$installDir" `
    -DLLVM_ENABLE_PROJECTS="lld;clang" `
    -DLLVM_ENABLE_LIBXML2=OFF `
    -DLLVM_ENABLE_ZLIB=OFF `
    -DLLVM_ENABLE_ZSTD=OFF `
    -DLLVM_INCLUDE_TESTS=OFF `
    -DLLVM_INCLUDE_EXAMPLES=OFF `
    -DLLVM_INCLUDE_BENCHMARKS=OFF `
    -DLLVM_INCLUDE_DOCS=OFF `
    -DLLVM_BUILD_TOOLS=ON `
    -DLLVM_BUILD_LLVM_C_DYLIB=OFF `
    -DLLVM_ENABLE_BINDINGS=OFF

if ($LASTEXITCODE -ne 0) { throw "CMake configuration failed" }

# Build
Write-Host ""
Write-Host "Building LLVM (this takes 1-2 hours)..." -ForegroundColor Yellow
Write-Host "Go grab a coffee! ☕" -ForegroundColor Cyan
Write-Host ""

cmake --build $buildDir --config Release

if ($LASTEXITCODE -ne 0) { throw "Build failed" }

# Install
Write-Host ""
Write-Host "Installing..." -ForegroundColor Yellow
cmake --install $buildDir

if ($LASTEXITCODE -ne 0) { throw "Install failed" }

# Package
Write-Host ""
Write-Host "Creating archive..." -ForegroundColor Yellow
if (Test-Path $outputFile) { Remove-Item $outputFile }
7z a -mx9 $outputFile "$installDir\*"

# Verify
Write-Host ""
Write-Host "Verifying build..." -ForegroundColor Yellow
& "$installDir\bin\llvm-config.exe" --version
& "$installDir\bin\llvm-config.exe" --targets-built

# Done!
$size = [math]::Round((Get-Item $outputFile).Length / 1MB, 1)
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  BUILD COMPLETE!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Output: $outputFile ($size MB)" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Create repo: gh repo create upstat-io/llvm-package-windows --public"
Write-Host "2. Upload: gh release create v$LLVM_VERSION `"$outputFile`" --repo upstat-io/llvm-package-windows --title `"LLVM $LLVM_VERSION for Windows`""
Write-Host ""
Write-Host "Or manually upload at: https://github.com/upstat-io/llvm-package-windows/releases/new"
Write-Host ""
