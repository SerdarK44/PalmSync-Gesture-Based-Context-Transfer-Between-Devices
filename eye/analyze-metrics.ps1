# PalmSync Eye - Camera 2.0 validation report
#
# Guided run (recommended, see run-camera-tests.ps1):
#   palmsync-eye.exe --detector landmark --guide 20 --metrics run.csv
# Free run:
#   palmsync-eye.exe --window --detector landmark --metrics run.csv
#   (do N open -> fist -> open cycles, then press Esc)
# Analyse:
#   .\analyze-metrics.ps1 run.csv [-Expected 20] [-Title "1 m"] [-ReportPath rapor.md]
#
# Guided CSVs carry a `cue` column: a GRAB only counts while the screen asked
# for a fist (cue=fist), a DROP only while it asked for an open hand
# (cue=open); any other gesture is a wrong event. MISS_* rows are cues that
# timed out. Latency runs from the first frame whose RAW pose matches the new
# gesture to the frame that emitted the event; camera exposure/transport
# delay before that frame is not included.
#
# This file is saved as UTF-8 with BOM so Windows PowerShell 5.1 reads the
# Turkish report text correctly.

param(
    [Parameter(Mandatory = $true)][string]$Path,
    [int]$Expected = 0,
    [int]$MedianTargetMs = 150,
    [int]$P95TargetMs = 250,
    [double]$SuccessTarget = 0.95,
    [string]$Title = "",
    [string]$ReportPath = "",
    [switch]$PassThru
)

$ErrorActionPreference = "Stop"
$rows = @(Import-Csv -Path $Path)
if ($rows.Count -lt 2) { throw "Kayıtta yeterli kare yok: $Path" }

function Get-Percentile([double[]]$values, [double]$p) {
    if ($values.Count -eq 0) { return [double]::NaN }
    $sorted = $values | Sort-Object
    $index = [Math]::Max(0, [Math]::Ceiling($p * $sorted.Count) - 1)
    return [double]$sorted[$index]
}

$hasCue = $rows[0].PSObject.Properties.Name -contains "cue"
$guided = $hasCue -and @($rows | Where-Object { $_.cue }).Count -gt 0
$emptyRun = $guided -and @($rows | Where-Object { $_.cue -eq "empty" }).Count -gt 0
$completed = (-not $guided) -or ($rows[-1].cue -eq "done")

$t = $rows | ForEach-Object { [double]$_.t_ms }
$duration = ($t[-1] - $t[0]) / 1000.0
$fps = $rows.Count / [Math]::Max(0.001, $duration)
$handShare = @($rows | Where-Object { $_.found -eq "1" }).Count / $rows.Count
$trackMs = [double[]]($rows | ForEach-Object { [double]$_.track_ms })

$latency = @{ GRAB = New-Object System.Collections.Generic.List[double]
              DROP = New-Object System.Collections.Generic.List[double] }
$events = @{ GRAB = 0; DROP = 0 }
$ok = @{ GRAB = 0; DROP = 0 }
$miss = @{ GRAB = 0; DROP = 0 }
$wrong = 0
$stuck = 0
$setup = 0
$after = 0
for ($i = 0; $i -lt $rows.Count; $i++) {
    $ev = $rows[$i].event
    if ($ev -eq "MISS_GRAB") { $miss.GRAB++; continue }
    if ($ev -eq "MISS_DROP") { $miss.DROP++; continue }
    if ($ev -like "STUCK_*") { $stuck++; continue }
    if ($ev -ne "GRAB" -and $ev -ne "DROP") { continue }
    # Gestures made while getting into position are not detector errors.
    # Gestures after the run ended come from lowering the hand: a real
    # false-trigger risk, reported separately so it is not lost.
    if ($guided -and $rows[$i].cue -in @("countdown", "show_open")) { $setup++; continue }
    if ($guided -and $rows[$i].cue -eq "done") { $after++; continue }
    $events[$ev]++
    $target = if ($ev -eq "GRAB") { "fist" } else { "open" }
    if ($guided -and $rows[$i].cue -ne $target) { $wrong++; continue }
    $ok[$ev]++

    # Smoothing can make the raw pose lead the event; find the raw run of
    # the target pose that ends at or just before the event frame.
    $j = $i
    while ($j -ge 0 -and ($i - $j) -lt 15 -and $rows[$j].raw_pose -ne $target) { $j-- }
    if ($j -lt 0 -or $rows[$j].raw_pose -ne $target) { continue }
    while ($j -gt 0 -and $rows[$j - 1].raw_pose -eq $target) { $j-- }
    $latency[$ev].Add([double]$rows[$i].t_ms - [double]$rows[$j].t_ms)
}

$all = [double[]](@($latency.GRAB) + @($latency.DROP))
$median = Get-Percentile $all 0.5
$p95 = Get-Percentile $all 0.95

# Attempts: guided runs know every cue; free runs need -Expected.
$attempts = @{}
foreach ($name in @("GRAB", "DROP")) {
    $attempts[$name] = if ($guided) { $ok[$name] + $miss[$name] } elseif ($Expected -gt 0) { $Expected } else { 0 }
}

$checks = New-Object System.Collections.Generic.List[object]
function Add-Check([string]$label, [bool]$pass) { $checks.Add([pscustomobject]@{ Label = $label; Pass = $pass }) }

if ($guided) { Add-Check "Senaryo sonuna kadar tamamlandı" $completed }
if ($emptyRun) {
    Add-Check ("Yanlış olay yok (GRAB {0}, DROP {1})" -f $events.GRAB, $events.DROP) (($events.GRAB + $events.DROP) -eq 0)
} else {
    foreach ($name in @("GRAB", "DROP")) {
        if ($attempts[$name] -gt 0) {
            $rate = $ok[$name] / $attempts[$name]
            $extra = if (-not $guided) { [Math]::Max(0, $events[$name] - $attempts[$name]) } else { 0 }
            Add-Check ("{0} başarısı {1}/{2} (%{3:N0}, hedef ≥ %{4:N0})" -f $name, $ok[$name], $attempts[$name], ($rate * 100), ($SuccessTarget * 100)) `
                (($rate -ge $SuccessTarget) -and $extra -eq 0)
        }
    }
    if ($guided) { Add-Check ("Yanlış zamanda olay: {0}" -f $wrong) ($wrong -eq 0) }
    if ($all.Count -gt 0) {
        Add-Check ("Medyan gecikme {0:N0} ms (hedef ≤ {1})" -f $median, $MedianTargetMs) ($median -le $MedianTargetMs)
        Add-Check ("p95 gecikme {0:N0} ms (hedef ≤ {1})" -f $p95, $P95TargetMs) ($p95 -le $P95TargetMs)
    }
}
$failed = @($checks | Where-Object { -not $_.Pass }).Count -gt 0
$result = if ($failed) { "KALDI" } else { "GEÇTİ" }

$info = @(
    ("Kareler: {0} kare, {1:N1} sn ({2:N1} FPS)" -f $rows.Count, $duration, $fps),
    ("Elin görüldüğü kare oranı: %{0:N0}" -f ($handShare * 100)),
    ("Takip süresi: medyan {0:N1} ms, p95 {1:N1} ms" -f (Get-Percentile $trackMs 0.5), (Get-Percentile $trackMs 0.95)),
    ("Olaylar: GRAB {0}, DROP {1}, kaçan GRAB {2}, kaçan DROP {3}, takılan bekleme {4}" -f $events.GRAB, $events.DROP, $miss.GRAB, $miss.DROP, $stuck),
    ("Test dışı olaylar: hazırlıkta {0}, test bittikten sonra (el indirilirken) {1}" -f $setup, $after)
)
$warnings = @()
if ($after -gt 0) { $warnings += ("Test bittikten sonra {0} olay: eli indirmek hareket tetikliyor" -f $after) }

if ($Title) { Write-Host "== $Title ==" -ForegroundColor Cyan }
$info | ForEach-Object { Write-Host $_ }
foreach ($c in $checks) {
    Write-Host ("{0} {1}" -f $(if ($c.Pass) { "[OK]  " } else { "[HATA]" }), $c.Label) -ForegroundColor $(if ($c.Pass) { "Green" } else { "Red" })
}
$warnings | ForEach-Object { Write-Host ("[UYARI] " + $_) -ForegroundColor Yellow }
Write-Host ("Sonuç: {0}" -f $result) -ForegroundColor $(if ($failed) { "Red" } else { "Green" })

if ($ReportPath) {
    $md = New-Object System.Collections.Generic.List[string]
    $md.Add("")
    $md.Add(("### {0} — {1}" -f $(if ($Title) { $Title } else { Split-Path $Path -Leaf }), (Get-Date -Format "yyyy-MM-dd HH:mm")))
    $md.Add("")
    $md.Add(("- **Sonuç:** {0}" -f $result))
    $md.Add(("- **Kayıt:** ``{0}``" -f (Split-Path $Path -Leaf)))
    $info | ForEach-Object { $md.Add("- " + $_) }
    foreach ($c in $checks) { $md.Add(("- {0} {1}" -f $(if ($c.Pass) { "✅" } else { "❌" }), $c.Label)) }
    $warnings | ForEach-Object { $md.Add("- ⚠️ " + $_) }
    [IO.File]::AppendAllText($ReportPath, ($md -join "`r`n") + "`r`n", (New-Object Text.UTF8Encoding $false))
}

if ($PassThru) {
    [pscustomobject]@{
        Result    = $result
        Completed = $completed
        Grab      = if ($emptyRun) { "-" } else { "{0}/{1}" -f $ok.GRAB, $attempts.GRAB }
        Drop      = if ($emptyRun) { "-" } else { "{0}/{1}" -f $ok.DROP, $attempts.DROP }
        Wrong     = if ($emptyRun) { $events.GRAB + $events.DROP } else { $wrong }
        After     = $after
        MedianMs  = if ($all.Count) { [Math]::Round($median) } else { $null }
        P95Ms     = if ($all.Count) { [Math]::Round($p95) } else { $null }
        Fps       = [Math]::Round($fps, 1)
        HandShare = [Math]::Round($handShare * 100)
    }
}

if ($failed) { exit 1 }
exit 0
