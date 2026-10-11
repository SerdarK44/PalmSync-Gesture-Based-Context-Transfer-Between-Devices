# PalmSync - publish a new version to GitHub Releases
#
#   .\release.ps1                  # next patch version (0.2.0 -> 0.2.1)
#   .\release.ps1 -Version 0.3.0   # a specific version
#   .\release.ps1 -DryRun          # build the setup only, publish nothing
#   .\release.ps1 -SkipBuild       # publish the setup already in dist\
#
# What it does:
#   1. writes the new version to VERSION
#   2. builds everything + dist\PalmSync-Setup-<version>.exe (+ .sha256)
#   3. commits VERSION, tags v<version>, pushes branch + tag
#   4. creates the GitHub release and uploads the setup + .sha256
#
# Installed PalmSync apps check GitHub Releases and offer (or install) it.
# GitHub login: uses the credentials git already has (Git Credential
# Manager), or $env:GITHUB_TOKEN if set. No GitHub CLI needed.
# Keep this file ASCII-only: Windows PowerShell 5.1 reads it as ANSI.

param(
    [string]$Version,
    [string]$Notes,
    [switch]$NoSign,
    [switch]$DryRun,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Definition
$repo = "SerdarK44/PalmSync-Gesture-Based-Context-Transfer-Between-Devices"
Set-Location $root

function Step($text) { Write-Host "`n== $text" -ForegroundColor Cyan }

# git writes normal progress to stderr; Windows PowerShell 5.1 would turn that
# into terminating errors. Run it with Continue and check the exit code.
function Invoke-Git {
    $old = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $errs = @()
    $out = & git.exe @args 2>&1 | ForEach-Object {
        if ($_ -is [System.Management.Automation.ErrorRecord]) { $errs += "$_" } else { "$_" }
    }
    $code = $LASTEXITCODE
    $ErrorActionPreference = $old
    if ($code -ne 0) { throw "git $($args -join ' ') failed:`n$(($errs + $out) -join "`n")" }
    if ($errs) { $errs | Where-Object { $_ -notmatch '^warning: in the working copy' } | Write-Host -ForegroundColor DarkGray }
    return $out
}

# -- 1. Version -------------------------------------------------------------
$current = (Get-Content "$root\VERSION" -Raw).Trim()
if (-not $Version) {
    $p = $current.Split('.')
    $Version = "{0}.{1}.{2}" -f $p[0], $p[1], ([int]$p[2] + 1)
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version must look like 1.2.3" }
if ([version]$Version -lt [version]$current -and -not $DryRun) {
    throw "New version $Version must not be lower than the current $current"
}
$tag = "v$Version"

if (-not $DryRun) {
    Step "Checking git"
    $dirty = Invoke-Git status --porcelain | Where-Object { $_ -and $_ -notmatch ' VERSION$' }
    if ($dirty) {
        Write-Host ($dirty -join "`n")
        throw "Uncommitted changes. Commit (or stash) them first, then run release.ps1 again."
    }
    if (Invoke-Git tag --list $tag) { throw "Tag $tag already exists" }
}

Set-Content -Path "$root\VERSION" -Value $Version -Encoding ascii
Write-Host "Releasing $tag (was v$current)" -ForegroundColor Green

# -- 2. Build ---------------------------------------------------------------
if ($SkipBuild) {
    Write-Host "Skipping build, using dist\PalmSync-Setup-$Version.exe" -ForegroundColor Yellow
} else {
Step "Building"
$buildArgs = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "$root\build.ps1", "-Installer")
if (-not $NoSign) { $buildArgs += "-Sign" }
# Child process: cargo/cmake write progress to stderr, which a direct call
# under ErrorActionPreference=Stop would treat as a failure.
& powershell.exe @buildArgs
if ($LASTEXITCODE -ne 0) {
    Set-Content -Path "$root\VERSION" -Value $current -Encoding ascii
    throw "Build failed - VERSION restored to $current"
}
}

$setup = "$root\dist\PalmSync-Setup-$Version.exe"
$sha = "$setup.sha256"
if (-not (Test-Path $setup) -or -not (Test-Path $sha)) { throw "Setup or .sha256 missing in dist\" }

if ($DryRun) {
    Set-Content -Path "$root\VERSION" -Value $current -Encoding ascii
    Write-Host "`nDry run: $setup built, nothing published." -ForegroundColor Yellow
    exit 0
}

# -- 3. Commit, tag, push ---------------------------------------------------
Step "Tagging $tag"
$lastTag = Invoke-Git tag --list "v*" --sort=-v:refname | Where-Object { $_ -and $_ -ne $tag } | Select-Object -First 1
if (-not $Notes) {
    $range = if ($lastTag) { "$lastTag..HEAD" } else { "HEAD" }
    $log = Invoke-Git log $range --pretty="- %s" --no-merges | Select-Object -First 30
    $Notes = "## PalmSync $tag`n`n" + ($log -join "`n") +
             "`n`nKurulu PalmSync uygulamalari bu surumu kendiliginden bulur (Ayarlar > Hakkinda > Guncellemeler)." +
             "`nYeni kurulum icin asagidaki PalmSync-Setup-$Version.exe dosyasini indirin."
}
Invoke-Git add VERSION | Out-Null
if (Invoke-Git status --porcelain VERSION) { Invoke-Git commit -m "release: $tag" | Out-Null }
Invoke-Git tag -a $tag -m "PalmSync $tag" | Out-Null
$branch = (Invoke-Git rev-parse --abbrev-ref HEAD | Select-Object -First 1).Trim()
Invoke-Git push origin $branch | Out-Host
Invoke-Git push origin $tag | Out-Host

# -- 4. GitHub release ------------------------------------------------------
Step "Creating GitHub release"
$token = $env:GITHUB_TOKEN
if (-not $token) {
    $old = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $cred = "protocol=https`nhost=github.com`n`n" | git credential fill 2>$null
    $ErrorActionPreference = $old
    $token = ($cred | Where-Object { $_ -like "password=*" }) -replace '^password=', ''
}
if (-not $token) { throw "No GitHub credentials: set GITHUB_TOKEN or log in with git once" }

$headers = @{
    Authorization          = "Bearer $token"
    Accept                 = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2022-11-28"
}
$body = @{ tag_name = $tag; name = "PalmSync $tag"; body = $Notes; draft = $false; prerelease = $false } |
    ConvertTo-Json
$release = Invoke-RestMethod -Method Post -Uri "https://api.github.com/repos/$repo/releases" `
    -Headers $headers -Body ([Text.Encoding]::UTF8.GetBytes($body)) -ContentType "application/json; charset=utf-8"

foreach ($file in @($setup, $sha)) {
    $name = Split-Path $file -Leaf
    Write-Host "  uploading $name ..."
    Invoke-RestMethod -Method Post -Headers $headers -InFile $file -ContentType "application/octet-stream" `
        -Uri "https://uploads.github.com/repos/$repo/releases/$($release.id)/assets?name=$name" | Out-Null
}

Write-Host "`nPublished: $($release.html_url)" -ForegroundColor Green
Write-Host "Installed PalmSync apps will offer $tag within a few hours (or at next start)." -ForegroundColor Green
