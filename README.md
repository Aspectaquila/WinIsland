# WinIsland: a Dynamic Island for Windows

> **AI disclosure:** WinIsland was made with AI assistance. Large parts of the code, UI design, installer,
> website and promotional video were generated with Anthropic's Claude, directed and reviewed by the author.

A little black island at the top of your main display that unfolds into live activities. It's a native app
written in Rust with [GPUI](https://gpui.rs/) (the GPU UI framework behind the Zed editor): one small exe,
no browser engine. Two looks, switchable any time from the top of the Settings window:

| Look | What it is |
|---|---|
| **Dynamic Notch** | A notch flush with the top edge of the screen, with concave shoulders. Click or hover and it opens into a dashboard with Home, Lyrics and Timer tabs. |
| **Dynamic Island** | A floating capsule 10 px below the edge, built like the iPhone's: a compact view with content on the left and right, a second bubble that splits off when two activities run (tap it to swap), Live Activity cards for music and timers, widget tiles (sky weather, Activity-style CPU/RAM rings) and an iOS-style volume HUD. |

* **Idle:** time + weather (or an empty island).
* **Live activities:** now playing (album art + visualizer from your real audio output), karaoke lyrics that sing
  along in the island, timer, camera/mic in use.
* **Events:** volume HUD, music pop-up on track change (can be switched off), notifications (if Windows allows),
  greeting at start-up.
* **Expanded view (click or hover):** album art, seek bar, controls, volume, weather, CPU/RAM, time-synced karaoke
  lyrics from LRCLIB (click a line to jump there) and a timer. With several players open (e.g. Spotify + YouTube)
  you choose which one you control.
* Moving the pointer off the island closes it, however it was opened. Scroll on the island to change volume.
  Hides during fullscreen apps, while locked and when the screen is off.

## Install
Run `dist\WinIslandSetup.exe` (about 6.2 MB). The wizard asks for the look, the folder (per-user, no admin),
autostart, shortcuts and lyrics. Uninstall from **Windows Settings → Apps → Installed apps → WinIsland**.

Silent: `WinIslandSetup.exe /silent [/dir="…"] [/style=notch|island] [/noautostart] [/desktop] [/nostartmenu] [/nolaunch]`,
uninstall: `"<install dir>\Uninstall.exe" /uninstall /silent [/keepsettings]`.

## Use
| What | How |
|---|---|
| Settings | **Ctrl + Alt + F**, the tray icon, or Start → *WinIsland* |
| Switch look | the Dynamic Notch / Dynamic Island switch at the top of Settings |
| Quit | tray icon → Exit, or Settings → General → Quit |
| Autostart | `HKCU\…\Run\FrostDesktop` (toggle in Settings → General) |
| Demo states | `winisland.exe --demo <scene> --demo-style notch\|island` (idle, music, expanded, lyrics, volume, drop, timer, two-activities, …) |

## Project layout
```
FrostDesktop\
  winisland\      the app: Rust + GPUI. Build: cd winisland; cargo build --release → target\release\winisland.exe
    vendor\gpui\  gpui 0.2.2 with one patch: the vsync thread sleeps while no window needs a frame (≈0% idle CPU)
  installer\      Setup\ (the .NET Framework 4.8 wizard: Engine.cs, Ui.cs, ui\setup.*) and build.ps1
  dist\           WinIslandSetup.exe (built by installer\build.ps1)
  website\        promotional landing page (GitHub Pages)
  video\          After Effects promo project
  branding\       app icon in all sizes (frost.ico, icon-*.png), social images; frost-v1-original.ico = the old icon
  src\, web\      the previous version (C# + WebView2), kept for reference
  tools\Capture\  WebView2 renderer used for the previous version's screenshots and video frames
  settings.json   options (created on first run, not in the repo). Installed copies keep it next to the app,
                  or in %LOCALAPPDATA%\WinIsland if the install folder isn't writable.
```

The installer is attached to each GitHub release. Build it yourself with `installer\build.ps1`
(needs Rust and the .NET SDK).

## Footprint (measured on this PC)
Idle: ~0 % CPU and ~82 MB private memory, in a single process (the previous WebView2 version used ~210 MB across
7 processes). While music plays, the visualizer and karaoke redraw at 30 fps for ~2 % of one core. Audio is sampled
only while music plays and the island is visible; lyrics are fetched once per song; system stats are sampled only
while they're on screen.