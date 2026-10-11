# 🌐 PalmSync - Proje Yol Haritası ve Mikro Fazlar

## 📋 Proje Özeti

PalmSync; Rust ve C/C++ ile geliştirilmiş, tamamen bulutsuz P2P çalışma alanı senkronizasyon sistemidir. Kullanıcı bir el hareketi ile bir bilgisayardaki çalışma alanını yakalar (Grab) ve ağdaki başka bir bilgisayarda anında kaldığı yerden canlandırır (Drop).

---

## 🎯 Mikro Fazlar ve Zaman Çizelgesi

| Faz | Başlık | Süre | Durum | Belge |
|-----|--------|------|-------|-------|
| **1** | Temel Altyapı ve Networking | 2-3 hafta | ✅ Tamamlandı | [faz-1-networking.md](./faz-1-networking.md) |
| **2** | Görüntü İşleme (The Eye) | 3-4 hafta | ✅ Tamamlandı | [faz-2-eye.md](./faz-2-eye.md) |
| **3** | Çekirdek Sistem (The Brain) | 2-3 hafta | ✅ Tamamlandı | [faz-3-brain.md](./faz-3-brain.md) |
| **4** | Platform Entegrasyonu | 2 hafta | ✅ Tamamlandı | [faz-4-deployment.md](./faz-4-deployment.md) |
| **5** | UI ve Kullanıcı Deneyimi | 2 hafta | ✅ Tamamlandı | [faz-5-ui.md](./faz-5-ui.md) |
| **6** | Güvenlik ve Optimizasyon | 2 hafta | ✅ Tamamlandı | [faz-6-security.md](./faz-6-security.md) |
| **7** | Testing ve Kalite Kontrol | 2 hafta | ✅ Tamamlandı | [faz-7-testing.md](./faz-7-testing.md) |
| **8** | Kamera 2.0 — Landmark Algılama | Aşamalı | 🚧 Aşama 4 gerçek kamera ölçümü | [faz-8-kamera-2.md](./faz-8-kamera-2.md) |
| **9** | Evrensel Uygulama Aktarımı | 1 oturum | ✅ Kod tamam, 2 bilgisayarda doğrulanacak | [faz-9-evrensel-aktarim.md](./faz-9-evrensel-aktarim.md) |

Kamera 2.0'ın kısa ilerleme özeti ve sıradaki işler için
[kamera-2-durum-ve-sonraki-adimlar.md](./kamera-2-durum-ve-sonraki-adimlar.md) belgesine bakın.
Gerçek kamera test sonuçları [kamera-2-test-gunlugu.md](./kamera-2-test-gunlugu.md) dosyasında tutulur.

---

## 🏗️ Mimari Genel Bakış

```
┌─────────────────────────────────────────────────────────────┐
│                    TAILSCALE MESH VPN                       │
│              (Uçtan Uca Şifrelenmiş 100.x.x.x)              │
└─────────────────────────────────────────────────────────────┘
         │                    │                    │
    ┌────┴────┐          ┌────┴────┐          ┌────┴────┐
    │  SLAVE  │          │ MASTER  │          │  SLAVE  │
    │ Node 01 │          │  Node   │          │ Node 02 │
    └────┬────┘          └────┬────┘          └────┬────┘
         │                    │                    │
    ┌────┴──────────────┐ ┌───┴────────────┐ ┌────┴──────────────┐
    │  The Eye (C++)    │ │  State Storage │ │  The Eye (C++)    │
    │  MediaPipe + IPC  │ │  (RAM Cache)   │ │  MediaPipe + IPC  │
    └────┬──────────────┘ └───┬────────────┘ └────┬──────────────┘
         │                    │                    │
    ┌────┴──────────────┐ ┌───┴────────────┐ ┌────┴──────────────┐
    │  The Brain (Rust) │ │  WebSocket RPC │ │  The Brain (Rust) │
    │  Tokio + Context  │ │   Dispatcher   │ │  Tokio + Context  │
    └───────────────────┘ └────────────────┘ └───────────────────┘
```

---

## 📚 Her Faz İçin Yapılacaklar

Her faz için ayrı bir markdown belge hazırlanmıştır. Detaylara ulaşmak için ilgili linke tıklayınız:

- **Faz 1 (Networking)**: P2P ağ altyapısı, Master/Slave mimarisi, WebSocket RPC
- **Faz 2 (The Eye)**: C++ goruntü işleme, MediaPipe, Jest durumu, IPC
- **Faz 3 (The Brain)**: Rust daemon, Tokio event loop, VS Code entegrasyonu
- **Faz 4 (Deployment)**: Windows setup, daemonization, startup integration
- **Faz 5 (UI)**: System tray, notifications, configuration interface
- **Faz 6 (Security)**: Encryption, authentication, optimization
- **Faz 7 (Testing)**: Unit tests, integration tests, E2E tests
- **Faz 8 (Kamera 2.0)**: Landmark tabanlı el algılama, kalibrasyon ve düşük gecikme
- **Faz 9 (Evrensel aktarım)**: Her uygulamanın yakalanması; aynı/eşdeğer uygulamada, varsayılan programda ya da ekran görüntüsüyle geri yükleme

---

## 🏁 1.0 Sürümü İçin Kalanlar

| # | İş | Durum |
|---|---|---|
| 1 | Kamera 2.0 gerçek kamera doğrulaması (20 tekrar × 3 mesafe × 3 ışık, yüz-only, boş sahne) | ⏳ Kullanıcı testi — [test günlüğü](./kamera-2-test-gunlugu.md) |
| 2 | Eli indirince yanlış GRAB | ✅ Düzeltildi (landmark eşikleri + 550 ms geçiş penceresi, kayıtla doğrulandı) |
| 3 | Loş ışıkta kameranın bulunamaması | ✅ Düzeltildi |
| 4 | Sessiz bağlantı kopmasında yeniden bağlanma | ✅ Heartbeat eklendi |
| 5 | Sistem kontrolünde model doğrulaması | ✅ Eklendi |
| 6 | Tüm bileşenlerde tek sürüm (`VERSION` dosyası) | ✅ |
| 7 | Lisans (Apache-2.0), NOTICE, üçüncü taraf bildirimleri | ✅ |
| 8 | Her uygulamanın aktarımı (Faz 9) | ✅ Kod — iki bilgisayarda deneme bekliyor |
| 9 | Temiz Windows'ta kurulum + otomatik güncelleme testi | ⏳ Temiz makine gerekiyor |
| 10 | Kalibrasyon sihirbazı (Faz 8 Aşama 5) | ➡ 1.1'e ertelendi |

Gerçekleşen mimari ve derleme talimatları için kök dizindeki [README.md](../README.md) dosyasına bakın;
yukarıdaki mimari çizim ilk planı (Tailscale, MediaPipe) gösterir.

---

**Son Güncelleme**: 2026-10-11  
**Hedef sürüm**: 1.0.0
