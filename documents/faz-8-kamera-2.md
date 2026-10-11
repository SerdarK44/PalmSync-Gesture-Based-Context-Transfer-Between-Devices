# Faz 8: Kamera 2.0 — Anlık ve Güvenilir El Algılama

**Durum:** Aşama 1-3 tamamlandı; Aşama 4'te kod tamamlandı, gerçek kamera ölçümü bekliyor  
**Öncelik:** Kritik  
**Başlangıç sürümü:** v0.2.3  
**Hedef:** Kameradan gelen açık el / kapalı el hareketlerini düşük gecikmeyle, farklı ışık ve ten koşullarında güvenilir biçimde algılamak.

---

## 1. Teknik Karar

Kamera 2.0 için birincil algılama motoru, MediaPipe tabanlı avuç ve el-pozu ONNX modellerini mevcut **OpenCV DNN** çalışma zamanı ile kullanacaktır.

Bu tercih ile:

- Her el için 21 eklem noktası elde edilir.
- Parmakların açık/kapalı durumu renk yerine eklem geometrisinden hesaplanır.
- Mevcut CMake, vcpkg ve Windows installer zinciri korunur.
- Uygulamaya ikinci bir Bazel/MSYS2 derleme sistemi eklenmez.
- v0.2.3'te çalışan klasik ten-rengi algılayıcı yedek motor olarak kalır.

Doğrudan MediaPipe C++ entegrasyonu şimdilik tercih edilmeyecektir; Windows desteği deneysel ve mevcut derleme zincirine göre daha yüksek bakım riski taşımaktadır.

### Kaynaklar

- [MediaPipe Hand Landmarker](https://developers.google.com/mediapipe/solutions/vision/hand_landmarker): 21 el noktası, handedness ve video/canlı akış izleme davranışı.
- [MediaPipe C++ kurulumu](https://developers.google.com/mediapipe/framework/getting_started/install): Windows üzerinde Bazel/MSYS2 gereksinimleri ve deneysel destek notu.
- [OpenCV Zoo](https://github.com/opencv/opencv_zoo): OpenCV DNN için MP-PalmDet ve MP-HandPose modelleri.

---

## 2. Mimari

```text
Kamera (latest-frame queue)
        |
        v
Detector backend seçimi
  ├─ Landmark: PalmDet -> HandPose -> 21 nokta
  └─ Classic: mevcut ten-rengi/contour algılayıcı
        |
        v
Ortak HandObservation
        |
        v
Geometrik poz sınıflandırıcı
        |
        v
Zamansal filtre + Gesture FSM
        |
        v
Brain / UDP olayı
```

Backend çalışma modları:

- `auto`: Landmark modeli hazırsa onu, değilse klasik motoru kullanır.
- `landmark`: Yalnızca yeni modeli kullanır; sorun varsa açık hata verir.
- `classic`: v0.2.3 davranışına geri döner.

---

## 3. Uygulama Adımları

### Aşama 1 — Temel altyapı ve güvenli geri dönüş

- [x] v0.2.3 çalışan sürümünü başlangıç noktası olarak sabitle.
- [x] Ortak `IHandTracker` arayüzünü oluştur.
- [x] Mevcut `HandDetector` sınıfını `classic` backend olarak bu arayüze bağla.
- [x] `auto`, `landmark`, `classic` seçimlerini komut satırı ve ayarlara ekle.
- [x] Seçilen backend'i loglarda açıkça göster.
- [x] Eski davranışın unit testlerini eksiksiz geçir.

**Çıkış kriteri:** Mimari yeni motoru kabul eder hale gelir; mevcut kamera davranışı değişmez ve geri dönüş tek ayarla mümkündür.

### Aşama 2 — Model yönetimi ve tedarik güvenliği

- [x] Resmî PalmDet ve HandPose model sürümlerini sabitle.
- [x] Model lisanslarını ve kaynak adreslerini `THIRD_PARTY_NOTICES` içinde kaydet.
- [x] SHA-256 doğrulaması yapan model indirme/paketleme adımı ekle.
- [x] Eksik veya bozuk modelde otomatik `classic` fallback uygula.
- [x] Modelleri installer'a kontrollü biçimde dahil et.

**Çıkış kriteri:** Model sürümü tekrarlanabilir, doğrulanabilir ve internet olmadan kurulumdan sonra çalışır.

### Aşama 3 — Landmark algılama motoru

- [x] OpenCV `dnn` bileşenini build'e açıkça ekle.
- [x] PalmDet ön işleme ve inference katmanını uygula (tek el için en yüksek skorlu aday).
- [x] El bölgesini döndürerek/kırparak HandPose modeline aktar.
- [x] 21 görüntü noktası, güven skoru ve sağ/sol el bilgisini üret.
- [x] Önceki kareden ROI izleyerek her karede palm detection çalıştırma.
- [x] El kaybolduğunda aynı karede hızlı yeniden arama uygula.

**Çıkış kriteri:** Kamera önündeki elin 21 noktası overlay üzerinde kararlı biçimde çizilir ve kısa örtülmelerden sonra tekrar yakalanır.

### Aşama 4 — Geometrik açık/kapalı el sınıflandırması

- [x] Her parmağın MCP/PIP/DIP açılarını hesapla.
- [x] Başparmak için ayrı geometri kullan: eklem açısına ek olarak başparmak ucunun serçe MCP'ye uzaklığı avuç genişliği biriminde ölçülür. İşaret→serçe ekseni başparmağın hangi tarafta olduğunu zaten verdiğinden ölçüm sağ/sol el ve avuç/el sırtı için aynı çalışır; aynalanan kare modelin handedness çıktısını ters çevirdiği için handedness'e güvenilmez.
- [x] Gerçek kamera doğrulaması için kare başına ölçüm kaydı (`--metrics`) ve rapor betiği (`eye/analyze-metrics.ps1`) ekle.
- [x] Ekranda yönlendirmeli test modu (`--guide`, `--guide-empty`) ve senaryo yürütücüsü (`eye/run-camera-tests.ps1`) ekle.
- [ ] Gerçek kamerada 20 tekrar, 3 mesafe ve 3 ışık ortamında kabul kriterlerini ölç (protokol aşağıda).
- [x] İlk kayıtlara göre landmark poz eşiklerini ve geçiş penceresini ayarla (eli indirme yanlış GRAB'ı).
- [ ] 20 tekrarlık oturumlarla eşikleri doğrula; gerekirse yeniden ayarla (`palmsync-eye-replay` ile kayıtlar üzerinde dene).
- [x] Açık, kapalı ve belirsiz poz skorlarını üret.
- [x] Ölçekten bağımsız eklem açıları kullan.
- [x] Histerezis ve kısa zamansal filtre ekle.
- [x] Hızlı açık→kapalı ve kapalı→açık geçişlerini FSM'e bağla.

**Çıkış kriteri:** Cilt rengi, yüz ve arka plandan bağımsız poz sınıflandırması; tek karelik sıçramalar olay üretmez.

#### Gerçek kamera doğrulama protokolü

Doğrulama rehberli test yürütücüsüyle yapılır:

```powershell
.\eye\run-camera-tests.ps1            # yeni oturum
.\eye\run-camera-tests.ps1 -Session son   # yarıda kalanı sürdür
```

Yürütücü sırasıyla ısınma (3 tekrar, sayılmaz), 3 mesafe × 3 ışık (20'şer tekrar), yüz-only (5 dk) ve boş sahne (10 dk) senaryolarını çalıştırır. Her senaryodan önce konsolda Türkçe hazırlık talimatı gösterir; kamera penceresinde (`--guide`) büyük renkli komutlar (`SIMDI: YUMRUK YAP`, `YUMRUGU TUT`, `SIMDI: ELINI AC` …), tekrar sayacı, 5 sn'lik süre çubuğu ve ne yanlış gittiğini söyleyen ipucu satırı bulunur. Rehber, FSM'in GRAB/DROP sonrası 1,4 sn'lik kilidi bitmeden bir sonraki hareketi istemez. Rehberli modda olaylar Brain'e gönderilmez.

Her kayıt `analyze-metrics.ps1` ile değerlendirilir: GRAB yalnızca ekran yumruk isterken, DROP yalnızca açık el isterken başarılı sayılır; diğer anlardaki olaylar "hatalı", süresi dolan komutlar "kaçan" sayılır. Gecikme, ham pozun yeni duruma geçtiği ilk kareden olayın üretildiği kareye kadar ölçülür (kamera pozlama/aktarım gecikmesi dahil değildir). Sonuçlar [kamera-2-test-gunlugu.md](./kamera-2-test-gunlugu.md) dosyasına satır olarak eklenir.

### Aşama 5 — Kişisel kalibrasyon (1.1 sürümüne ertelendi)

- [ ] UI'a “Kamerayı Kalibre Et” sihirbazı ekle.
- [ ] Kullanıcıdan kısa süre açık el ve yumruk örneği al.
- [ ] El boyutu, hareket aralığı ve güven eşiklerini kişiye göre ayarla.
- [ ] Profilde kamera, backend ve performans tercihlerini sakla.
- [ ] “Varsayılana dön” seçeneği ekle.

**Çıkış kriteri:** İlk kullanım kalibrasyonu 10 saniyeden kısa sürer ve ayarlar sonraki açılışta korunur.

### Aşama 6 — Performans modları

- [ ] `Hızlı`, `Dengeli`, `Tasarruflu` profilleri ekle.
- [ ] İşlem kuyruğunu en yeni kareyi tutacak şekilde sınırla; eski kareleri işlemeden atla.
- [ ] Model giriş çözünürlüğü ve palm-search sıklığını profile bağla.
- [ ] OpenCV CPU/OpenCL hedeflerini ölçerek uygun olanı otomatik seç.
- [ ] Algılama, sınıflandırma ve olay gecikmesini ayrı ayrı ölç.
- [ ] Uzun kullanımda bellek ve sıcaklık davranışını kontrol et.

**Çıkış kriteri:** Görsel gecikme birikmez; sistem yavaşlasa bile eski kare kuyruğu oluşmaz.

### Aşama 7 — Kamera tanılama ekranı

- [ ] 21 noktalı el iskeleti overlay'i ekle.
- [ ] FPS, inference süresi, toplam gecikme ve güven skorunu göster.
- [ ] “El çok uzakta”, “ışık yetersiz”, “el kadraj dışında” yönlendirmeleri ekle.
- [ ] Kamera/backend/model durumunu UI'da görünür yap.
- [ ] Tanılama logunu tek tıkla dışarı aktarma ekle.

**Çıkış kriteri:** Kullanıcı algılamanın neden başarısız olduğunu uygulama içinden anlayabilir.

### Aşama 8 — Test, paketleme ve yayın

- [ ] Kaydedilmiş videolardan deterministik replay testleri oluştur.
- [ ] Farklı ışık, mesafe, ten tonu, arka plan ve iki el senaryolarını test et.
- [ ] Yüz-only ve boş-kare false positive testleri ekle.
- [ ] Model eksik/bozuk olduğunda fallback testini ekle.
- [ ] Temiz Windows kurulumunda installer ve otomatik güncellemeyi test et.
- [ ] Beta sürümü yayınla, gerçek kullanım geri bildirimini ölç.
- [ ] Kabul kriterleri sağlanınca kararlı sürümü yayınla.

---

## 4. Ölçülebilir Kabul Kriterleri

| Ölçüm | Hedef |
|---|---:|
| Algılama → UDP olay medyan gecikmesi | ≤ 150 ms |
| Algılama → UDP olay p95 gecikmesi | ≤ 250 ms |
| Açık→kapalı başarı oranı | ≥ %95 |
| Kapalı→açık başarı oranı | ≥ %95 |
| Örtülme sonrası tekrar yakalama | ≤ 500 ms |
| Boş/yüz-only sahnede yanlış olay | 10 dakikada 0 |
| Model soğuk başlangıcı | ≤ 2 sn |
| Fast modunda CPU üst sınırı | Ortalama ≤ %60 |
| Balanced modunda CPU üst sınırı | Ortalama ≤ %35 |
| Installer boyutu artışı | ≤ 20 MB |

Başarı oranı; 0,5 m, 1 m ve 1,5 m mesafelerde, aydınlık/loş/arkadan ışıklı üç ortamda en az 20'şer tekrar üzerinden ölçülecektir.

---

## 5. Test Matrisi

| Senaryo | Beklenen sonuç |
|---|---|
| Açık el sabit | Bir kez kilitlenir, tekrar tekrar olay üretmez |
| Hızlı yumruk | En geç 250 ms içinde kapalı el olayı |
| Yumrukla kadraja giriş | El bulunur; kalibrasyon gerektirmeden takip başlar |
| Yüz kameraya yakın | El olayı üretilmez |
| El yüzün önünden geçer | Takip kısa sürede geri kazanılır |
| Loş ışık | Güven düşebilir, yanlış olay oluşmaz |
| Model dosyası yok/bozuk | `auto` modunda classic backend açılır |
| Kamera kısa süre kesilir | Uygulama çökmeksizin yeniden bağlanır |
| Uzun süre çalışma | Bellek sürekli büyümez, gecikme birikmez |

---

## 6. Sürümleme ve Geri Dönüş

- Her aşama ayrı, küçük ve test edilebilir commit olacak.
- Landmark motoru kabul kriterlerini geçene kadar varsayılan `auto` modu fallback içerecek.
- Model veya runtime hatasında uygulama tamamen durmak yerine `classic` moda geçecek.
- v0.2.3 davranışı en az ilk kararlı Kamera 2.0 sürümüne kadar korunacak.
- Beta paket, ana release'ten önce gerçek kamera üzerinde smoke test görecek.

---

## 7. İlerleme Günlüğü

| Tarih | Adım | Sonuç |
|---|---|---|
| 2026-10-11 | Yol haritası ve mimari karar | Tamamlandı |
| 2026-10-11 | Ortak `IHandTracker` arayüzü ve classic adaptörü | Tamamlandı |
| 2026-10-11 | Backend seçimi, fallback ve test altyapısı | Tamamlandı |
| 2026-10-11 | Sabit model manifesti, SHA-256 doğrulaması ve paketleme | Tamamlandı |
| 2026-10-11 | PalmDet + HandPose OpenCV DNN pipeline'ı | Resmî örnek karede 21 nokta ve açık el doğrulandı |
| 2026-10-11 | Önceki kare ROI takibi | Ortalama 6,95 ms/HandPose karesi |
| 2026-10-11 | Model eksikliği fallback smoke testi | `auto→classic` başarılı, zorunlu landmark hata kodu 4 |
| 2026-10-11 | Eklem açısı tabanlı açıklık ölçümü | Sentetik açık el `1,00`, yumruk `0,00` |
| 2026-10-11 | Başparmak uzanım geometrisi | Avuca katlanmış düz başparmak artık sayılmıyor (4 parmak, `0,80`); aynalanmış el aynı sonucu veriyor; sentetik açık el `0,95` |
| 2026-10-11 | `--metrics` kaydı ve `analyze-metrics.ps1` | Sentetik kayıtta GRAB 99 ms / DROP 165 ms doğru hesaplandı; eksik tekrar başarısız sayıldı |
| 2026-10-11 | Logitech kamera, boş sahne duman testi | 12 sn, 0 yanlış olay; boş sahnede PalmDet medyan 12,9 ms/kare, kamera ~15 FPS verdi |
| 2026-10-11 | Rehberli test modu ve yürütücü | 4 rehber birim testi geçti; %125 ölçekli ekranda pencere sığıyor; duman testinde gerçek GRAB+DROP 128 / 145 ms |
| 2026-10-11 | Gerçek kamera test oturumu | Başlatıldı — sonuçlar [kamera-2-test-gunlugu.md](./kamera-2-test-gunlugu.md) |
| 2026-10-11 | İlk oturum analizi | Yürütücü hatası yüzünden 3 tekrar (geçersiz); eli indirince GRAB ve loş ışıkta kameranın atlanması bulundu |
| 2026-10-11 | Landmark poz eşikleri | Yumruk < 0,15, açık > 0,60, geçiş penceresi 550 ms (ölçülen: gerçek 63–400 ms, el indirme 591 ms); kayıtlardan yeniden oynatma testiyle sabitlendi |
| 2026-10-11 | Loş ışıkta kamera seçimi | Monokrom eşiği parlaklığa göre ölçekleniyor; karanlık odada kamera açıldı |

