# PalmSync — Code signing
#
# Signs all PalmSync binaries (and optionally the installer) so they no longer
# show up as "Unknown Publisher". Uses a self-signed certificate by default;
# point $PfxPath/$PfxPassword at a real code-signing cert for distribution.
#
# IMPORTANT about Smart App Control (SAC): a SELF-SIGNED cert does NOT satisfy
# SAC — SAC trusts reputation/EV-signed apps, not local certs. Self-signing
# removes "Unknown Publisher" on machines WITHOUT SAC (and, once the cert is
# trusted via trust-cert.ps1, silences SmartScreen there). To run under SAC you
# must either turn SAC off or use a reputable/EV certificate.

param(
    [string]$PfxPath,                 # optional: real cert; omit to self-sign
    [string]$PfxPassword,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
$build = Join-Path $root "build"

# ── Obtain a signing certificate ─────────────────────────
if ($PfxPath) {
    Write-Host "Using provided certificate: $PfxPath" -ForegroundColor Cyan
    $cert = Get-PfxCertificate -FilePath $PfxPath -Password `
        (ConvertTo-SecureString $PfxPassword -AsPlainText -Force)
} else {
    $subject = "CN=PalmSync (Self-Signed)"
    $cert = Get-ChildItem Cert:\CurrentUser\My |
        Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } |
        Select-Object -First 1

    if (-not $cert) {
        Write-Host "Creating self-signed code-signing certificate..." -ForegroundColor Yellow
        $cert = New-SelfSignedCertificate -Type CodeSigningCert `
            -Subject $subject -CertStoreLocation Cert:\CurrentUser\My `
            -KeyUsage DigitalSignature -KeyExportPolicy Exportable `
            -NotAfter (Get-Date).AddYears(5)

        # Export the public cert so it can be trusted on target machines
        $cerOut = Join-Path $PSScriptRoot "PalmSync-PublicCert.cer"
        Export-Certificate -Cert $cert -FilePath $cerOut | Out-Null
        Write-Host "Public cert exported: $cerOut" -ForegroundColor Green
        Write-Host "  -> run trust-cert.ps1 (as admin) on each machine to trust it." -ForegroundColor Gray
    } else {
        Write-Host "Reusing existing self-signed certificate." -ForegroundColor Cyan
    }
}

# ── Sign the binaries ────────────────────────────────────
$targets = @(
    "palmsync-daemon.exe",
    "palmsync-eye.exe",
    "gesture_sim.exe",
    "ui\palmsync-ui.exe"
) | ForEach-Object { Join-Path $build $_ } | Where-Object { Test-Path $_ }

foreach ($file in $targets) {
    $r = Set-AuthenticodeSignature -FilePath $file -Certificate $cert `
        -TimestampServer $TimestampUrl -HashAlgorithm SHA256

    # A self-signed cert reports "UnknownError" (untrusted root) on machines
    # where it isn't trusted yet — the signature IS applied. Treat "signature
    # present" as success; only a null signer is a real failure.
    $applied = $null -ne $r.SignerCertificate
    if ($r.Status -eq 'Valid') { $label = 'VALID'; $color = 'Green' }
    elseif ($applied)          { $label = 'signed (trust cert to validate)'; $color = 'Green' }
    else                       { $label = "FAILED: $($r.Status)"; $color = 'Red' }

    Write-Host ("  [{0}] {1}" -f $label, (Split-Path $file -Leaf)) -ForegroundColor $color
}

Write-Host "`nBinaries signed. Now build the installer (installer\PalmSync.iss)," -ForegroundColor Cyan
Write-Host "then sign the installer exe too:" -ForegroundColor Cyan
Write-Host '  Set-AuthenticodeSignature dist\PalmSync-Setup-0.1.0.exe -Certificate $cert -TimestampServer ' + $TimestampUrl -ForegroundColor Gray
