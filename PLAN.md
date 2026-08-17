# PLAN.md — OpenDesign Hatch → oh-my-opencode-slim Desktop Companion (Linux / Tauri v2)

> **Durum:** v1.0 — Oracle incelemesi tamamlandı (APPROVE-WITH-CHANGES, tüm bulgular uygulandı)
> **Tarih:** 2026-08-17
> **Proje kökü:** `/home/exoristos/Projelerim/opencode-companion`

---

## 1. Proje Özeti

OpenDesign "Hatch" pipeline'ı ile üretilmiş **husky-pet** (siyah-beyaz Sibirya husky, chibi pixel-art)
sprite varlıklarını temel alan, Linux üzerinde çalışan hafif, çerçevesiz, transparan bir
**oh-my-opencode-slim desktop companion overlay** uygulaması. Uygulama, OpenCode ajan durumlarını
(`idle` / `busy` / `waiting-input`) canlı olarak pet animasyonlarıyla görselleştirir ve global kısayol
ile hızlı prompt giriş moduna geçebilen hibrit bir mimaride çalışır.

**Önemli düzeltmeler (araştırma sonucu, kaynak: upstream `alvinunreal/oh-my-opencode-slim` @ master):**

| Varsayım (görev metni) | Gerçek (doğrulanmış) |
|---|---|
| Entegrasyon: stdin/IPC event köprüsü | **Paylaşılan JSON durum dosyası, 250 ms mtime polling** — plugin binary'yi `stdio: 'ignore'`, argümansız spawn eder |
| Durumlar: `idle/thinking/executing/error` | Protokol durumları: **`idle` / `busy` / `waiting-input`**; ajan kimliği `active_agents` dizisinde |
| Config: `~/.config/opencode/oh-my-opencode-slim.json` | Config: **`~/.config/opencode/oh-my-opencode-slim.jsonc`** (loader `.jsonc`'yi tercih eder) |
| Hazır OpenDesign web arayüzü (HTML/CSS/JS) | **HTML/CSS/JS yok** — yalnızca sprite varlıkları; viewer UI sıfırdan inşa edilecek |
| Global kısayol her ortamda çalışır | `tauri-plugin-global-shortcut` **Linux'ta yalnızca X11** (global-hotkey 0.8.0); Wayland'de callback tetiklenmez |

---

## 2. Mimari ve Teknoloji Yığını

- **Frontend:** Vanilla HTML5 + CSS3 + JS (sprite-atlas animasyonlu pet viewer) — `src/` (Tauri webview kökü)
- **Runtime/Host:** Tauri v2 (Rust + WebKitGTK), crate `tauri` 2.11.x, `@tauri-apps/api` 2.11.x
- **Platform:** Linux (X11 & Wayland uyumlu), `app-id: oh-my-opencode-slim-companion`
- **Entegrasyon:** `companion-state.json` dosya polling (Rust) → Tauri event → frontend `data-state`/`data-agent`
- **Kısayol:** `tauri-plugin-global-shortcut` 2.3.x (X11) + Wayland fallback (compositor keybind → CLI `--toggle`)

### 2.1 Dosya düzeni (hedef)

```
opencode-companion/
├── PLAN.md
├── src/
│   ├── index.html            # pet viewer UI (Tauri webview girişi)
│   ├── main.js               # event dinleyici + durum → animasyon eşleme
│   ├── styles.css            # transparan arka plan + sprite animasyonları
│   └── assets/
│       └── spritesheet.webp  # 1536×1872, 8×9 grid, 192×208 hücre (husky-pet'ten kopya)
├── package.json
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json   # pencere yapılandırması
    ├── capabilities/default.json
    ├── icons/
    └── src/
        ├── main.rs       # env var işleme + NVIDIA/Wayland workaround'ları
        └── lib.rs        # state poller, event emit, kısayol, CLI toggle
```

---

## 3. Varlık Kaynakları (doğrulanmış)

| Varlık | Kaynak | Detay |
|---|---|---|
| Sprite atlası | `~/.codex/pets/husky/spritesheet.webp` (veya `.od/projects/husky-pet-5cf7/run/final/spritesheet.{png,webp}`) | 1536×1872, 8 kolon × 9 satır, hücre 192×208, lossless WebP + alpha |
| Frame manifesti | `.od/projects/husky-pet-5cf7/run/frames/frames-manifest.json` | Satır sırası + durum başına kare sayıları |
| Doğrulama | `.od/projects/husky-pet-5cf7/run/final/validation.json` | `ok: true`, 72 hücre, 0 hata |
| Pet spec | `.od/projects/husky-pet-5cf7/run/pet_request.json` | Husky, mavi gözler, chibi pixel-art, kalın kontur, düz cel shading |

**Atlas satır sırası (satır → durum → kare sayısı):**
`0 idle(6) · 1 running-right(8) · 2 running-left(8) · 3 waving(4) · 4 jumping(5) · 5 failed(8) · 6 waiting(6) · 7 running(6) · 8 review(6)` — toplam 57 kare.

---

## 4. Uygulama Fazları

### Faz 1: Tauri v2 İskeleti, Overlay Pencere ve Pet Viewer UI

**1.1 Sistem bağımlılıkları (Arch/CachyOS):**
```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl librsvg
```
(xdotool / libappindicator / appmenu-gtk-module gerekli değil — tray ve X11 otomasyonu kapsam dışı.)

**1.2 Scaffold:**
```bash
cd /home/exoristos/Projelerim/opencode-companion
npm create tauri-app@latest . -- --template vanilla --manager npm \
  --identifier oh-my-opencode-slim-companion --yes
```

**1.3 Pencere yapılandırması (`src-tauri/tauri.conf.json`, `app.windows[0]`):**
```json
{
  "label": "main",
  "url": "index.html",
  "title": "oh-my-opencode-slim-companion",
  "width": 240, "height": 320,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "shadow": false,
  "focus": false,
  "focusable": true,
  "resizable": false,
  "visible": true
}
```
Notlar:
- `shadow` Linux'ta desteklenmez (no-op). `transparent` WebKitGTK'de çalışır; **compositor gerektirir**.
- Boyut 240×320: pet hücresi 192×208 + durum etiketi + gizli prompt input + kenar boşlukları.
  (Görev metnindeki 340×480, daha büyük bir bileşen varsaymıştı; `tauri.conf.json`'dan ayarlanabilir.)
- `focus: false` (spawn'da odak çalmaz — referans `active(false)` ile aynı) ama **`focusable: true`**
  (kısayolla açıldığında prompt input klavye odağı alabilmeli — B2).

**1.4 Transparanlık workaround'ları (`src-tauri/src/main.rs`, app build öncesi):**
```rust
#[cfg(target_os = "linux")]
{
    // DİKKAT: main() içinde, app build'den ÖNCE tek iş parçacıklı ortamda çağrılmalı (Rust 2024).
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1"); // NVIDIA+Wayland GBM "Error 71" koruması
    std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
}
```
CSS zorunluluğu: `html, body { background: transparent; }` (kök kapsayıcıda `background-color: transparent`).

**1.5 Pet viewer UI (sprite atlas animasyonu):**
- `spritesheet.webp` → `assets/` (kopya).
- CSS: her durum için `background-position` animasyonu (satır offset × 192/208, kare sayısı × adım).
- Sprite render politikası: **tam sayı ölçek + `image-rendering: pixelated`** (kesirli ölçek pixel-art'ı bozar).
- Pet kapsayıcısına `data-tauri-drag-region` attribute'u (çerçevesiz pencerede sürükleyerek taşıma — M3).
- Durum → animasyon eşlemesi (protokol sözlüğüne göre):
  - `idle` → satır 0 (idle)
  - `busy` → satır 7 (running)
  - `waiting-input` → satır 6 (waiting)
  - `error` → **protokolde yok** — bkz. Risk R4
- UI: pet + durum etiketi + (Faz 3'te) gizlenebilir prompt input. Tüm fontlar yerel (çevrimdışı).
- **Ajan kimliği:** `active_agents` atlas satırı değil, **etiket metni** olarak gösterilir; animasyon
  satırını yalnızca `status` belirler (m1).

**Faz 1 çıktısı:** Derlenebilir Tauri iskeleti + transparan overlay penceresi + spritesheet'ten animasyonlu pet.

---

### Faz 2: oh-my-opencode-slim Durum Entegrasyonu (State Poller & UI)

**2.1 Rust tarafı (`src-tauri/src/lib.rs`):**
- `std::thread::spawn` içinde `companion-state.json` dosyasının **mtime'ını 250 ms'de** kontrol eden poller
  (referans `state.rs::poll_loop` ile birebir davranış).
- Dosya yolu: `$XDG_DATA_HOME/opencode/storage/oh-my-opencode-slim/companion-state.json`
  (varsayılan `~/.local/share/opencode/storage/oh-my-opencode-slim/companion-state.json`).
- JSON parse (`serde_json`) → `AppHandle` clone ile frontend'e event emit:
  - **Event:** `opencode:state-change`
  - **Payload (protokol gerçeğiyle düzeltilmiş):**
    ```json
    { "session_id": "...", "cwd": "/path", "active_agents": ["orchestrator","fixer"],
      "status": "idle|busy|waiting-input", "message": null }
    ```
- **Oturum seçimi** (referans `app.rs::choose_session`): en yeni `waiting-input` kazanır → non-intro
  `active_agents` olan → `busy` → en yeni. `OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID` env var'ı varsa
  sahip oturum önceliklidir.
- **Env var sözleşmesi:** plugin binary'yi `OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID` ile spawn eder;
  referans binary yoksa çıkar. Davranış: env var yoksa ve `--dev` bayrağı verilmemişse çık; `--dev`
  standalone test modu açar.
- **Kenar durumları (M1):**
  - Dosya yok / okunamıyor / `sessions: []` → **intro/idle** görünümü (plugin çalışmıyor = dosya yok, daemon
    için normal durum). Çökme yok.
  - JSON parse hatası / bozuk dosya → **son iyi durumu koru**, çökme yok, log spam yok.
  - Sahip oturum (env var) listede yok → standart seçim mantığına düş.
  - `version != 1` → **hard reject + son iyi durumu koru** (plugin'in kendi `readState` davranışını
    yansıtır; "zarif degrade" değil — ora-3 doğrulaması).
- **`window_positions` geri yazımı v1'den ÇIKARILDI** (M2): protokolün `companion-state.json.lock` dizin
  kilidini gerektirir ve plugin dosyasına yazan tek kısımdır; upstream'te doğrulanmamıştır. v1.1'e ertelendi
  (uygulanırsa: önce lock dizinini al, sonra atomik tmp+rename).

**2.2 Frontend (`main.js`):**
```js
// Vanilla template'te bundler yok — bare-specifier import çözülmez.
// withGlobalTauri: true sayesinde window.__TAURI__.event.listen kullanılır.
const listen = window.__TAURI__.event.listen;
const unlisten = await listen('opencode:state-change', (e) => {
  const { status, active_agents } = e.payload;
  document.body.dataset.state = status;          // idle|busy|waiting-input → animasyon satırı
  document.body.dataset.agent = active_agents?.[0] ?? 'intro'; // yalnızca etiket metni (m1)
});
```
- DOM'a `data-state` / `data-agent` attribute'ları; CSS `data-state`'e göre sprite satırını ve
  glow/pulse animasyonlarını tetikler; `data-agent` yalnızca etiket metnini besler.

**Faz 2 çıktısı:** Gerçek plugin durum dosyasından canlı pet animasyonu.

---

### Faz 3: Hibrit Asistan Yetenekleri (Global Kısayol & Genişleme)

**3.1 `tauri-plugin-global-shortcut` (yalnızca X11):**
- `Cargo.toml`: `[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]`
  `tauri-plugin-global-shortcut = "2.3"` (Rust ≥ 1.77.2 gerekir).
- `lib.rs`: `.plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(...).build())` +
  `setup` içinde `app.global_shortcut().register("Ctrl+Space")`.
- **Oturum tipi tespiti (M4):** başlangıçta `XDG_SESSION_TYPE` / `WAYLAND_DISPLAY` kontrol et;
  kısayolu **yalnızca X11'de** kaydet. Wayland altında çalışırken tek satır ipucu logla
  (kullanılacak hyprland `bind` satırı) — sessiz "başarılı" kayıt maskelenmesin.
- `capabilities/default.json`:
  ```json
  { "identifier": "main-capability", "windows": ["main"],
    "permissions": ["global-shortcut:allow-register", "global-shortcut:allow-unregister",
                    "global-shortcut:allow-is-registered"] }
  ```
- Handler: pencere görünürse gizle, değilse göster + `set_focus()` + prompt input'a `focus`.

**3.1b Tek-instance (B1 — zorunlu):**
- `tauri-plugin-single-instance` ekle: ikinci çağrı argv'yi birincil sürece iletir; birincil süreç
  `--toggle` görürse pencere görünürlüğünü değiştirir. Wayland yolu (compositor keybind) bu mekanizmaya
  dayanır — plugin-spawn edilmiş örnek zaten canlıyken ikinci bir pencere açılmaz.

**3.2 Wayland fallback (zorunlu — CachyOS/Hyprland):**
- `tauri-plugin-global-shortcut` Linux'ta **yalnızca X11** (global-hotkey 0.8.0, x11rb; XDG portal
  desteği yalnızca birleştirilmemiş PR'larda). Wayland'de kayıt "başarılı" görünür ama callback tetiklenmez.
- Çözüm: binary'ye **CLI bayrağı** `--toggle` (tek-instance üzerinden birincil sürece iletilir) +
  compositor keybind:
  ```ini
  # ~/.config/hypr/hyprland.conf
  bind = CTRL SPACE, exec, /home/exoristos/.local/bin/opencode-companion --toggle
  ```
- X11 oturumlarında plugin kısayolu, Wayland'de compositor keybind'i devrededir (ikisi de aynı toggle
  mantığını çağırır).
- **Not (ora-4):** `Ctrl+Space` ibus/fcitx IME değiştirici ve editör kısayoluyla çakışır (global X grab
  herkesten çalar). Spec gereği korundu; çakışma olursa `Ctrl+Alt+Space`'e geçilebilir.
- **Not (ora-4):** `opencode:show-prompt` event'i webview listener'ından önce düşerse pencere prompt
  kapalı açılır — kurtarma yolu pet'e tıklamaktır (v1 için kabul edildi).
- **Pencere kuralları (ora-2/ora-5):** always-on-top/boyut Wayland'de ipucudur; compositor pencereyi
  tile edebilir. **Bu makine KDE/KWin** (XDG_CURRENT_DESKTOP=KDE, Hyprland değil). İlk görsel doğrulamada:
  - KWin pencere kuralları (Sistem Ayarları → Pencere Yönetimi → Pencere Kuralları): pencere sınıfına
    göre **Yüzer / Her zaman üstte / Boyut 240×320** (gerçek window class ilk çalıştırmada
    `xprop WM_CLASS` veya KWin ile doğrulanacak).
  - KWin özel kısayol (Sistem Ayarları → Kısayollar → Özel Kısayollar): `~/.local/bin/opencode-companion --toggle`
    çalıştıran yeni kısayol (örn. Ctrl+Space) — Wayland'de toggle'ın tek yolu budur.
  - Hyprland kullanılırsa alternatif:
    ```ini
    windowrulev2 = float, class:^(oh-my-opencode-slim-companion)$
    windowrulev2 = size 240 320, class:^(oh-my-opencode-slim-companion)$
    windowrulev2 = pin, class:^(oh-my-opencode-slim-companion)$
    windowrulev2 = noborder noinitialfocus, class:^(oh-my-opencode-slim-companion)$
    ```
- **Not (ora-5):** başıboş `--dev`/manuel başlatılmış örnekler her OpenCode oturumunda geçici bir
  spawn'a yol açar (54ms'de çıkar, ölü PID yazar). İlk gerçek oturumdan önce
  `pkill -f oh-my-opencode-slim-companion` ile temizle.
- **Not (ora-5):** plugin yalnızca spawn eden oturum kapanırken companion'ı öldürür; son oturum başka
  bir oturumsa companion hayatta kalır (idle gösterir). Kapatma anahtarı: `pkill -f oh-my-opencode-slim-companion`.

**3.3 Prompt input:**
- UI'da gizlenebilir minimal input; kısayol/tıklama ile açılır ve `focus` alır. Enter → v1'de log/echo;
  v2'de `opencode` CLI'ya iletim (kapsam dışı, ayrı karar).

**Faz 3 çıktısı:** Kısayolla açılıp kapanan, prompt girişli hibrit overlay.

---

### Faz 4: Linux Derleme ve Companion Entegrasyonu

**4.1 Release derleme:**
```bash
npm run tauri build   # → src-tauri/target/release/oh-my-opencode-slim-companion
```
Not: `bundle.active: false` (Oracle ora-2 onaylı sapma) — AppImage bundling linuxdeploy/FUSE
gerektirir ve bu ortamda çalışmıyor; plugin ham binary'yi `binaryPath` ile spawn ettiği için
paketleme gerekmez. Dağıtım gerekirse yeniden etkinleştirilir.

**4.2 Binary yerleşimi:**
```bash
mkdir -p ~/.local/bin
ln -sf /home/exoristos/Projelerim/opencode-companion/src-tauri/target/release/oh-my-opencode-slim-companion \
  ~/.local/bin/opencode-companion
```
Not (ora-5): symlink kırılgan — `cargo clean` veya repo taşınırsa plugin "binary not found" diyip
companion'ı sessizce atlar. Kabul edilebilir (yerinde rebuild çalışır); taşınma durumunda symlink
yenilenmeli.

**4.3 Plugin config wiring (`~/.config/opencode/oh-my-opencode-slim.jsonc`):**
```jsonc
{
  "companion": {
    "enabled": true,
    "binaryPath": "/home/exoristos/.local/bin/opencode-companion",
    "position": "bottom-right",
    "size": "medium",
    "loopStyle": "classic",
    "speed": 1
  }
}
```
Notlar:
- `binaryPath` özel binary'yi otomatik güncellemez (istenen davranış — Windows PE32+ exe'yi ezer).
- Plugin, spawn edilen child PID'yi `companion.pid` dosyasına yazar (tek-instance guard).
- Mevcut bozuk kurulum (`~/.local/share/opencode/storage/oh-my-opencode-slim/bin/oh-my-opencode-slim-companion.exe`,
  Windows PE32+) `binaryPath` ile devre dışı kalır; istenirse silinebilir.

**Faz 4 çıktısı:** Çalışan, plugin tarafından spawn edilen, canlı durum gösteren companion.

---

## 5. Kabul Kriterleri

- **AC-1:** `npm run tauri build` Linux (CachyOS) üzerinde hatasız tamamlanır.
- **AC-2:** Pencere çerçevesiz, transparan, her zaman üstte, görev çubuğunda görünmez, 240×320.
- **AC-3:** `companion-state.json` değişince pet animasyonu **bir poll döngüsü içinde (~250 ms)**
  güncellenir (idle/busy/waiting-input eşlemeleri doğru).
- **AC-4:** X11'de `Ctrl+Space` toggle çalışır; Wayland'de compositor keybind ile aynı davranış.
- **AC-5:** Plugin `companion.binaryPath` üzerinden binary'yi spawn eder; gerçek bir OpenCode oturumunda
  ajan aktivitesi canlı görselleşir.
- **AC-6:** Çevrimdışı çalışır — ağ çağrısı yok, fontlar yerel.
- **AC-7:** Durum dosyası yok / bozuk / `sessions: []` iken uygulama çökmez; intro/idle görünümü gösterir.
- **AC-8:** `OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID` env var'ı olmadan (ve `--dev` olmadan) binary
  düzgün çıkar; `--dev` ile standalone çalışır.
- **AC-9:** Kısayol toggle sonrası yazılan karakterler prompt input'a ulaşır (X11 ve Hyprland'de).

## 6. Riskler ve Önlemler

| # | Risk | Önlem |
|---|---|---|
| R1 | Wayland'de global kısayol tetiklenmez (X11-only plugin) | Compositor keybind → `--toggle` CLI bayrağı (Faz 3.2) |
| R2 | NVIDIA + Wayland transparanlık GBM "Error 71" / siyah köşeler | `WEBKIT_DISABLE_DMABUF_RENDERER=1`, `__NV_DISABLE_EXPLICIT_SYNC=1` (Faz 1.4) |
| R3 | WebKitGTK < 2.48 transparan pencerede repaint hatası (tauri#12800) | webkit2gtk-4.1 güncel tut (CachyOS paketi 2.48+); gerekirse `WEBKIT_DISABLE_COMPOSITING_MODE=1` |
| R4 | Protokolde `error` durumu yok — hata görselleştirmesi protokol dışı | v1: `error` eşlemesi yok; `failed` animasyonu yalnızca plugin genişletilirse (kapsam dışı, ayrı iş). Dokümante edilir |
| R5 | Tauri binary + plugin sözleşmesi upstream'te test edilmemiş | Gerçek oturumla doğrula (AC-5); başarısızlıkta referans eframe binary'sine dönüş yolu açık |
| R6 | Transparanlık compositor gerektirir; **always-on-top Wayland'de ipucudur, garanti değildir** (Hyprland genelde onurlandırır, bazı compositor'lar onurlandırmaz) | Dokümante edilir; X11'de compositor olmadan çalışmaz |
| R7 | **WebKitGTK kaynak ayak izi:** webview ~100–200 MB RSS (eframe referansı ~30 MB) + transparan pencere için compositor katmanı | **Faz 1'de ölçüldü: RSS 195 MB ✓, idle CPU %0.4 ✓** (mitigasyon sonrası, 30s warmup + 10s pencere ölçümü). Mitigasyon: drop-shadow kaldırıldı, idle animasyonu 900ms'e yavaşlatıldı, ambient 6.8s. Eframe fallback gerekmedi (eşikler: RSS >300 MB veya CPU >%5) |

## 7. Doğrulama Stratejisi

1. `cargo check` / `cargo build` — derleme doğrulaması.
2. `npm run tauri dev` — overlay penceresi smoke testi (çerçevesiz/transparan/üstte).
3. **Faz 1 sonunda kaynak ölçümü (R7):** RSS + idle CPU ölç; >300 MB veya >%2 idle CPU → eframe
   fallback kararı (Faz 2–4'e girmeden).
4. Örnek `companion-state.json` yazarak durum geçişlerini test et (idle→busy→waiting-input) **+ kenar
   durumları: dosya yok, bozuk JSON, `sessions: []`, bayat oturum (plugin öldü → pet `busy`'de donmasın;
   N değişmeyen poll sonrası idle'a düş — opsiyonel)**.
5. Gerçek OpenCode oturumu + plugin ile uçtan uca doğrulama (AC-5).
6. RAM/CPU ölçümü — hafiflik hedefi (referans eframe binary'siyle karşılaştırma).

## 8. Kapsam Dışı (v1)

- `error` durumunun protokole eklenmesi (plugin genişletmesi gerektirir).
- Kullanılmayan atlas satırları (3 waving, 4 jumping, 5 failed, 8 review) — gelecekteki plugin
  genişletmeleri için ayrılmıştır (R4'ü de kapatır).
- `window_positions` geri yazımı (v1.1; lock sözleşmesiyle birlikte).
- Prompt'un OpenCode CLI'ya iletilmesi (v2).
- Çoklu monitör konum yönetimi, ekran kilidi/uyku sonrası durum kurtarma.
- Tray ikonu (referans binary'de yok; gerekirse ayrı iş).
- Pencere başlangıç konumu: v1'de sabit (sistem varsayılanı); `window_positions`/config'ten ilk yerleşim v1.1.