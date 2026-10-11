[CmdletBinding()]
param(
    [string]$Destination = (Join-Path $PSScriptRoot "models"),
    [switch]$CheckOnly
)

$ErrorActionPreference = "Stop"
$manifestPath = Join-Path $PSScriptRoot "models\models.json"

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Model manifest not found: $manifestPath"
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1 -or -not $manifest.assets) {
    throw "Unsupported or empty model manifest: $manifestPath"
}

if (-not (Test-Path -LiteralPath $Destination -PathType Container)) {
    if ($CheckOnly) { throw "Model directory not found: $Destination" }
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
}
$destinationPath = (Resolve-Path -LiteralPath $Destination).Path

function Test-ModelAsset {
    param([string]$Path, $Asset)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $false }
    $file = Get-Item -LiteralPath $Path
    if ($file.Length -ne [int64]$Asset.bytes) { return $false }
    $actualHash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    return $actualHash -eq [string]$Asset.sha256
}

$verified = 0
foreach ($asset in $manifest.assets) {
    $fileName = [string]$asset.file
    if ([IO.Path]::GetFileName($fileName) -ne $fileName) {
        throw "Manifest asset must be a plain file name: $fileName"
    }

    $target = Join-Path $destinationPath $fileName
    if (Test-ModelAsset -Path $target -Asset $asset) {
        Write-Host "  [OK] $fileName" -ForegroundColor Green
        $verified++
        continue
    }

    if ($CheckOnly) {
        throw "Model asset is missing or failed verification: $target"
    }

    $temporary = Join-Path $destinationPath (".{0}.{1}.download" -f $fileName, $PID)
    try {
        Write-Host "  [GET] $($asset.name)" -ForegroundColor Yellow
        Invoke-WebRequest -Uri ([string]$asset.url) -OutFile $temporary -UseBasicParsing

        if (-not (Test-ModelAsset -Path $temporary -Asset $asset)) {
            throw "Downloaded asset failed size or SHA-256 verification: $fileName"
        }

        if (Test-Path -LiteralPath $target) {
            $stamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
            $backup = "$target.invalid-$stamp"
            Move-Item -LiteralPath $target -Destination $backup
            Write-Host "  [WARN] Preserved invalid file as $([IO.Path]::GetFileName($backup))" -ForegroundColor Yellow
        }
        Move-Item -LiteralPath $temporary -Destination $target
        Write-Host "  [OK] Verified $fileName" -ForegroundColor Green
        $verified++
    } finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
    }
}

Write-Host "Verified $verified/$($manifest.assets.Count) Camera 2.0 assets." -ForegroundColor Cyan

