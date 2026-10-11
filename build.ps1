param(
    [switch]$SkipEye,
    [switch]$SkipUi,
    [switch]$SkipTests,
    [switch]$Sign,      # code-sign the collected binaries (installer\sign.ps1)
    [switch]$Installer  # also build dist\PalmSync-Setup-<version>.exe (+ .sha256)
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

# Single source of truth for the version: the VERSION file. It ends up in the
# UI assembly (what the updater compares) and in the installer name.
$version = (Get-Content (Join-Path $rootDir "VERSION") -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "VERSION must look like 1.2.3 (got '$version')" }
Write-Host "Version: $version`n" -ForegroundColor Cyan

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
        --self-contained false -o $uiPublishDir "-p:Version=$version"

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
        # Face model: keeps the user's face out of the hand search
        Copy-Item "$eyeBuildDir\Release\*.xml" $buildOutput -Force -ErrorAction SilentlyContinue
        $dllCount = (Get-ChildItem "$buildOutput\*.dll" -ErrorAction SilentlyContinue).Count
        Write-Host "  [OK] palmsync-eye.exe + $dllCount runtime DLLs" -ForegroundColor Green
    } else {
        Write-Host "  [WARN] palmsync-eye.exe not found" -ForegroundColor Yellow
    }
}

# Visual C++ runtime (app-local). palmsync-eye.exe, the OpenCV DLLs and
# palmsync-daemon.exe import MSVCP140 / VCRUNTIME140(_1). On PCs without the
# VC++ Redistributable the camera exited instantly with 0xC0000135 ("camera
# doesn't show up on other computers"). Shipping the DLLs next to the exes fixes it.
$vsRoot = "C:\Program Files\Microsoft Visual Studio\18\Community"
$crtDir = Get-ChildItem "$vsRoot\VC\Redist\MSVC\*\x64\Microsoft.VC14*.CRT" -Directory -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1
if ($crtDir) {
    foreach ($dll in "msvcp140.dll", "msvcp140_1.dll", "msvcp140_2.dll", "vcruntime140.dll", "vcruntime140_1.dll") {
        $src = Join-Path $crtDir.FullName $dll
        if (Test-Path $src) { Copy-Item $src $buildOutput -Force }
    }
    Write-Host "  [OK] VC++ runtime DLLs ($($crtDir.FullName))" -ForegroundColor Green
} else {
    Write-Host "  [WARN] VC++ redist folder not found - target PCs need vc_redist.x64.exe" -ForegroundColor Yellow
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

# ── Optional: installer ──────────────────────────────────
if ($Installer) {
    Write-Host "`nBuilding installer..." -ForegroundColor Yellow
    $iscc = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $iscc) { throw "Inno Setup 6 (ISCC.exe) not found - install it from jrsoftware.org" }

    & $iscc "/DMyAppVersion=$version" "$rootDir\installer\PalmSync.iss" | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Installer build failed" }

    $setup = Join-Path $rootDir "dist\PalmSync-Setup-$version.exe"
    if ($Sign) {
        $cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
            Where-Object { $_.Subject -eq "CN=PalmSync (Self-Signed)" -and $_.NotAfter -gt (Get-Date) } |
            Select-Object -First 1
        if ($cert) {
            Set-AuthenticodeSignature -FilePath $setup -Certificate $cert -HashAlgorithm SHA256 `
                -TimestampServer "http://timestamp.digicert.com" | Out-Null
            Write-Host "  [OK] installer signed" -ForegroundColor Green
        }
    }

    # The in-app updater verifies the download against this hash.
    $hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -Path "$setup.sha256" -Value "$hash  PalmSync-Setup-$version.exe" -Encoding ascii
    $sizeMB = [Math]::Round((Get-Item $setup).Length / 1MB, 1)
    Write-Host "  [OK] $setup ($sizeMB MB)" -ForegroundColor Green
    Write-Host "  [OK] SHA256 $hash" -ForegroundColor Green
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
