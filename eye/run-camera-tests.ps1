# PalmSync Eye - Camera 2.0 guided validation session (Faz 8 / Asama 4)
#
#   .\eye\run-camera-tests.ps1                  # new session
#   .\eye\run-camera-tests.ps1 -Session son     # resume the latest session
#   .\eye\run-camera-tests.ps1 -Only 1,4        # run selected scenarios
#
# Every scenario opens the eye window with big on-screen cues (--guide),
# records a per-frame CSV and appends the analysis to:
#   test-results\kamera2\<session>\rapor.md     full per-scenario report
#   test-results\kamera2\<session>\durum.csv    one row per scenario (resume)
#   documents\kamera-2-test-gunlugu.md          cumulative log in the repo
# Gesture events are NOT sent to the Brain during guided runs.
#
# Saved as UTF-8 with BOM so Windows PowerShell 5.1 shows the Turkish text.

param(
    [string]$Session = "",
    [int[]]$Only = @(),
    [int]$Reps = 20,
    [int]$FaceSeconds = 300,
    [int]$EmptySeconds = 600,
    [string]$Detector = "landmark",
    [int]$Camera = -1,
    [string]$EyeExe = "$env:USERPROFILE\.palmsync-build\eye\Release\palmsync-eye.exe"
)

$ErrorActionPreference = "Stop"
$eyeDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$repoDir = Split-Path -Parent $eyeDir
$analyzer = Join-Path $eyeDir "analyze-metrics.ps1"
$journal = Join-Path $repoDir "documents\kamera-2-test-gunlugu.md"
$utf8 = New-Object Text.UTF8Encoding $false

function Say([string]$text, [string]$color = "Gray") { Write-Host $text -ForegroundColor $color }

# ── Scenarios ───────────────────────────────────────────────
$light = @{
    aydinlik = "Odanın ışıkları açık olsun; elin ve yüzün net aydınlansın. Arkanda pencere olmasın."
    los      = "Tavan ışığını kapat; yalnızca ekran ışığı ya da uzaktaki bir masa lambası kalsın."
    arka     = "Arkanda açık bir pencere veya güçlü bir lamba olsun (ışık kameraya doğru gelsin)."
}
$distance = @{
    "0.5" = "Elin kameradan yaklaşık 0,5 m uzakta olsun (dirsek bükülü, el ekrana yakın)."
    "1"   = "Elin kameradan yaklaşık 1 m uzakta olsun (normal oturma mesafesi, kol uzatılmış)."
    "1.5" = "Elin kameradan yaklaşık 1,5 m uzakta olsun (sandalyeyi geri çek ya da ayağa kalk)."
}
$scenarios = @(
    @{ Id = 0;  Kind = "warmup"; Title = "Isinma (sayilmaz)";  Name = "Isınma — akışı tanımak için 3 tekrar, sonuçlara sayılmaz"; Dist = "1";   Light = "aydinlik" }
    @{ Id = 1;  Kind = "gesture"; Title = "1 m - aydinlik";     Name = "1 m — aydınlık";        Dist = "1";   Light = "aydinlik" }
    @{ Id = 2;  Kind = "gesture"; Title = "0.5 m - aydinlik";   Name = "0,5 m — aydınlık";      Dist = "0.5"; Light = "aydinlik" }
    @{ Id = 3;  Kind = "gesture"; Title = "1.5 m - aydinlik";   Name = "1,5 m — aydınlık";      Dist = "1.5"; Light = "aydinlik" }
    @{ Id = 4;  Kind = "gesture"; Title = "1 m - los";          Name = "1 m — loş";             Dist = "1";   Light = "los" }
    @{ Id = 5;  Kind = "gesture"; Title = "0.5 m - los";        Name = "0,5 m — loş";           Dist = "0.5"; Light = "los" }
    @{ Id = 6;  Kind = "gesture"; Title = "1.5 m - los";        Name = "1,5 m — loş";           Dist = "1.5"; Light = "los" }
    @{ Id = 7;  Kind = "gesture"; Title = "1 m - arkadan isik"; Name = "1 m — arkadan ışıklı";  Dist = "1";   Light = "arka" }
    @{ Id = 8;  Kind = "gesture"; Title = "0.5 m - arkadan isik"; Name = "0,5 m — arkadan ışıklı"; Dist = "0.5"; Light = "arka" }
    @{ Id = 9;  Kind = "gesture"; Title = "1.5 m - arkadan isik"; Name = "1,5 m — arkadan ışıklı"; Dist = "1.5"; Light = "arka" }
    @{ Id = 10; Kind = "face";    Title = "Yuz yakin, el yok";  Name = "Yüz kameraya yakın, el yok" }
    @{ Id = 11; Kind = "empty";   Title = "Bos sahne";          Name = "Boş sahne — kamerada kimse yok" }
)

$gestureGuide = @"
  Ekrandaki büyük yazıyı takip et:
    ELINI AC VE SABIT TUT  (yeşil)   avucun kameraya dönük, parmaklar açık
    SIMDI: YUMRUK YAP      (turuncu) tek hareketle yumruk yap
    YUMRUGU TUT            (sarı)    yazı değişene kadar yumruğu açma (~1,5 sn)
    SIMDI: ELINI AC        (yeşil)   tek hareketle elini aç
    ELINI ACIK TUT         (sarı)    yazı değişene kadar açık tut
  - Yazı çıkar çıkmaz hareket et; eli kadraj içinde kaydırma, sadece aç/kapat.
  - Alttaki çizgi 5 sn'de dolar; dolarsa o deneme 'kaçtı' sayılır. Sorun değil, devam et.
  - Alt satırdaki kırmızı uyarı neyin yanlış olduğunu söyler (ör. 'ELIN GORUNMUYOR').
  - Esc: senaryoyu yarıda bırakır.
"@

# ── Session folder ──────────────────────────────────────────
$root = Join-Path $repoDir "test-results\kamera2"
New-Item -ItemType Directory -Force $root | Out-Null
if ($Session -eq "son") {
    $latest = Get-ChildItem $root -Directory | Sort-Object Name | Select-Object -Last 1
    if (-not $latest) { throw "Devam edilecek oturum bulunamadı: $root" }
    $Session = $latest.Name
}
if (-not $Session) { $Session = Get-Date -Format "yyyy-MM-dd_HHmm" }
$sessionDir = Join-Path $root $Session
New-Item -ItemType Directory -Force $sessionDir | Out-Null
$report = Join-Path $sessionDir "rapor.md"
$state = Join-Path $sessionDir "durum.csv"
if (-not (Test-Path $report)) {
    [IO.File]::WriteAllText($report, "# Kamera 2.0 test oturumu $Session`r`n`r`nAlgılayıcı: $Detector, tekrar: $Reps`r`n", $utf8)
}
$done = @{}
if (Test-Path $state) {
    foreach ($row in Import-Csv $state) { if ($row.Sonuc -ne "YARIDA") { $done[[int]$row.Id] = $row.Sonuc } }
}

if (-not (Test-Path $journal)) {
    [IO.File]::WriteAllText($journal, "# Kamera 2.0 — Test Günlüğü`r`n`r`n| Tarih | Oturum | Senaryo | Sonuç | GRAB | DROP | Hatalı | Medyan ms | p95 ms | FPS | El % |`r`n|---|---|---|---|---|---|---|---|---|---|---|`r`n", $utf8)
}

# ── Pre-flight ──────────────────────────────────────────────
Clear-Host
Say "PalmSync — Kamera 2.0 rehberli test oturumu" "Cyan"
Say "Oturum klasörü: $sessionDir"
if (-not (Test-Path $EyeExe)) { throw "palmsync-eye.exe bulunamadı: $EyeExe  (önce .\eye\build.ps1 çalıştır)" }
$busy = @(Get-Process -Name "palmsync-eye", "PalmSync*" -ErrorAction SilentlyContinue)
if ($busy.Count -gt 0) {
    Say "UYARI: Çalışan PalmSync süreçleri var: $(($busy | ForEach-Object { $_.ProcessName }) -join ', ')" "Yellow"
    Say "Kamera başka bir süreçte açıksa test açılamaz. Tepsi uygulamasından PalmSync'i kapatman önerilir." "Yellow"
}
Say ""
Say "Senaryolar:" "Cyan"
foreach ($s in $scenarios) {
    $mark = if ($done.ContainsKey($s.Id)) { "[$($done[$s.Id])]" } else { "" }
    Say ("  {0,2}. {1} {2}" -f $s.Id, $s.Name, $mark)
}
Say ""
Say "Her senaryodan önce hazırlık talimatı gösterilir; sen Enter'a basınca kamera penceresi açılır." "Gray"
Say "Gerçek GRAB/DROP olayları bu testte Brain'e GÖNDERİLMEZ; çalışma alanın etkilenmez." "Gray"

# ── Run ─────────────────────────────────────────────────────
$results = New-Object System.Collections.Generic.List[object]
foreach ($s in $scenarios) {
    if ($Only.Count -gt 0 -and $Only -notcontains $s.Id) { continue }
    if ($Only.Count -eq 0 -and $done.ContainsKey($s.Id)) { continue }

    $attempt = 1
    while ($true) {
        Say ""
        Say ("═══ Senaryo {0}: {1} ═══" -f $s.Id, $s.Name) "Cyan"
        switch ($s.Kind) {
            { $_ -in "gesture", "warmup" } {
                # Not "$reps": PowerShell names are case-insensitive, so that
                # would overwrite the -Reps parameter after the warm-up.
                $repCount = if ($s.Kind -eq "warmup") { 3 } else { $Reps }
                Say ("  Işık   : " + $light[$s.Light])
                Say ("  Mesafe : " + $distance[$s.Dist])
                Say ("  Tekrar : $repCount kez açık el → yumruk → açık el (yaklaşık {0} dk)" -f [Math]::Ceiling($repCount * 5 / 60))
                Say $gestureGuide "Gray"
                $guideArgs = "--guide $repCount"
            }
            "face" {
                Say "  Elini kameraya GÖSTERME. Yüzün kameraya yakın (40-60 cm) olsun; başını çevir,"
                Say "  konuş, ekrana bak; normal çalışır gibi davran. Ellerin klavyede/masada kalabilir"
                Say ("  ama kameraya kaldırma. Süre: {0}:{1:00} dk. Ekranda geri sayım görünür." -f [int][Math]::Floor($FaceSeconds / 60), ($FaceSeconds % 60))
                $guideArgs = "--guide-empty $FaceSeconds"
            }
            "empty" {
                Say "  Kameranın görüş alanından çık (ya da kamerayı boş bir duvara çevir)."
                Say ("  Süre: {0}:{1:00} dk. Pencere süre bitince kendiliğinden kapanır." -f [int][Math]::Floor($EmptySeconds / 60), ($EmptySeconds % 60))
                $guideArgs = "--guide-empty $EmptySeconds"
            }
        }
        $answer = Read-Host "  [Enter] başlat   [a] atla   [q] oturumu bitir"
        if ($answer -eq "q") { break }
        if ($answer -eq "a") { Say "  Atlandı." "DarkGray"; break }

        $slug = "{0:00}-{1}" -f $s.Id, (($s.Title.ToLowerInvariant() -replace '[^a-z0-9]+', '-').Trim('-'))
        if ($attempt -gt 1) { $slug += "-deneme$attempt" }
        $csv = Join-Path $sessionDir "$slug.csv"
        $argLine = "$guideArgs --detector $Detector --guide-title `"$($s.Title)`" --metrics `"$csv`" --log-dir `"$sessionDir`" --port 45999"
        if ($Camera -ge 0) { $argLine += " --camera $Camera" }

        Say "  Kamera penceresi açılıyor..." "Gray"
        $proc = Start-Process -FilePath $EyeExe -ArgumentList $argLine -PassThru -Wait `
            -RedirectStandardOutput (Join-Path $sessionDir "$slug.stdout.txt") `
            -RedirectStandardError (Join-Path $sessionDir "$slug.stderr.txt")
        if (-not (Test-Path $csv) -or (Get-Content $csv | Measure-Object -Line).Lines -lt 3) {
            Say "  Kamera açılamadı ya da kayıt boş (çıkış kodu $($proc.ExitCode))." "Red"
            Say "  Ayrıntı: $(Join-Path $sessionDir 'palmsync-eye.log')" "Red"
            $r = $null
        } else {
            Say ""
            $r = & $analyzer -Path $csv -Title ("Senaryo {0}: {1}" -f $s.Id, $s.Name) -ReportPath $report -PassThru
        }

        if ($r) {
            $verdict = if (-not $r.Completed) { "YARIDA" } elseif ($s.Kind -eq "warmup") { "DENEME" } else { $r.Result }
            $row = [pscustomobject]@{
                Id = $s.Id; Senaryo = $s.Name; Sonuc = $verdict; GRAB = $r.Grab; DROP = $r.Drop
                Hatali = $r.Wrong; MedyanMs = $r.MedianMs; P95Ms = $r.P95Ms; FPS = $r.Fps
                ElYuzde = $r.HandShare; Zaman = (Get-Date -Format "yyyy-MM-dd HH:mm"); Kayit = "$slug.csv"
            }
            $row | Export-Csv -Path $state -Append -NoTypeInformation -Encoding UTF8
            $results.Add($row)
            if ($s.Kind -ne "warmup") {
                $wrongCell = if ($r.After -gt 0) { "$($r.Wrong) (+$($r.After) bitişte)" } else { "$($r.Wrong)" }
                $line = "| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} | {9} | {10} |`r`n" -f `
                    $row.Zaman, $Session, $s.Name, $verdict, $r.Grab, $r.Drop, $wrongCell, $r.MedianMs, $r.P95Ms, $r.Fps, $r.HandShare
                [IO.File]::AppendAllText($journal, $line, $utf8)
            }
        }

        $next = Read-Host "  [Enter] sonraki senaryo   [t] bu senaryoyu tekrarla   [q] oturumu bitir"
        if ($next -eq "t") { $attempt++; continue }
        if ($next -eq "q") { $answer = "q" }
        break
    }
    if ($answer -eq "q") { break }
}

# ── Summary ─────────────────────────────────────────────────
Say ""
Say "═══ Oturum özeti ═══" "Cyan"
if ($results.Count -gt 0) {
    $results | Format-Table Id, Senaryo, Sonuc, GRAB, DROP, Hatali, MedyanMs, P95Ms, FPS, ElYuzde -AutoSize | Out-String | Write-Host
} else {
    Say "Bu çalıştırmada senaryo tamamlanmadı."
}
Say "Rapor      : $report"
Say "Günlük     : $journal"
Say "Devam etmek için: .\eye\run-camera-tests.ps1 -Session $Session"
