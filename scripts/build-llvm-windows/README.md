# Build LLVM for Windows

One-click script to build LLVM 17.0.6 with all targets for Windows.

## Prerequisites

- Windows 10/11
- [Visual Studio 2022](https://visualstudio.microsoft.com/downloads/) with "Desktop development with C++" workload

## Build

Open PowerShell and run:

```powershell
powershell -ExecutionPolicy Bypass -File \\wsl$\Ubuntu\home\eric\ori_lang\scripts\build-llvm-windows\build.ps1
```

Or copy to a Windows path first (faster file access):

```powershell
cp \\wsl$\Ubuntu\home\eric\ori_lang\scripts\build-llvm-windows\build.ps1 C:\
powershell -ExecutionPolicy Bypass -File C:\build.ps1
```

The script will:
1. Install Scoop, CMake, Ninja, 7zip (if needed)
2. Clone LLVM 17.0.6
3. Build with all targets (~1-2 hours)
4. Package as `LLVM-17.0.6-win64.7z`

## After Building

Upload to GitHub releases:

```powershell
gh repo create upstat-io/llvm-package-windows --public
gh release create v17.0.6 LLVM-17.0.6-win64.7z --repo upstat-io/llvm-package-windows --title "LLVM 17.0.6 for Windows"
```

Then enable Windows builds in `.github/workflows/release.yml` by removing the `if: false` line.
