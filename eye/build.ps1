# PalmSync Eye - Build Script
# Uses VS bundled CMake + vcpkg toolchain
# Build output goes to ~/.palmsync-build/eye to avoid WDAC issues

$ErrorActionPreference = "Stop"

$vsPath = "C:\Program Files\Microsoft Visual Studio\18\Community"
$cmakeBin = "$vsPath\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
$vcpkgRoot = "C:\vcpkg"
$toolchain = "$vcpkgRoot\scripts\buildsystems\vcpkg.cmake"

$eyeDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$buildDir = "$env:USERPROFILE\.palmsync-build\eye"

Write-Host "`n[0/3] Verifying Camera 2.0 model assets..." -ForegroundColor Yellow
& (Join-Path $eyeDir "fetch-models.ps1")

Write-Host "PalmSync Eye - Build" -ForegroundColor Cyan
Write-Host "Source : $eyeDir" -ForegroundColor Gray
Write-Host "Build  : $buildDir" -ForegroundColor Gray
Write-Host "CMake  : $cmakeBin" -ForegroundColor Gray

if (-not (Test-Path $buildDir)) {
    New-Item -ItemType Directory $buildDir | Out-Null
}

Write-Host "`n[1/3] CMake configure..." -ForegroundColor Yellow
& $cmakeBin -S $eyeDir -B $buildDir `
    -G "Visual Studio 18 2026" -A x64 `
    "-DCMAKE_TOOLCHAIN_FILE=$toolchain" `
    "-DVCPKG_TARGET_TRIPLET=x64-windows"

if ($LASTEXITCODE -ne 0) { throw "CMake configure failed" }

Write-Host "`n[2/3] CMake build (Release)..." -ForegroundColor Yellow
& $cmakeBin --build $buildDir --config Release --parallel

if ($LASTEXITCODE -ne 0) { throw "CMake build failed" }

Write-Host "`n[3/3] Running eye tests..." -ForegroundColor Yellow
& $cmakeBin --build $buildDir --config Release --target RUN_TESTS

if ($LASTEXITCODE -ne 0) { throw "Tests failed" }

Write-Host "`nBuild complete!" -ForegroundColor Green
Write-Host "Executable: $buildDir\Release\palmsync-eye.exe" -ForegroundColor White
