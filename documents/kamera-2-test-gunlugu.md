# Kamera 2.0 — Test Günlüğü

Faz 8 / Aşama 4 gerçek kamera doğrulamasının kalıcı kaydı. Satırları `eye/run-camera-tests.ps1` her senaryodan sonra otomatik ekler. Ham kayıtlar (CSV, eye logu, ayrıntılı `rapor.md`) git'e girmeyen `test-results/kamera2/<oturum>/` klasöründe durur.

**Nasıl başlatılır:** proje kökünde `.\eye\run-camera-tests.ps1` (yarıda kalan oturum için `-Session son`, tek senaryo için `-Only 4`).

**Kabul kriterleri** ([faz-8-kamera-2.md](./faz-8-kamera-2.md)): GRAB ve DROP başarısı ≥ %95, yanlış zamanda olay 0, medyan gecikme ≤ 150 ms, p95 ≤ 250 ms; yüz-only ve boş sahnede yanlış olay 0.

Sütunlar: GRAB/DROP = başarılı/deneme, Hatalı = istenmeyen anda üretilen olay, El % = elin göründüğü kare oranı.

| Tarih | Oturum | Senaryo | Sonuç | GRAB | DROP | Hatalı | Medyan ms | p95 ms | FPS | El % |
|---|---|---|---|---|---|---|---|---|---|---|
| 2026-10-11 19:21 | 2026-10-11_1919 | 1 m — aydınlık | GEÇTİ | 3/3 | 3/3 | 0 | 128 | 144 | 15,1 | 72 |
| 2026-10-11 19:21 | 2026-10-11_1919 | 0,5 m — aydınlık | KALDI | 3/3 | 3/3 | 3 | 144 | 208 | 15,1 | 88 |
| 2026-10-11 19:22 | 2026-10-11_1919 | 1,5 m — aydınlık | KALDI | 3/3 | 3/3 | 2 | 128 | 144 | 15 | 76 |

> **2026-10-11_1919 oturumu geçersiz (deneme):** yürütücüdeki bir hata (`$reps` / `-Reps` büyük-küçük harf çakışması) yüzünden senaryolar 20 yerine 3 tekrarla koştu. "Hatalı" sütunundaki olayların tamamı geri sayım sırasında ya da test bittikten sonra oluşmuştu; yeni analizle 0,5 m ve 1,5 m de geçiyor. Bulgular:
> - Test bittikten sonra eli indirirken GRAB tetikleniyor (0,5 m ve 1,5 m'de birer kez) — gerçek kullanımda yanlış yakalama riski, düzeltilecek.
> - Loş ışıkta renkli kamera IR/monokrom sanılıp atlanıyordu; "1 m — loş" kayıt üretemedi. Kamera seçimi düzeltildi.
>
> Geçerli ölçüm için yeni oturum açılmalı.
