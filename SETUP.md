# PalmSync — Kurulum ve Kullanım

İki bilgisayar arasında çalışma alanını (açık dosya + pano) el hareketiyle taşı.
İki makine **aynı yerel ağda** (aynı Wi-Fi / aynı modem) olmalı.

---

## Kurulum (her iki bilgisayar)

1. `PalmSync-Setup-0.1.0.exe`'yi çalıştır (yönetici izni ister — **Evet** de).
2. İleri, İleri, Kur. Bu kadar.
3. Kurulum bitince PalmSync açılır ve saat yanındaki tepside (🖐 mavi "P")
   simgesi belirir. Bilgisayarı her açtığında otomatik başlar.

> **.NET 8 Desktop Runtime** kurulumun içinde gömülü gelir; yoksa arkada
> sessizce kurulur. İnternet veya ek indirme **gerekmez**. (Bu yüzden setup
> ~66 MB.)

**Hiçbir IP, port, master/slave ayarı yok.** Her şey uygulama içinde.

---

## Kullanım

### 1. Bilgisayar A — Oturumu başlat

1. PalmSync penceresinde **Oturum başlat** butonuna tıkla.
2. Ekranda bir **kod** (örn. `R2M02-51354`) ve bir **şifre** (örn. `K7PQM3XR`)
   çıkar. Yanlarındaki **Copy** ile kopyalayabilirsin.
3. Bu kod ile şifreyi diğer bilgisayara ilet (yaz, oku, mesajla).

### 2. Bilgisayar B — Bağlan

1. PalmSync penceresinde **Oturuma katıl** butonuna tıkla.
2. A'daki **kodu** ve **şifreyi** yaz, **Bağlan** de.
3. "Joined the session ✓" görürsün. A'da durum "Other device connected ✓" olur.

### 3. Çalışma alanını taşı

Artık her iki pencerede iki büyük buton var:

- **✊ Yakala** — o bilgisayarda öndeki uygulamayı + panoyu yakalar.
- **🖐 Geri yükle** — yakalananı o bilgisayarda açar.

**Neler taşınır:** öndeki **her** uygulama. Yakalama anında şunlar alınır:
uygulamanın kendisi, açık belgesi (bulunabilirse içeriğiyle birlikte),
tarayıcıdaki sayfa adresi, Dosya Gezgini'ndeki klasör, pencerenin ekran
görüntüsü ve pano.

**B'de nasıl açılır:**

| A'da öndeki | B'de |
|---|---|
| Aynı uygulama B'de kurulu | Belge/sayfa **aynı uygulamada** açılır |
| Tarayıcı (ör. Chrome), B'de yok | Sayfa B'nin **varsayılan tarayıcısında** açılır (ör. Brave) |
| Metin/kod düzenleyici (ör. Sublime Text), B'de yok | Dosya B'deki ilk düzenleyicide açılır: VS Code → Cursor → VSCodium → Sublime Text → Notepad++ → **Not Defteri** (her zaman var) |
| Belge uygulaması (ör. Word), B'de yok | Dosya, B'de o dosya türünü açan programla açılır (ör. LibreOffice) |
| Dosya Gezgini klasörü | Aynı klasör B'de varsa açılır |
| Uygulama B'de hiç yok, açacak program da yok | **Ekran görüntüsü** açılır ve "Uygulama bulunamadı: …" uyarısı çıkar; dosya varsa `Gelen Dosyalar`a kaydedilip Gezgin'de gösterilir |
| Belgesi olmayan uygulama (Spotify, Discord…), B'de kurulu | Uygulama başlatılır |

Gelen dosyalar `Belgeler\PalmSync\Gelen Dosyalar` altına yazılır ve "başka
bilgisayardan geldi" işareti alır (Office bunları Korumalı Görünüm'de açar).
Komut çalıştıran dosya türleri (`.bat`, `.ps1`, `.py`, `.exe`…) **asla
çalıştırılmaz**, sadece bir düzenleyicide metin olarak açılır.

**Örnek:** A'da Sublime Text'te `notlar.txt` açıkken **Yakala** de → B'de
**Geri yükle** de → B'de Sublime Text varsa orada, yoksa VS Code'da (o da yoksa
Not Defteri'nde) aynı içerikle açılır; pano da taşınır.

> Not: Yakala, o an **önde olan** pencereyi yakalar. Tarayıcı sekmesini
> taşımak için Yakala'ya basmadan önce tarayıcı penceresi önde olmalı.

> İnternete/buluta hiçbir şey gitmez; her şey iki makine arasında şifreli
> (AES-256) olarak yerel ağda akar.

---

## Kamera ile el hareketi

Butona basmak yerine gerçek el hareketiyle de tetikleyebilirsin. Algılama
kurulumla gelen MediaPipe el modelleriyle (Kamera 2.0) çalışır: her karede
elin 21 eklem noktası bulunur, açık el / yumruk parmak açılarından hesaplanır —
cilt rengine, yüze ve arka plana bağlı değildir.

1. Oturuma başladıktan/bağlandıktan sonra PalmSync penceresinde
   **📷 El hareketleri** kartındaki **Aç** butonuna bas
   (oturum başlayınca kamera kendiliğinden de açılmaya çalışır).
2. Webcam açılır ve canlı önizlemede elinin iskeleti çizilir.
3. Hareketler:
   - **Yakala (GRAB)**: avucun kameraya dönük **açık elini** göster, sonra
     tek hareketle **yumruk yap**.
   - **Geri yükle (DROP)**: **yumruğunu** göster, sonra tek hareketle **elini aç**.
   - Bir hareketten sonra ~1,5 sn bekle (önizlemede "(wait)" yazar); bu sürede
     yeni hareket algılanmaz. Böylece yumruktan sonra eli açmak aynı
     bilgisayarda yanlışlıkla geri yükleme yapmaz.
   - Elini yavaşça indirmek ya da çevirmek hareket sayılmaz; hareket kısa ve
     net olmalı.
4. Kapatmak için aynı karttaki **Kapat** butonuna bas.

Modeller eksik ya da bozuksa kamera eski (ten rengi) algılayıcıyla çalışmaya
devam eder; **Sistem kontrolü** bunu "Kamera 2.0 el modelleri" satırında söyler.

Aktarılan dosya: Yakala anında açık belgenin **içeriği** de gönderilir (metin
en fazla 2 MB, diğer dosyalar en fazla 8 MB). Düzenleyicilerde kaydedilmemiş
değişiklikler, pencere öndeyse önce Ctrl+S ile kaydedilir. Karşı bilgisayarda
aynı dosya aynı içerikle varsa o açılır; yoksa
`Belgeler\PalmSync\Gelen Dosyalar\...` altına yazılıp açılır.

İpuçları: iyi ışık algılamayı iyileştirir; çok karanlıkta el bulunamayabilir.

Kamera olmadan da uygulama tam çalışır (yukarıdaki Yakala / Geri yükle butonları).

---

## Güncellemeler

PalmSync açılışta ve 6 saatte bir GitHub'daki proje sayfasının **Releases**
bölümüne bakar. Yeni sürüm varsa ana pencerenin üstünde **"⬆ Güncelleme var —
Şimdi yükle"** şeridi çıkar ve bildirim gelir. "Şimdi yükle" sürümü indirir,
yayınlanan SHA-256 özetiyle (ve imzasıyla) doğrular, kurar ve PalmSync'i
kaldığı oturumla yeniden açar (Windows bir kez yönetici izni sorar).

**⚙ Ayarlar → Hakkında → Güncellemeler:** *sor* (varsayılan), *otomatik indir ve
kur* ya da *denetleme*; **↻ Şimdi denetle** ile elle kontrol.

### Yeni sürüm yayınlamak (geliştirici)

1. Değişiklikleri commit et.
2. `.\release.ps1` (bir sonraki yama sürümü) ya da `.\release.ps1 -Version 0.3.0`.
   Betik sürümü `VERSION` dosyasına yazar, her şeyi derleyip imzalar,
   `dist\PalmSync-Setup-<sürüm>.exe` + `.sha256` üretir, `v<sürüm>` etiketini
   GitHub'a gönderir ve Release'i dosyalarla birlikte oluşturur. GitHub girişi
   için git'in kayıtlı hesabını (ya da `GITHUB_TOKEN`) kullanır.
3. Kurulu tüm PalmSync'ler yeni sürümü birkaç saat içinde (ya da bir sonraki
   açılışta) görür. Sadece setup üretmek için: `.\release.ps1 -DryRun`.

---

## Sistem kontrolü ve izinler (🔧)

Kurulumdan sonra PalmSync ilk açıldığında **Sistem kontrolü** penceresi
kendiliğinden açılır; daha sonra ana penceredeki **🔧** (sorun varsa **⚠ N**)
ya da tepsi menüsünden açılabilir. Şunları kontrol eder:

- **Dosyalar:** program dosyaları ve DLL'ler eksik mi, Visual C++ çalışma zamanı
  var mı, daemon ve kamera modülü gerçekten çalışabiliyor mu (Windows/antivirüs
  engeli, eksik DLL), Windows medya bileşenleri (N sürümleri).
- **Kamera:** takılı mı / devre dışı mı / sürücü hatası var mı, Windows gizlilik
  izni, kamerayı şu an başka bir uygulama (Zoom, Teams…) kullanıyor mu ve kamera
  gerçekten açılıp görüntü veriyor mu.
- **Ağ ve izinler:** ağ bağlantısı, güvenlik duvarı kuralı, portlar, PalmSync
  sertifikası, Akıllı Uygulama Denetimi, bildirimler, Windows ile başlatma.
- **Sistem:** ayar dosyası bozuk mu, Belgeler klasörlerine yazılabiliyor mu,
  VS Code komut satırı.

**🔧 Hepsini düzelt** düzeltilebilen her şeyi düzeltir. Yönetici gerektirenler
(güvenlik duvarı, sertifika, Visual C++ kurulumu, kamerayı etkinleştirme…) için
Windows **tek bir kez** onay ister. Düzeltilemeyenler için ne yapılacağı yazar.

Bir oturum başlayınca ya da bir oturuma katılınca, düzeltilemeyen sorunlar ve
kamera hataları **ev sahibi bilgisayarın** `cihaz-raporlari.log` dosyasına
gönderilir (bağlantı yokken sırada bekler). Yani diğer bilgisayarda kamera
açılmıyorsa ev sahibi bunu kendi Kayıtlar penceresinde **📡 [BİLGİSAYAR-ADI]**
satırı olarak görür.

**🐞 Hata ayıklama modu** (⚙ Ayarlar): her şeyi ayrıntılı loglar (kameranın el
algılama istatistikleri dahil), açılışta kamerayı da test eder ve sistem
kontrolünün tam raporunu ev sahibine gönderir.

---

## Kayıtlar (loglar)

Ana penceredeki **📜** (ya da tepsi menüsü → *Kayıtlar*) tüm bileşenlerin
kayıtlarını tek pencerede, canlı olarak gösterir:

- **⭐ Önemli olaylar**: yakalama, geri yükleme, bağlantı, kamera ve hatalar — sade Türkçe.
- **📜 Tüm kayıtlar**: her teknik satır; kaynak (Daemon / Ağ / Kamera / Arayüz /
  Güvenlik), seviye ve metin araması (Ctrl+F) ile filtrelenir. Bir satıra
  tıklayınca tam metni alttaki panelde görünür.
- **📦 Dışa aktar**: tüm kayıt dosyaları + sorun giderme raporu + ayarlar
  (şifre gizlenmiş) tek bir `.zip` olur — hata bildirirken bunu ekle.
- Ana ekrandaki **Son olaylar** kutusu son 5 önemli olayı gösterir.

Dosyalar `Belgeler\PalmSync\Loglar` altında (klasördeki `BENİOKU.txt` hepsini açıklar): `palmsync-brain.log` (daemon),
`palmsync-eye.log` (kamera), `palmsync-ui.log` (arayüz), `audit.log` (güvenlik),
`cihaz-raporlari.log` (ev sahibinde: oturumdaki TÜM bilgisayarların bildirdiği sorunlar),
`sistem-kontrolu\` (her sistem kontrolünün raporu). Gelen dosyalar `Belgeler\PalmSync\Gelen Dosyalar` altına kaydedilir.
Hepsi aynı formattadır ve boyut sınırına gelince kendiliğinden döner
(`.1`, `.2`…), yani diski doldurmaz. Daha ayrıntılı kayıt için
**⚙ Ayarlar → Kayıt seviyesi → debug**.

---

## Sorun giderme

| Belirti | Çözüm |
|---|---|
| B "Connecting..." de takılı | Aynı Wi-Fi/ağda mı? Kod/şifre A'daki ile birebir aynı mı? |
| Hâlâ bağlanmıyor | A'nın host ekranındaki "This computer" IP'sine bak — `192.168.x` gibi normal bir yerel IP mi? Değilse A birden fazla ağa bağlı olabilir. |
| Bağlanmıyor (aynı ağ, doğru kod) | Router'da "AP/Client Isolation" (istemci yalıtımı) kapalı olmalı — bazı modemler cihazların birbirini görmesini engeller. |
| Uygulama açılmıyor | Kurulum .NET 8'i otomatik kurar; yine de olmazsa bilgisayarı yeniden başlat. |
| "Akıllı Uygulama Denetimi engelledi" | Aşağıdaki "İmza / Smart App Control" bölümüne bak. |
| Geri yükle beklenenden farklı programla açtı | Kaynak uygulama B'de kurulu değil; PalmSync eşdeğerini seçti. Bildirim hangi programın kullanıldığını yazar. **Sistem kontrolü → "Aktarım için uygulamalar"** B'de nelerin bulunduğunu gösterir. |
| "Uygulama bulunamadı — ekran görüntüsü açıldı" | O uygulama B'de yok ve içeriği açacak başka program da yok. Uygulamayı B'ye kur ya da ekran görüntüsüyle devam et. |
| 📷 kamera butonu gri / "Kamera modülü bulunamadı" | Kurulum sırasında kamera bileşeni atlanmış. PalmSync'i kaldırıp yeniden kur. |
| 📷 "Let desktop apps access your camera must be ON" | **Windows Ayarları → Gizlilik ve güvenlik → Kamera → "Masaüstü uygulamalarının kameranıza erişmesine izin verin" seçeneğini AÇ.** |
| 📷 "Windows Defender blocked it" | Yönetici PowerShell'de `installer\trust-cert.ps1` çalıştır (kamera exe'si imzalanmalı). |
| 📷 Kamera açılıyor ama el algılanmıyor | Işığı artır, elini 0,5–1,5 m mesafede tut; Sistem kontrolünde "Kamera 2.0 el modelleri" satırı yeşil olmalı. |

---

## İmza / Smart App Control

Binary'ler **kendinden imzalı** bir sertifikayla imzalıdır (bilinmeyen yayıncı
uyarısını azaltır). İki durum var:

- **Normal Windows / SmartScreen:** Kurulumu yönetici olarak çalıştırıp her
  makinede (yönetici PowerShell) `installer\trust-cert.ps1` çalıştırırsan
  sertifika güvenilir olur ve uyarı çıkmaz.
- **Smart App Control (Akıllı Uygulama Denetimi) AÇIK ise:** Kendinden imzalı
  sertifika SAC için **yeterli değil** (SAC yalnızca itibar/EV imzalı uygulamalara
  güvenir). SAC'yi kapatman gerekir:
  **Windows Güvenliği → Uygulama ve tarayıcı denetimi → Akıllı Uygulama Denetimi
  → Kapalı.** ⚠️ Bir kez kapatınca Windows'u temiz kurmadan tekrar açamazsın.

**Oturumu bitir:** PalmSync penceresinde **End Session**.
**Kaldır:** Ayarlar → Uygulamalar → PalmSync → Kaldır.

Detaylı log: `%LOCALAPPDATA%\PalmSync\logs\palmsync-brain.log`
