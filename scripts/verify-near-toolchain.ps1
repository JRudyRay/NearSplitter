$ErrorActionPreference = 'Stop'

Write-Host "== Toolchain versions ==" -ForegroundColor Cyan
clang --version
clang++ --version
rustc --version
cargo --version
rustup show
rustup target list --installed

Write-Host "== Build checks ==" -ForegroundColor Cyan
$projectRoot = Resolve-Path "$PSScriptRoot\..\contracts\near_splitter"
$vswhere = "$env:ProgramFiles(x86)\Microsoft Visual Studio\Installer\vswhere.exe"
$vsDevCmd = $null

if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($vsPath) {
        $vsDevCmd = "$vsPath\Common7\Tools\VsDevCmd.bat"
    }
}

if (-not $vsDevCmd) {
    $fallback = "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat"
    if (Test-Path $fallback) {
        $vsDevCmd = $fallback
    }
}

if (-not $vsDevCmd) {
    throw "VsDevCmd.bat not found. Install Visual Studio Build Tools with C++ workload."
}

$projectRootPath = $projectRoot.Path
$testCmd = 'call "' + $vsDevCmd + '" -arch=amd64 -host_arch=amd64 && set CC=cl && set CXX=cl && cd /d "' + $projectRootPath + '" && cargo test --lib --target x86_64-pc-windows-msvc'
cmd /c $testCmd
if ($LASTEXITCODE -ne 0) {
    throw "MSVC test run failed with exit code $LASTEXITCODE"
}

$buildCmd = 'set CC=clang && set CXX=clang++ && cd /d "' + $projectRootPath + '" && cargo build --release --target wasm32-unknown-unknown'
cmd /c $buildCmd
if ($LASTEXITCODE -ne 0) {
    throw "Wasm build failed with exit code $LASTEXITCODE"
}

Write-Host "All checks passed." -ForegroundColor Green
