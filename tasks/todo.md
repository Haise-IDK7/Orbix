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

### Round 2: icons, dock pins, fonts, panels, clipboard
- [x] Store-app icons from the package manifest (`package_icon.rs`), icon cache v2
- [x] Pinned apps manager + max pinned (Settings → Dock)
- [x] Notch fonts (6 bundled) + tabular numbers
- [x] Performance tile in quick actions
- [x] Lyrics layout: vertical / horizontal
- [x] Pomodoro panel (isolated store) and Tasks panel
- [x] Audio device card, localized headphone detection, current output in controls
- [x] Shelf: always reachable, Files/Clipboard tabs, paste, clipboard history (`clipboard.rs`)
- [x] Page dots, mojibake fixes

### Previous

- [x] Task 1: Backend Non-Recursive Direct Folder Scanning (`src-tauri/src/wallpaper.rs`)
- [x] Task 2: Backend 800x450 Thumbnail Resolution & Persistent Disk Cache (`src-tauri/src/wallpaper.rs`)
- [x] Task 3: Frontend Carousel Track Virtualization (17 Cards) (`src/components/wallpaper/WallpaperView.tsx`)
- [x] Task 4: Frontend Priority Queue & Batched Thumbnail Loading (`src/components/wallpaper/WallpaperView.tsx`)
- [x] Task 5: Final Validation & Release Build
