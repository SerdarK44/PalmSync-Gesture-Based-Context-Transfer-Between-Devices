# PalmSync — Trust the self-signed certificate on a machine
#
# Run this AS ADMINISTRATOR on each computer where you install a self-signed
# PalmSync build. It imports PalmSync-PublicCert.cer into the machine's
# Trusted Root + Trusted Publishers stores, which silences SmartScreen /
# "Unknown Publisher" for PalmSync on that machine.
#
# NOTE: This does NOT make PalmSync run under Smart App Control (SAC). SAC does
# not honor local certificate trust — for SAC you must turn SAC off (Windows
# Security -> App & browser control -> Smart App Control -> Off) or use an
# EV/reputable certificate.

$ErrorActionPreference = "Stop"

$cer = Join-Path $PSScriptRoot "PalmSync-PublicCert.cer"
if (-not (Test-Path $cer)) {
    Write-Error "PalmSync-PublicCert.cer not found next to this script. Run sign.ps1 first to generate it."
    exit 1
}

# Require elevation
$admin = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
    ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    Write-Error "Please run this script as Administrator."
    exit 1
}

Import-Certificate -FilePath $cer -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
Write-Host "  Trusted Root: imported" -ForegroundColor Green
Import-Certificate -FilePath $cer -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
Write-Host "  Trusted Publishers: imported" -ForegroundColor Green

Write-Host "`nPalmSync certificate is now trusted on this machine." -ForegroundColor Cyan
