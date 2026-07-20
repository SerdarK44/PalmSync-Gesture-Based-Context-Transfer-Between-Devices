param(
    [switch]$SkipEye,
    [switch]$SkipUi,
    [switch]$SkipTests,
    [switch]$Sign   # code-sign the collected binaries (installer\sign.ps1)
)

$ErrorActionPreference = "Stop"

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  PalmSync - Unified Release Build" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

# ── Prerequisites ────────────────────────────────────────
function Check-Prereq($name, $cmd) {
    try {
        & $cmd --version 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw }
        Write-Host "  [OK] $name" -ForegroundColor Green
    } catch {
        Write-Host "  [FAIL] $name not found" -ForegroundColor Red
        exit 1
    }
}

Write-Host "[0/4] Checking prerequisites..." -ForegroundColor Yellow
Check-Prereq "Rust/Cargo" "cargo"
if (-not $SkipEye) {
    $cmakeBin = "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    if (-not (Test-Path $cmakeBin)) {
        Write-Host "  [FAIL] CMake not found at VS path" -ForegroundColor Red
        exit 1
    }
    Write-Host "  [OK] CMake (VS bundled)" -ForegroundColor Green
}
Write-Host ""

$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$buildOutput = Join-Path $rootDir "build"

# ── 1. Build Rust (Brain) ────────────────────────────────
Write-Host "[1/4] Building Rust (palmsync-daemon)..." -ForegroundColor Yellow

$cargoTargetDir = "C:\Users\serda\.cargo-targets\palmsync"
cargo build --release --manifest-path "$rootDir\Cargo.toml"

if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
Write-Host "  [OK] palmsync-daemon.exe built`n" -ForegroundColor Green

# ── 2. Build C++ (Eye) ───────────────────────────────────
if ($SkipEye) {
    Write-Host "[2/4] Skipping Eye build (-SkipEye)`n" -ForegroundColor Gray
} else {
    Write-Host "[2/4] Building C++ (palmsync-eye)..." -ForegroundColor Yellow

    $eyeDir = Join-Path $rootDir "eye"
    $eyeBuildDir = "$env:USERPROFILE\.palmsync-build\eye"
    $vcpkgRoot = "C:\vcpkg"
    $toolchain = "$vcpkgRoot\scripts\buildsystems\vcpkg.cmake"

    if (-not (Test-Path $eyeBuildDir)) {
        New-Item -ItemType Directory $eyeBuildDir | Out-Null
    }

    & $cmakeBin -S $eyeDir -B $eyeBuildDir `
        -G "Visual Studio 18 2026" -A x64 `
        "-DCMAKE_TOOLCHAIN_FILE=$toolchain" `
        "-DVCPKG_TARGET_TRIPLET=x64-windows"

    if ($LASTEXITCODE -ne 0) { throw "CMake configure failed" }

    & $cmakeBin --build $eyeBuildDir --config Release --parallel

    if ($LASTEXITCODE -ne 0) { throw "CMake build failed" }
    Write-Host "  [OK] palmsync-eye.exe built`n" -ForegroundColor Green
}

# ── 2b. Build C# (Tray UI) ───────────────────────────────
if ($SkipUi) {
    Write-Host "[2b/4] Skipping UI build (-SkipUi)`n" -ForegroundColor Gray
} else {
    Write-Host "[2b/4] Building C# (palmsync-ui)..." -ForegroundColor Yellow

    $dotnet = "C:\Program Files\dotnet\dotnet.exe"
    if (-not (Test-Path $dotnet)) {
        Write-Host "  [FAIL] .NET SDK not found" -ForegroundColor Red
        exit 1
    }

    $uiPublishDir = "$env:USERPROFILE\.palmsync-build\ui\publish"
    & $dotnet publish "$rootDir\ui\PalmSync.Tray.csproj" -c Release -r win-x64 `
        --self-contained false -o $uiPublishDir

    if ($LASTEXITCODE -ne 0) { throw ".NET build failed" }
    Write-Host "  [OK] palmsync-ui.exe built`n" -ForegroundColor Green
}

# ── 3. Run Tests ─────────────────────────────────────────
if ($SkipTests) {
    Write-Host "[3/4] Skipping tests (-SkipTests)`n" -ForegroundColor Gray
} else {
    Write-Host "[3/4] Running tests..." -ForegroundColor Yellow

    cargo test --manifest-path "$rootDir\Cargo.toml"
    if ($LASTEXITCODE -ne 0) { throw "Rust tests failed" }
    Write-Host "  [OK] Rust tests passed" -ForegroundColor Green

    if (-not $SkipEye) {
        $eyeTestExe = "$eyeBuildDir\Release\palmsync-eye-tests.exe"
        if (Test-Path $eyeTestExe) {
            & $eyeTestExe
            if ($LASTEXITCODE -ne 0) { throw "Eye tests failed" }
            Write-Host "  [OK] Eye tests passed" -ForegroundColor Green
        }
    }
    Write-Host ""
}

# ── 4. Collect Artifacts ─────────────────────────────────
Write-Host "[4/4] Collecting build artifacts..." -ForegroundColor Yellow

if (Test-Path $buildOutput) { Remove-Item $buildOutput -Recurse -Force -Confirm:$false }
New-Item -ItemType Directory $buildOutput | Out-Null

$daemonSrc = "$cargoTargetDir\release\palmsync-daemon.exe"
if (Test-Path $daemonSrc) {
    Copy-Item $daemonSrc $buildOutput -Force
    Write-Host "  [OK] palmsync-daemon.exe" -ForegroundColor Green
} else {
    Write-Host "  [WARN] palmsync-daemon.exe not found at $daemonSrc" -ForegroundColor Yellow
}

# gesture_sim: lets you trigger GRAB/DROP without a camera (manual testing)
$simSrc = "$cargoTargetDir\release\gesture_sim.exe"
if (Test-Path $simSrc) {
    Copy-Item $simSrc $buildOutput -Force
    Write-Host "  [OK] gesture_sim.exe" -ForegroundColor Green
}

if (-not $SkipEye) {
    $eyeSrc = "$eyeBuildDir\Release\palmsync-eye.exe"
    if (Test-Path $eyeSrc) {
        Copy-Item $eyeSrc $buildOutput -Force
        # vcpkg copies the OpenCV runtime DLLs next to the exe — bundle them
        # so the camera runs on machines without vcpkg installed.
        Copy-Item "$eyeBuildDir\Release\*.dll" $buildOutput -Force -ErrorAction SilentlyContinue
        $dllCount = (Get-ChildItem "$buildOutput\*.dll" -ErrorAction SilentlyContinue).Count
        Write-Host "  [OK] palmsync-eye.exe + $dllCount runtime DLLs" -ForegroundColor Green
    } else {
        Write-Host "  [WARN] palmsync-eye.exe not found" -ForegroundColor Yellow
    }
}

if (-not $SkipUi) {
    $uiPublishDir = "$env:USERPROFILE\.palmsync-build\ui\publish"
    if (Test-Path "$uiPublishDir\palmsync-ui.exe") {
        New-Item -ItemType Directory "$buildOutput\ui" | Out-Null
        Copy-Item "$uiPublishDir\*" "$buildOutput\ui" -Recurse -Force
        Write-Host "  [OK] palmsync-ui (publish folder)" -ForegroundColor Green
    } else {
        Write-Host "  [WARN] palmsync-ui.exe not found" -ForegroundColor Yellow
    }
}

if (Test-Path "$rootDir\LICENSE") {
    Copy-Item "$rootDir\LICENSE" $buildOutput -Force
}

# ── Optional: code-sign ──────────────────────────────────
if ($Sign) {
    Write-Host "`nSigning binaries..." -ForegroundColor Yellow
    & (Join-Path $rootDir "installer\sign.ps1")
}

# ── Summary ──────────────────────────────────────────────
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  Build Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Output: $buildOutput" -ForegroundColor White

Get-ChildItem $buildOutput | ForEach-Object {
    $sizeMB = [Math]::Round($_.Length / 1MB, 2)
    Write-Host "    $($_.Name) ($sizeMB MB)" -ForegroundColor Gray
}
Write-Host ""
