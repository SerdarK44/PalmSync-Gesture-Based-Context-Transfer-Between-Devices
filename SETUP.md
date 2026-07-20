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

1. PalmSync penceresinde **Start Session** (Oturumu Başlat) tıkla.
2. Ekranda bir **kod** (örn. `R2M02-51354`) ve bir **şifre** (örn. `K7PQM3XR`)
   çıkar. Yanlarındaki **Copy** ile kopyalayabilirsin.
3. Bu kod ile şifreyi diğer bilgisayara ilet (yaz, oku, mesajla).

### 2. Bilgisayar B — Bağlan

1. PalmSync penceresinde **Connect** (Bağlan) tıkla.
2. A'daki **kodu** ve **şifreyi** yaz, **Connect** de.
3. "Joined the session ✓" görürsün. A'da durum "Other device connected ✓" olur.

### 3. Çalışma alanını taşı

Artık her iki pencerede iki büyük buton var:

- **✊ Capture workspace** — o bilgisayardaki açık dosyayı + panoyu yakalar.
- **🖐 Restore workspace** — yakalanan dosyayı o bilgisayarda açar.

**Örnek:** Bilgisayar A'da VS Code'da bir dosya açıkken **Capture** de →
Bilgisayar B'de **Restore** de → aynı dosya B'de açılır, pano da taşınır.

> İnternete/buluta hiçbir şey gitmez; her şey iki makine arasında şifreli
> (AES-256) olarak yerel ağda akar.

---

## Kamera ile el hareketi

Butona basmak yerine gerçek el hareketiyle de tetikleyebilirsin — **model
indirmek gerekmez**, tamamen yerleşik:

1. Oturuma başladıktan/bağlandıktan sonra PalmSync penceresinde
   **📷 Use hand gestures (camera)** butonuna bas.
2. Webcam açılır, küçük bir önizleme penceresi çıkar (elini gösterir, "OPEN /
   FIST" ve açıklık değerini yazar).
3. **Yumruk yap** = Capture (GRAB), **elini aç** = Restore (DROP).
4. Kapatmak için aynı butona tekrar bas (artık "Stop camera gestures").

İpuçları: elini kameraya yakın ve düz tut; arkanda ten rengine yakın büyük
yüzeyler olmasın (algılama cilt rengine dayanır); iyi ışık algılamayı iyileştirir.

Kamera olmadan da uygulama tam çalışır (yukarıdaki Capture/Restore butonları).

---

## Sorun giderme

| Belirti | Çözüm |
|---|---|
| B "Connecting..." de takılı | Aynı Wi-Fi/ağda mı? Kod/şifre A'daki ile birebir aynı mı? |
| Hâlâ bağlanmıyor | A'nın host ekranındaki "This computer" IP'sine bak — `192.168.x` gibi normal bir yerel IP mi? Değilse A birden fazla ağa bağlı olabilir. |
| Bağlanmıyor (aynı ağ, doğru kod) | Router'da "AP/Client Isolation" (istemci yalıtımı) kapalı olmalı — bazı modemler cihazların birbirini görmesini engeller. |
| Uygulama açılmıyor | Kurulum .NET 8'i otomatik kurar; yine de olmazsa bilgisayarı yeniden başlat. |
| "Akıllı Uygulama Denetimi engelledi" | Aşağıdaki "İmza / Smart App Control" bölümüne bak. |
| Restore dosyayı açmıyor | B'de VS Code'un `code` komutu kurulu olmalı (VS Code → "Shell Command: Install 'code' command") |

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
