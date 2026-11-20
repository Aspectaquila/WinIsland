using System.Diagnostics;
using System.Drawing;
using System.Text.Json.Nodes;
using static Frost.Native;

namespace Frost;

/// <summary>WinIsland: a macOS-notch style Dynamic Island for Windows. Nothing else.</summary>
internal sealed class App : ApplicationContext
{
    public readonly Settings Settings = new();
    private readonly SynchronizationContext _ui;
    private readonly MessageWindow _msg;
    private readonly NotifyIcon _tray;
    private readonly OcclusionMonitor _occ;
    private readonly MediaService _media;
    private readonly AudioService _audio;
    private readonly SystemStats _stats = new();
    private readonly WeatherService _weather;
    private readonly NotificationService _notify;
    private readonly PrivacyMonitor _privacy;

    private IslandSurface _island;
    private PanelSurface _panel;
    private bool _locked, _displayOff, _quitting;

    public App()
    {
        _ui = SynchronizationContext.Current ?? new WindowsFormsSynchronizationContext();
        Directory.CreateDirectory(Path.Combine(Settings.Data, "cache"));
        if (!File.Exists(Settings.FilePath)) Settings.Save();

        _msg = new MessageWindow(this);
        _occ = new OcclusionMonitor();
        _occ.Changed += _ => ApplyVisibility();
        _media = new MediaService(_ui);
        _audio = new AudioService(_ui);
        _weather = new WeatherService(Settings);
        _notify = new NotificationService(_ui);
        _privacy = new PrivacyMonitor(_ui);
        _tray = BuildTray();

        _media.Changed += m => { _island?.Post(m); _panel?.Post(m); };
        _audio.Changed += (lvl, muted, ours) => _island?.Post(new JsonObject { ["type"] = "volume", ["level"] = Math.Round(lvl, 3), ["muted"] = muted, ["ours"] = ours, ["event"] = true });
        _audio.Levels += (l, r) => _island?.Post(new JsonObject { ["type"] = "levels", ["l"] = Math.Round(l, 3), ["r"] = Math.Round(r, 3) });
        _stats.Sample += m => { _island?.Post(m); _panel?.Post(m); };
        _weather.Changed += m => { _island?.Post(m); _panel?.Post(m); };
        _notify.Arrived += m => { if (Settings.B("island.notifications")) _island?.Post(m); };
        _privacy.Changed += () => _island?.Post(PrivacyMsg());
        Settings.Changed += OnSettingsChanged;

        if (Settings.B("general.autostart")) DesktopState.SetAutostart(true);
        _ = StartAsync();
    }

    private async Task StartAsync()
    {
        _audio.Start();
        await ReconcileAsync();
        _ = _media.StartAsync();
        _ = _weather.StartAsync();
        _ = _notify.StartAsync();
    }

    private async Task ReconcileAsync()
    {
        try
        {
            if (Settings.B("island.enabled") && _island == null)
            {
                _island = new IslandSurface(this);
                _island.Received += OnPageMessage;
                await _island.InitAsync();
            }
            else if (!Settings.B("island.enabled") && _island != null)
            {
                _island.Close(); _island = null;
                _stats.Subscribe("island", false); _audio.SetMetering(false);
            }
        }
        catch (Exception ex) { Log("reconcile: " + ex); }
        ApplyVisibility();
    }

    // Hide during fullscreen apps on the main display, and while locked / screen off.
    private void ApplyVisibility()
    {
        if (_island == null) return;
        bool asleep = _locked || _displayOff;
        bool primaryFs = _occ.Current.Any(m => m.Primary && m.State == OcclusionMonitor.State.Fullscreen);
        bool show = !asleep && !(primaryFs && Settings.B("island.hideInFullscreen"));
        if (_island.Visible != show) _island.Visible = show;
        if (!show) _audio.SetMetering(false);
        _island.Post(new JsonObject { ["type"] = "visibility", ["visible"] = show });
    }

    private JsonObject PrivacyMsg() => new()
    {
        ["type"] = "privacy", ["camera"] = _privacy.Camera, ["mic"] = _privacy.Mic,
        ["cameraApp"] = _privacy.CameraApp, ["micApp"] = _privacy.MicApp,
    };

    // ================= messaging =================
    public void SendInitialState(SurfaceBase s)
    {
        s.Post(new JsonObject { ["type"] = "settings", ["settings"] = Settings.Doc.DeepClone() });
        s.Post((JsonObject)_media.Current.DeepClone());
        s.Post(new JsonObject { ["type"] = "volume", ["level"] = Math.Round(_audio.Level, 3), ["muted"] = _audio.Muted, ["event"] = false });
        if (_weather.Current != null) s.Post((JsonObject)_weather.Current.DeepClone());
        if (_stats.Last != null) s.Post((JsonObject)_stats.Last.DeepClone());
        if (s == _panel) s.Post(Diagnostics());
        if (s == _island)
        {
            s.Post(PrivacyMsg());
            if (Environment.GetEnvironmentVariable("FROST_ISLAND_DEMO") is { Length: > 0 } demo)
                s.Post(new JsonObject { ["type"] = "demo", ["view"] = demo }); // test hook: open a view without mouse input
        }
        ApplyVisibility();
    }

    private async void OnPageMessage(SurfaceBase from, JsonObject m)
    {
        try
        {
            switch ((string)m["type"])
            {
                case "media": _media.Command((string)m["action"]); break;
                case "seek": _media.Seek((double)m["position"]); break;
                case "selectSource": _media.Select((string)m["id"]); break;
                case "lyrics":
                {
                    var r = await LyricsService.GetAsync((string)m["artist"], (string)m["title"], (string)m["album"],
                        m["duration"]?.GetValue<double>() ?? 0, m["browser"]?.GetValue<bool>() ?? false);
                    r["req"] = (string)m["req"];
                    from.Post(r);
                    break;
                }
                case "levels": _audio.SetMetering(from == _island && (bool)m["on"]); break;
                case "setVolume": _audio.SetLevel((float)(double)m["level"]); break;
                case "toggleMute": _audio.ToggleMute(); break;
                case "subscribe": _stats.Subscribe(from == _island ? "island" : "panel", (bool)m["stats"]); break;
                case "openPanel": ShowPanel(); break;
                case "chrome": if (from == _panel) _panel.SetCaption((string)m["caption"]); break;
                case "settings": if (m["patch"] is JsonObject patch) Settings.Apply((JsonObject)patch.DeepClone()); break;
                case "setCity":
                {
                    var r = await WeatherService.GeocodeAsync((string)m["city"]);
                    if (r == null) { from.Post(new JsonObject { ["type"] = "toast", ["text"] = "City not found" }); break; }
                    Settings.Apply(new JsonObject { ["weather"] = r });
                    await _weather.RefreshAsync(true);
                    break;
                }
                case "locate":
                {
                    var r = await WeatherService.LocateAsync();
                    if (r == null) { from.Post(new JsonObject { ["type"] = "toast", ["text"] = "Location unavailable. Enter a city instead." }); break; }
                    Settings.Apply(new JsonObject { ["weather"] = r });
                    await _weather.RefreshAsync(true);
                    break;
                }
                case "resetSettings": Settings.ResetToDefaults(); break;
                case "diagnostics": from.Post(Diagnostics()); break;
                case "uninstall": RestoreAndQuit(); break;
                case "quit": Quit(); break;
            }
        }
        catch (Exception ex) { Log("msg: " + ex.Message); }
    }

    private void OnSettingsChanged(JsonObject patch)
    {
        var msg = new JsonObject { ["type"] = "settings", ["settings"] = Settings.Doc.DeepClone() };
        _island?.Post(msg); _panel?.Post((JsonObject)msg.DeepClone());
        if (patch.ContainsKey("general")) DesktopState.SetAutostart(Settings.B("general.autostart"));
        if (patch.ContainsKey("general") && Settings.B("general.hotkey") != _msg.HotkeyOn) _msg.SetHotkey(Settings.B("general.hotkey"));
        if (patch["weather"] is JsonObject w && w.ContainsKey("units")) _ = _weather.RefreshAsync(true);
        UpdateTray();
        _ = ReconcileAsync();
    }

    private JsonObject Diagnostics()
    {
        long hostWs = 0, wvWs = 0; int wvCount = 0;
        try
        {
            using var me = Process.GetCurrentProcess(); me.Refresh();
            hostWs = me.WorkingSet64;
            var env = WebEnv.Get();
            if (env.IsCompletedSuccessfully)
                foreach (var pi in env.Result.GetProcessInfos())
                    try { using var p = Process.GetProcessById(pi.ProcessId); wvWs += p.WorkingSet64; wvCount++; } catch { }
        }
        catch { }
        return new JsonObject
        {
            ["type"] = "diagnostics",
            ["totalMB"] = Math.Round((hostWs + wvWs) / 1048576.0), ["procs"] = wvCount + 1,
            ["windhawkIsland"] = DesktopState.WindhawkIslandActive(),
            ["fluentFlyout"] = DesktopState.FluentFlyoutRunning,
            ["notifications"] = _notify.Status,
            ["root"] = Settings.Root,
            ["data"] = Settings.Data,
        };
    }

    // ================= tray =================
    private ToolStripMenuItem _miIsland;

    private NotifyIcon BuildTray()
    {
        var font = new Font("Segoe UI Variable Text", 9.5f);
        var menu = new ContextMenuStrip { Renderer = new FrostMenuRenderer(), ShowImageMargin = false, Font = font };
        menu.Items.Add("Island settings", null, (_, _) => ShowPanel()).Font = new Font(font, FontStyle.Bold);
        _miIsland = new ToolStripMenuItem("Show island", null, (_, _) => Settings.Apply(new JsonObject { ["island"] = new JsonObject { ["enabled"] = !Settings.B("island.enabled") } }));
        menu.Items.Add(_miIsland);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add("Exit", null, (_, _) => Quit());
        foreach (ToolStripItem i in menu.Items) i.ForeColor = Color.FromArgb(232, 240, 255);
        var icoPath = Path.Combine(AppContext.BaseDirectory, "frost.ico");
        var tray = new NotifyIcon
        {
            Icon = File.Exists(icoPath) ? new Icon(icoPath) : SystemIcons.Application,
            Text = "WinIsland",
            ContextMenuStrip = menu,
            Visible = true,
        };
        tray.MouseClick += (_, e) => { if (e.Button == MouseButtons.Left) ShowPanel(); };
        UpdateTray();
        return tray;
    }

    private void UpdateTray() { if (_miIsland != null) _miIsland.Checked = Settings.B("island.enabled"); }

    public void ShowPanel()
    {
        if (_panel != null) { _panel.Activate(); return; }
        _panel = new PanelSurface(this);
        _panel.Received += OnPageMessage;
        _panel.Closed += () => { _panel = null; _stats.Subscribe("panel", false); GC.Collect(); };
        _ = _panel.InitAsync();
    }

    // ================= system events (from MessageWindow) =================
    public void OnDisplayChanged()
    {
        _ui.Post(async _ => { await Task.Delay(800); _island?.Reposition(); _occ.Invalidate(); }, null);
    }
    public void OnSession(bool locked) { _locked = locked; ApplyVisibility(); }
    public void OnDisplayPower(bool on) { _displayOff = !on; ApplyVisibility(); }

    // ================= lifecycle =================
    /// <summary>Turn off autostart and exit (Frost changes nothing else on the system).</summary>
    public void RestoreAndQuit()
    {
        Settings.Apply(new JsonObject { ["general"] = new JsonObject { ["autostart"] = false } });
        DesktopState.SetAutostart(false);
        Quit();
    }

    public void Quit()
    {
        if (_quitting) return;
        _quitting = true;
        _tray.Visible = false;
        try { _panel?.Close(); } catch { }
        try { _island?.Close(); } catch { }
        _occ.Dispose(); _audio.Dispose(); _stats.Dispose(); _privacy.Dispose();
        _tray.Dispose();
        _msg.Dispose();
        ExitThread();
    }

    public static void Log(string s)
    {
        try { File.AppendAllText(Path.Combine(Settings.Data, "cache", "frost.log"), $"{DateTime.Now:yyyy-MM-dd HH:mm:ss} {s}{Environment.NewLine}"); } catch { }
    }
}

/// <summary>Hidden top-level window: hotkey, display changes, lock/unlock, display power, commands from a second instance.</summary>
internal sealed class MessageWindow : NativeWindow, IDisposable
{
    private readonly App _app;
    public static readonly uint OpenPanelMsg = RegisterWindowMessage("FrostDesktop.OpenPanel");
    public static readonly uint CommandMsg = RegisterWindowMessage("FrostDesktop.Command");
    public bool HotkeyOn { get; private set; }

    public MessageWindow(App app)
    {
        _app = app;
        CreateHandle(new CreateParams { Caption = "FrostDesktopMessageWindow", ExStyle = WS_EX_TOOLWINDOW });
        WTSRegisterSessionNotification(Handle, 0);
        var g = GUID_CONSOLE_DISPLAY_STATE;
        RegisterPowerSettingNotification(Handle, ref g, 0);
        SetHotkey(app.Settings.B("general.hotkey"));
    }

    public void SetHotkey(bool on)
    {
        UnregisterHotKey(Handle, 1);
        // Ctrl + Alt + F  → island settings
        HotkeyOn = on && RegisterHotKey(Handle, 1, 0x0002 | 0x0001 | 0x4000 /*CTRL|ALT|NOREPEAT*/, (uint)Keys.F);
    }

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == (int)OpenPanelMsg) _app.ShowPanel();
        else if (m.Msg == (int)CommandMsg) { if ((int)m.WParam == 2) _app.RestoreAndQuit(); else _app.Quit(); }
        else switch (m.Msg)
        {
            case WM_HOTKEY when m.WParam == (IntPtr)1: _app.ShowPanel(); break;
            case WM_DISPLAYCHANGE: _app.OnDisplayChanged(); break;
            case WM_WTSSESSION_CHANGE:
                if ((int)m.WParam == 7) _app.OnSession(true);       // WTS_SESSION_LOCK
                else if ((int)m.WParam == 8) _app.OnSession(false); // WTS_SESSION_UNLOCK
                break;
            case WM_POWERBROADCAST when (int)m.WParam == 0x8013 /*PBT_POWERSETTINGCHANGE*/:
                var ps = System.Runtime.InteropServices.Marshal.PtrToStructure<POWERBROADCAST_SETTING>(m.LParam);
                if (ps.PowerSetting == GUID_CONSOLE_DISPLAY_STATE) _app.OnDisplayPower(ps.Data != 0);
                break;
        }
        base.WndProc(ref m);
    }

    public void Dispose() { UnregisterHotKey(Handle, 1); DestroyHandle(); }
}

/// <summary>Dark tray menu.</summary>
internal sealed class FrostMenuRenderer : ToolStripProfessionalRenderer
{
    public FrostMenuRenderer() : base(new FrostColors()) { RoundedEdges = true; }
    protected override void OnRenderItemCheck(ToolStripItemImageRenderEventArgs e)
    {
        var r = new Rectangle(e.Item.Width - 26, (e.Item.Height - 8) / 2, 8, 8);
        e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        using var b = new SolidBrush(Color.FromArgb(48, 209, 88));
        e.Graphics.FillEllipse(b, r);
    }
    private sealed class FrostColors : ProfessionalColorTable
    {
        public override Color ToolStripDropDownBackground => Color.FromArgb(18, 18, 20);
        public override Color MenuBorder => Color.FromArgb(58, 58, 62);
        public override Color MenuItemBorder => Color.FromArgb(58, 58, 62);
        public override Color MenuItemSelected => Color.FromArgb(44, 44, 48);
        public override Color MenuItemSelectedGradientBegin => Color.FromArgb(44, 44, 48);
        public override Color MenuItemSelectedGradientEnd => Color.FromArgb(44, 44, 48);
        public override Color SeparatorDark => Color.FromArgb(50, 50, 54);
        public override Color SeparatorLight => Color.FromArgb(50, 50, 54);
        public override Color ImageMarginGradientBegin => Color.FromArgb(18, 18, 20);
        public override Color ImageMarginGradientMiddle => Color.FromArgb(18, 18, 20);
        public override Color ImageMarginGradientEnd => Color.FromArgb(18, 18, 20);
        public override Color CheckBackground => Color.Transparent;
        public override Color CheckSelectedBackground => Color.Transparent;
        public override Color CheckPressedBackground => Color.Transparent;
    }
}
