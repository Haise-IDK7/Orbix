# Task Tracker

> Updated by the agent during task execution. Mark items `[/]` in-progress, `[x]` complete.

## Current Tasks

### Notch/dock improvements (wifi, performance, lyrics, dock pins, glass)
- [x] Wi-Fi: native WLAN API scan/connect, location-permission hint (`src-tauri/src/wifi.rs`, `WifiSubView.tsx`)
- [x] Performance panel: RAM usage, working-set trim, close heavy apps (`src-tauri/src/memory.rs`, `PerformanceSubView.tsx`)
- [x] Live synced lyrics from LRCLIB + collapsed-notch ticker (`src-tauri/src/lyrics.rs`, `src/components/lyrics/`)
- [x] Media timeline paired with LastUpdatedTime for accurate position (`services.rs`)
- [x] Dock: pin WhatsApp/Spotify once when installed; sturdier packaged-app icon lookup
- [x] Glass style for notch and dock with Appearance toggle (`src-tauri/src/glass.rs`, `src/components/glass/`)

### Previous

- [x] Task 1: Backend Non-Recursive Direct Folder Scanning (`src-tauri/src/wallpaper.rs`)
- [x] Task 2: Backend 800x450 Thumbnail Resolution & Persistent Disk Cache (`src-tauri/src/wallpaper.rs`)
- [x] Task 3: Frontend Carousel Track Virtualization (17 Cards) (`src/components/wallpaper/WallpaperView.tsx`)
- [x] Task 4: Frontend Priority Queue & Batched Thumbnail Loading (`src/components/wallpaper/WallpaperView.tsx`)
- [x] Task 5: Final Validation & Release Build
