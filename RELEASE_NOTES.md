## WinIsland: a Dynamic Island for Windows

A little black island at the top of your screen that unfolds into live activities. Now a native app built with Rust and GPUI: one small exe, ~0% CPU and ~82 MB of memory while idle.

### Highlights
- **Two looks:** the **Dynamic Notch**, flush with the top edge, opens into a dashboard. The **Dynamic Island** is a floating capsule with its own iPhone-style UI: content on both sides, a second bubble when two activities run, and Live Activity cards. Switch any time from the top of Settings.
- **Music:** album art, seek bar, controls and a visualizer driven by your real audio output. Works with Spotify, YouTube and any app that shows Windows media controls.
- **Karaoke lyrics:** time-synced lyrics that fill in as they're sung. Click a line to jump there.
- **Several players at once:** choose whether you control Spotify, YouTube or another player.
- **Also:** volume HUD, timer, weather, CPU/RAM rings, camera and microphone indicators, notifications.
- **Light:** native and event-driven. It hides during fullscreen games and videos.

### Install
1. Download **WinIslandSetup.exe** below.
2. Run it and pick your look, folder and preferences. It installs per-user, so no administrator rights are needed. Installing over an earlier version keeps your settings.
3. Open settings with **Ctrl + Alt + F**.

Uninstall from **Settings → Apps → Installed apps → WinIsland**.

### Requirements
- Windows 11 (x64)
- Nothing else: no .NET and no browser runtime needed by the app.

### Privacy
Lyrics come from LRCLIB (free, open source). Only the song's title, artist, album and length are sent, once per song. Weather comes from Open-Meteo, and only for the city you choose. No accounts and no tracking.

SHA-256 of `WinIslandSetup.exe`: `21A5667CAF38165BBE3E6DF119730AAA3D185D9529773860F25507AFAAB62093`

> **AI disclosure:** WinIsland was made with AI assistance. Large parts of the code, design, installer, website and promo video were generated with Anthropic's Claude, directed and reviewed by the author.
>
> WinIsland isn't affiliated with Apple. Dynamic Island is a trademark of Apple Inc.