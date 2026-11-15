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

### Round 3: like, dock, lock screen, controls, audio toast
- [x] Spotify like button (PKCE OAuth, DPAPI token, `/me/library`) (`spotify.rs`, `components/spotify/`)
- [x] Start icon spins on hover; show-desktop bar (`SpinningIcon.tsx`, `show_desktop`)
- [x] Lock screen: idle/lock trigger, fonts, auto palette, depth/glass/blend, widgets (`lockscreen.rs`, `components/lockscreen/`, Settings → Lock Screen)
- [x] Minimal control center restyle
- [x] Date in the expanded notch
- [x] Audio toast: diff all active outputs, not just the default device

### Round 4: fixes from testing
- [x] Notch date as dd/mm
- [x] Spotify heart always visible; setup/unavailable states lead to the fix
- [x] Lock screen: Enter/click to unlock, fusion on/off, keep in front, "Go Straight to PIN" (NoLockScreen)
- [x] Wi-Fi: physical-only Ethernet detection, SSID from connection profile, location checklist with auto re-check
- [x] Bluetooth: paired device list, connect/disconnect audio (KS one-shot), Add device, connected name on tile

### Round 5: battery, translation, sleep, memory
- [x] Bluetooth battery % (Windows' device battery property) in list and tile
- [x] Lyrics translation (MyMemory, batched, deduped, disk cache) with toggle beside the lyrics button
- [x] Sleep: call SetSuspendState directly (rundll32 form hibernated); Modern Standby fallback
- [x] Memory: WebView2 low memory target for hidden Settings/Lock screen, depth model in a throwaway worker, lock screen media only while shown, clipboard image cap 24 MB, lyrics cache cap, "Orbix is using X MB" in Performance

### Round 6: README and translation engines
- [x] Translation: on-device WebView2/Edge Translator first (no limit), Microsoft Translator with user key (2M chars/month, DPAPI), MyMemory fallback
- [x] Settings → Notch: translation engines panel
- [x] README: What's New section, new screenshots, network use table, fork download

### Previous

- [x] Task 1: Backend Non-Recursive Direct Folder Scanning (`src-tauri/src/wallpaper.rs`)
- [x] Task 2: Backend 800x450 Thumbnail Resolution & Persistent Disk Cache (`src-tauri/src/wallpaper.rs`)
- [x] Task 3: Frontend Carousel Track Virtualization (17 Cards) (`src/components/wallpaper/WallpaperView.tsx`)
- [x] Task 4: Frontend Priority Queue & Batched Thumbnail Loading (`src/components/wallpaper/WallpaperView.tsx`)
- [x] Task 5: Final Validation & Release Build
