# Kamera 2.0 — Güncel Durum ve Sonraki Adımlar

**Son güncelleme:** 2026-10-11  
**Başlangıç sürümü:** v0.2.3  
**Şu anki konum:** Aşama 4'ün kod tarafı (başparmak geometrisi dahil) ve ölçüm araçları tamamlandı; sıradaki iş gerçek kamera önünde ölçüm almak.  
**Yayın durumu:** Henüz push, beta release veya kullanıcı kurulumu yapılmadı.

Detaylı teknik yol haritası: [faz-8-kamera-2.md](./faz-8-kamera-2.md)

---

## Tamamlanan Aşamalar

| Aşama | Durum | Tamamlananlar |
|---|---|---|
| **1 — Temel altyapı ve geri dönüş** | ✅ Tamamlandı | Ortak `IHandTracker`, classic backend, `auto/landmark/classic` seçimi, loglama ve fallback altyapısı |
| **2 — Model yönetimi ve tedarik güvenliği** | ✅ Tamamlandı | Sabit model sürümleri, SHA-256 kontrolü, lisans bildirimi, doğrulamalı indirme ve installer paketleme |
| **3 — Landmark algılama motoru** | ✅ Kod tamamlandı | PalmDet, HandPose, 21 nokta, güven/handedness çıktısı, döndürülmüş ROI, önceki kare takibi ve hızlı yeniden arama |
| **4 — Geometrik poz sınıflandırması** | 🚧 Kod tamamlandı, ölçüm bekliyor | Parmak eklem açıları, başparmak uzanım geometrisi, açıklık skoru, histerezis, zamansal filtre, açık↔yumruk FSM bağlantısı, `--metrics` kaydı ve rapor betiği |

### Tamamlanan doğrulamalar

- [x] 4/4 C++ test paketi başarılı.
- [x] Resmî OpenCV örnek karesinde 21 nokta doğru elin üzerine oturdu.
- [x] Örnek açık elde güven `0,995`, açıklık `1,00`, parmak sayısı `5` ölçüldü.
- [x] Sentetik açık el/yumruk geometrisi `1,00 / 0,00` verdi.
- [x] Takip modunda HandPose ortalaması `6,95 ms/kare` ölçüldü.
- [x] Gerçek Logitech kamera DirectShow ile `640×480 @ 30 FPS` açıldı.
- [x] Model varken `auto → landmark` seçimi çalıştı.
- [x] Model yokken `auto → classic` geri dönüşü çalıştı.
- [x] Zorunlu ama kullanılamayan landmark seçiminde açık hata üretildi.
- [x] Modelleri ve lisansı içeren 73,1 MB installer başarıyla derlendi.
- [x] Avuca katlanmış düz başparmak artık açık sayılmıyor; aynalanmış elde sonuç birebir aynı.
- [x] `--metrics` kaydı gerçek kamerada çalıştı; boş sahnede 12 sn boyunca 0 yanlış olay.
- [x] `analyze-metrics.ps1` sentetik kayıtta gecikmeyi (99 / 165 ms) ve eksik tekrarı doğru raporladı.

### Gözlem

- Boş sahnede (el yokken her karede PalmDet çalışırken) kamera yalnızca ~15 FPS verdi; PalmDet medyanı 12,9 ms olduğundan darboğaz büyük olasılıkla kameranın loş ortamda pozlama süresini uzatması. Aşama 6'da kamera pozlama ayarı ile birlikte ölçülmeli.

---

## Şu Anda Bitirilmesi Gereken Aşama

### Aşama 4 — Geometrik açık/kapalı el sınıflandırmasını kapatma

Bu aşama bitmeden kalibrasyon ve beta yayına geçilmeyecek.

- [x] Başparmağın açık/kapalı durumunu avuç geometrisiyle ayrıca hesapla (handedness aynalı karede ters döndüğü için kullanılmadı).
- [x] Kare başına ölçüm kaydı ve otomatik rapor aracı ekle.
- [ ] Gerçek kamerada açık→yumruk→açık dizisini en az 20 kez ölç.
- [ ] Yumrukla doğrudan kadraja girişin algılandığını doğrula.
- [ ] 0,5 m, 1 m ve 1,5 m mesafelerde test et.
- [ ] Aydınlık, loş ve arkadan ışıklı ortamları test et.
- [ ] Yanlış açık/yumruk sınıflandırmalarına göre eşikleri ayarla.
- [ ] Gerçek kullanımda medyan ve p95 olay gecikmesini ölç.

**Aşama 4 tamamlanma kriteri:** Her iki geçişte en az `%95` başarı; medyan gecikme en fazla `150 ms`, p95 en fazla `250 ms`.

---

## Sıradaki Aşamalar

### Aşama 5 — Kişisel kalibrasyon (1.1 sürümüne ertelendi)

- [ ] UI'a “Kamerayı Kalibre Et” sihirbazı ekle.
- [ ] Kullanıcıdan kısa açık el ve yumruk örnekleri al.
- [ ] Kişiye özel poz/güven eşiklerini hesapla.
- [ ] Kamera, backend ve kalibrasyon ayarlarını profilde sakla.
- [ ] Varsayılan ayarlara dönme seçeneği ekle.

**Hedef:** Kalibrasyon 10 saniyeden kısa sürecek ve sonraki açılışta korunacak.

### Aşama 6 — Performans ve gecikme

- [ ] `Hızlı`, `Dengeli` ve `Tasarruflu` çalışma profilleri ekle.
- [ ] Kamera kuyruğunda yalnızca en yeni kareyi işle.
- [ ] PalmDet çalışma sıklığını performans profiline bağla.
- [ ] CPU ve OpenCL seçeneklerini gerçek donanımda karşılaştır.
- [ ] Algılama, sınıflandırma ve UDP olay gecikmelerini ayrı ölç.
- [ ] Uzun kullanımda CPU, sıcaklık ve bellek kararlılığını test et.

**Hedef:** El takip ediliyorken 30 FPS kamera akışını gecikme biriktirmeden sürdürmek.

### Aşama 7 — Kamera tanılama ve kullanıcı yönlendirmesi

- [x] Motor tarafında 21 noktalı el iskeleti çizimi hazır.
- [ ] İskelet ve güven bilgisini uygulama önizlemesine eksiksiz taşı.
- [ ] FPS, inference süresi ve toplam gecikmeyi göster.
- [ ] “El çok uzakta”, “ışık yetersiz” ve “kadraj dışında” uyarıları ekle.
- [ ] Aktif backend ve model durumunu UI'da göster.
- [ ] Tek tıkla tanılama logu dışa aktarma ekle.

**Hedef:** Kullanıcı, algılamanın neden başarısız olduğunu uygulama içinden görebilecek.

### Aşama 8 — Replay testleri, beta ve kararlı yayın

- [ ] Kaydedilmiş kamera görüntülerinden deterministik replay testi oluştur.
- [ ] Farklı ışık, ten tonu, mesafe ve arka plan senaryolarını otomatik test et.
- [ ] Yüz-only ve boş sahnede yanlış olay testlerini ekle.
- [ ] En az 10 dakikalık kesintisiz çalışma testi yap.
- [ ] Temiz Windows kurulumunda installer ve otomatik güncellemeyi doğrula.
- [ ] Beta release yayınla ve gerçek kullanım sonuçlarını topla.
- [ ] Kabul kriterleri geçilince kararlı release yayınla.

**Hedef:** 10 dakikalık yüz-only/boş sahnede sıfır yanlış olay ve temiz kurulumdan çalışan otomatik güncelleme.

---

## Uygulama Sırası

```text
Şimdi
  ↓
Aşama 4: Gerçek kamera doğrulaması + başparmak geometrisi
  ↓
Aşama 5: Kişisel kalibrasyon
  ↓
Aşama 6: Performans profilleri ve latest-frame işleme
  ↓
Aşama 7: UI tanılama ekranı
  ↓
Aşama 8: Replay testleri → beta → kararlı release → push
```

---

## Bir Sonraki Çalışma Oturumu

Bir sonraki doğrudan iş **Aşama 4 gerçek kamera doğrulaması** ve şu anda yürütülüyor.

1. Proje kökünde `.\eye\run-camera-tests.ps1` çalıştır (yarıda kaldıysa `-Session son`).
2. Her senaryodan önce konsoldaki hazırlık talimatını uygula; kamera penceresindeki büyük komutları takip et.
3. Sonuçlar otomatik olarak [kamera-2-test-gunlugu.md](./kamera-2-test-gunlugu.md) ve `test-results/kamera2/<oturum>/rapor.md` dosyalarına yazılır.
4. Kalan senaryolarda CSV'deki `openness`, `fingers`, `raw_pose`, `cue` sütunlarından hatalı kareler incelenir ve eşikler ayarlanır.
5. Tüm senaryolar geçince Aşama 4 kapatılır ve Aşama 5'e (kalibrasyon) geçilir.
