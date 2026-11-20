# WinIsland 2.0: a Dynamic Island for Windows

A little black island at the top of your main display that unfolds into live activities. Two looks, switchable
any time from the top of the Settings window:

| Look | What it is |
|---|---|
| **macOS** | A notch flush with the top edge of the screen, with concave shoulders (MacBook style). The original version. |
| **iOS** | A floating capsule 10 px below the edge (iPhone Dynamic Island style), with iOS segmented tabs, a sky-coloured weather widget, Activity-style CPU/RAM rings, iOS sliders and green switches. |

* **Idle:** time + weather (or an empty island).
* **Live activities:** now playing (album art + visualizer from your real audio output), karaoke lyrics that sing
  along in the island, timer, camera/mic in-use dots.
* **Events:** volume HUD, music pop-up on track change (can be switched off), notifications (if Windows allows),
  greeting at start-up.
* **Dashboard (click or hover):** Home (album art, seek bar, controls, volume, weather, CPU/RAM), **Lyrics**
  (time-synced karaoke from LRCLIB; click a line to jump there) and Timer. With several players open
  (e.g. Spotify + YouTube) chips at the top choose which one you control.
* Moving the pointer off the island closes it, however it was opened. Scroll on the island to change volume.
  Hides during fullscreen apps, while locked and when the screen is off.

## Install
Run `dist\WinIslandSetup.exe` (49 MB, self-contained: no .NET install needed; needs the WebView2 runtime, which
Windows 11 includes). The wizard asks for the look, the folder (per-user, no admin), autostart, shortcuts and lyrics.
Uninstall from **Windows Settings → Apps → Installed apps → WinIsland**.

Silent: `WinIslandSetup.exe /silent [/dir="…"] [/style=ios] [/noautostart] [/desktop] [/nostartmenu] [/nolaunch]`,
uninstall: `"<install dir>\Uninstall.exe" /uninstall /silent [/keepsettings]`.

## Use
| What | How |
|---|---|
| Settings | **Ctrl + Alt + F**, the tray icon, or Start → *WinIsland* |
| Switch look | the macOS / iOS switch at the top of Settings |
| Quit | tray icon → Exit, or Settings → General → Quit |
| Autostart | `HKCU\…\Run\FrostDesktop` (toggle in Settings → General) |

## Project layout
```
FrostDesktop\
  src\            C# host (.NET 10, WebView2, DirectComposition). Dev build: cd src; dotnet build -c Release → bin\
  web\            island.* (the island), panel.* (settings), common.js, demo.js (demo bridge for previews), icon.svg
  installer\      Setup\ (the .NET Framework 4.8 wizard: Engine.cs, Ui.cs, ui\setup.*) and build.ps1
  dist\           WinIslandSetup.exe (built by installer\build.ps1)
  tools\Capture\  WebView2 renderer for screenshots and deterministic video frames (used for the website & promo)
  website\        promotional landing page
  video\          After Effects promo project and render
  branding\       app icon in all sizes (frost.ico, icon-*.png); frost-v1-original.ico = the old icon
  settings.json   options (created on first run, not in the repo). Installed copies keep it next to the app,
                  or in %LOCALAPPDATA%\WinIsland if the install folder isn't writable.
```

The installer is in `website/downloads/WinIslandSetup.exe`. Build it yourself with `installer\build.ps1`
(needs the .NET 10 SDK).

## Footprint (measured on this PC)
Idle: ~0.04 % total CPU, ~460 MB working set (~210 MB private) across the host and its 6 WebView2 processes.
The visualizer samples audio only while music plays and the island is visible; lyrics are fetched once per song;
system stats are sampled only while the dashboard is open.
