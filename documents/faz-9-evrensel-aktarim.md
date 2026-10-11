# Faz 9: Evrensel Uygulama Aktarımı

**Durum:** Kod tamamlandı, birim ve tek bilgisayar testleri geçti; iki bilgisayarda deneme bekliyor
**Hedef sürüm:** 1.0.0
**Hedef:** Yakala/Geri yükle yalnızca VS Code, tarayıcı ve panoyla sınırlı kalmasın; öndeki **her** uygulama aktarılabilsin. Karşı bilgisayarda uygulama yoksa en yakın eşdeğerle açılsın, o da yoksa ekran görüntüsü ve açık bir uyarı gösterilsin.

---

## 1. Davranış

| Kaynakta öndeki | Hedefte aynı uygulama var | Yoksa |
|---|---|---|
| Tarayıcı (Chrome, Edge, Brave, Firefox, Opera, Vivaldi) | Sayfa aynı tarayıcıda | Varsayılan tarayıcıda (Chrome → Brave gibi) |
| Metin/kod düzenleyici | Dosya aynı düzenleyicide | VS Code → Cursor → VSCodium → Sublime Text → Notepad++ → Not Defteri |
| Belge ya da dosya açan herhangi bir uygulama (Word, Excel, Acrobat, Typora, Photoshop…) | Dosya aynı uygulamada | O uzantıyı destekleyen programda: varsayılan program, yoksa o tür için kayıtlı başka bir program (`OpenWithProgids` / `OpenWithList`); metinse en son düzenleyici |
| Tarayıcıda açık yerel dosya (`file:///…pdf`) | Aynı tarayıcıda | Dosya olarak gider, uzantısını destekleyen programda açılır |
| Dosya Gezgini | Aynı klasör varsa açılır | Ekran görüntüsü |
| Belgesi olmayan uygulama (Spotify, Discord, Figma…) | Uygulama başlatılır | Ekran görüntüsü + "Uygulama bulunamadı: …" |
| Hiçbiri uymuyor | — | Dosya `Gelen Dosyalar`a kaydedilir, Gezgin'de gösterilir, ekran görüntüsü açılır |

Her geri yüklemeden sonra bildirimde ne yapıldığı yazar ("Sublime Text bu bilgisayarda yok — notlar.txt VS Code ile açıldı").

## 2. Yakalama

- **Uygulama:** öndeki pencerenin işlemi, exe yolu (`%LOCALAPPDATA%\…` gibi taşınabilir biçimde), görünen adı, başlığı ve türü.
- **Belge yolu:** sırayla (1) başlıktaki tam yol, (2) işlemin ve başlığın dosya adını içeren diğer işlemlerin komut satırları (Windows 11 Not Defteri'nin `/SESSION:` argümanındaki UTF-16 yol dahil), (3) Windows "Son kullanılanlar" kısayolları. Bulunan dosya başlıktaki adla eşleşmek zorunda.
- **İçerik:** UTF-8 metin ≤ 2 MB → `file_content`; diğer dosyalar ≤ 8 MB → `file_data_b64`.
- **Klasör:** Dosya Gezgini penceresinin klasörü (Shell.Application).
- **Ekran görüntüsü:** `PrintWindow` ile, PalmSync önizlemesi önde olsa bile; en fazla 1600×1000 JPEG. Parola yöneticileri, kimlik/UAC pencereleri ve gizli gezinti pencerelerinden alınmaz.
- Kaydedilmemiş değişiklikler yalnızca düzenleyici gerçekten öndeyse Ctrl+S ile kaydedilir.

## 3. Güvenlik kuralları

- Ağdan gelen exe yolu asla çalıştırılmaz; uygulama hedefte yerel olarak aranır (App Paths, bilinen klasörler, PATH).
- Kabuklar, betik yorumlayıcıları, kurulum ve sistem araçları hiç başlatılmaz; `%WINDIR%` içinden yalnızca Not Defteri / Paint gibi birkaç araç.
- Kod çalıştıran dosya türleri yalnızca düzenleyicide açılır.
- Her şey `ShellExecuteW` / `CreateProcess` ile açılır, `cmd` hiç kullanılmaz.
- Gelen dosyalara Mark-of-the-Web eklenir.

## 4. Uygulama Adımları

- [x] `StateSnapshot`'a `app`, `file_data_b64`, `screenshot_jpeg_b64` alanları (eski sürümlerle uyumlu)
- [x] Doğrulama kuralları ve testleri
- [x] Uygulama kataloğu, eşdeğer zincirleri ve saf karar fonksiyonları (`brain/src/apps.rs`)
- [x] Win32 yardımcıları: ShellExecute, dosya ilişkisi, pencere görüntüsü (`brain/src/win.rs`)
- [x] Genel belge bulma ve klasör yakalama (`context_capture.rs`)
- [x] Geri yükleme motoru ve ekran görüntüsü yedeği (`executor.rs`)
- [x] Arayüz: olay açıklamaları, sistem kontrolünde "Aktarım için uygulamalar"
- [x] Tanılama: `palmsync-daemon capture-test [out.json]`, `restore-test <in.json>`
- [ ] İki bilgisayarda gerçek deneme: tarayıcı (farklı varsayılan), Sublime → VS Code, Word → LibreOffice, olmayan uygulama → ekran görüntüsü

## 5. İlerleme Günlüğü

| Tarih | Adım | Sonuç |
|---|---|---|
| 2026-10-11 | Not Defteri yakalama (Windows 11, Türkçe) | Başlık `dosya.txt, Not Defteri` biçiminde; virgül ve oturum argümanı desteği eklendi; dosya ve içerik bulundu |
| 2026-10-11 | VS Code yakalama | `README.md` diskte bulundu, taşınabilir yol `%USERPROFILE%\Desktop\…`, 250 KB ekran görüntüsü |
| 2026-10-11 | Geri yükleme: kaynak Sublime Text | Bu makinede kurulu olduğu için aynı uygulamada açıldı |
| 2026-10-11 | Geri yükleme: kaynak Photoshop (yok) | "Uygulama bulunamadı: Adobe Photoshop — ekran görüntüsü açıldı"; görüntü tam pencere |
| 2026-10-11 | Açılan uygulamaların daemon çıktısını miras alması | Bulundu ve düzeltildi (stdio null) |
| 2026-10-11 | Uzantıyı destekleyen programla açma | Varsayılan program yoksa kayıtlı diğer programlar (`ShellExecuteEx` + sınıf); kod düzenleyiciden gelen dosya düzenleyicide kalır; tarayıcıdaki yerel dosya sekmesi dosya olarak gider (önceden bozuk `https://file:///…` adresine dönüşüyordu). Bu makinede `.pdf` için 5, `.mp4` için 5 kayıtlı program bulundu; `.txt` dosyası kayıtlı sınıfla açıldı |
