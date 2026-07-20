# 🌐 PalmSync - Proje Yol Haritası ve Mikro Fazlar

## 📋 Proje Özeti

PalmSync; Rust ve C/C++ ile geliştirilmiş, tamamen bulutsuz P2P çalışma alanı senkronizasyon sistemidir. Kullanıcı bir el hareketi ile bir bilgisayardaki çalışma alanını yakalar (Grab) ve ağdaki başka bir bilgisayarda anında kaldığı yerden canlandırır (Drop).

---

## 🎯 Mikro Fazlar ve Zaman Çizelgesi

| Faz | Başlık | Süre | Durum | Belge |
|-----|--------|------|-------|-------|
| **1** | Temel Altyapı ve Networking | 2-3 hafta | 📝 Planlama | [faz-1-networking.md](./faz-1-networking.md) |
| **2** | Görüntü İşleme (The Eye) | 3-4 hafta | ⏳ Beklemede | [faz-2-eye.md](./faz-2-eye.md) |
| **3** | Çekirdek Sistem (The Brain) | 2-3 hafta | ⏳ Beklemede | [faz-3-brain.md](./faz-3-brain.md) |
| **4** | Platform Entegrasyonu | 2 hafta | ⏳ Beklemede | [faz-4-deployment.md](./faz-4-deployment.md) |
| **5** | UI ve Kullanıcı Deneyimi | 2 hafta | ⏳ Beklemede | [faz-5-ui.md](./faz-5-ui.md) |
| **6** | Güvenlik ve Optimizasyon | 2 hafta | ⏳ Beklemede | [faz-6-security.md](./faz-6-security.md) |
| **7** | Testing ve Kalite Kontrol | 2 hafta | ⏳ Beklemede | [faz-7-testing.md](./faz-7-testing.md) |

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

---

## 🚀 Başlangıç

1. **Faz 1 belgesini okuyun**: [faz-1-networking.md](./faz-1-networking.md)
2. **Proje yapısını oluşturun**: Cargo workspace ve CMake build setup
3. **Bağımlılıkları kurun**: Tokio, Tailscale SDK, OpenCV
4. **İlk test kodunu yazın**: WebSocket echo server

---

## 📞 İletişim ve Notlar

- Her faz bağımsız şekilde geliştirilebilir ancak sıraya uyulması önerilir
- Bağımlılıklar varsa faz belgelerinde açıkça belirtilmiştir
- Regular update ve refactoring için yer ayrılmıştır
- Ön şart olarak Rust 1.70+ ve MSVC derleyicisi gereklidir

---

**Son Güncelleme**: 2026-07-20  
**Versiyon**: 1.0.0 (Pre-Alpha)
