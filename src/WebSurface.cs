using System.Drawing;
using System.Text.Json.Nodes;
using Microsoft.Web.WebView2.Core;
using static Frost.Native;

namespace Frost;

/// <summary>One WebView2 environment shared by the island and the settings window.</summary>
internal static class WebEnv
{
    public const string Host = "frost.local";
    public static readonly string WebRoot = Path.Combine(Settings.Root, "web");
    private static Task<CoreWebView2Environment> _env;

    public static Task<CoreWebView2Environment> Get() => _env ??= Create();

    private static async Task<CoreWebView2Environment> Create()
    {
        var opts = new CoreWebView2EnvironmentOptions
        {
            AdditionalBrowserArguments = string.Join(' ',
                "--autoplay-policy=no-user-gesture-required",      // the timer chime plays without a click
                "--disable-background-networking",                 // no component updates / safe-browsing pings
                "--disable-component-update",
                "--disable-sync",
                "--disable-features=msSmartScreenProtection,msWebOOUI,msPdfOOUI,SpareRendererForSitePerProcess,CalculateNativeWinOcclusion,BackForwardCache",
                "--process-per-site"),                             // island + settings share one renderer
            IsCustomCrashReportingEnabled = true,
        };
        var dataDir = Path.Combine(Settings.Data, "cache", "webview");
        return await CoreWebView2Environment.CreateAsync(null, dataDir, opts);
    }

    public static void Configure(CoreWebView2 wv, bool interactive)
    {
        var s = wv.Settings;
        s.AreDefaultContextMenusEnabled = false;
        s.AreDevToolsEnabled = Environment.GetEnvironmentVariable("FROST_DEVTOOLS") == "1";
        s.IsStatusBarEnabled = false;
        s.IsZoomControlEnabled = false;
        s.IsPinchZoomEnabled = false;
        s.AreBrowserAcceleratorKeysEnabled = false;
        s.IsSwipeNavigationEnabled = false;
        s.IsPasswordAutosaveEnabled = false;
        s.IsGeneralAutofillEnabled = false;
        s.AreHostObjectsAllowed = false;
        s.IsBuiltInErrorPageEnabled = false;
        wv.SetVirtualHostNameToFolderMapping(Host, WebRoot, CoreWebView2HostResourceAccessKind.Allow);
        // Never navigate away from our own pages.
        wv.NavigationStarting += (_, e) => { if (!e.Uri.StartsWith($"https://{Host}/")) e.Cancel = true; };
        wv.NewWindowRequested += (_, e) => e.Handled = true;
    }
}

/// <summary>Common message plumbing for a page.</summary>
internal abstract class SurfaceBase
{
    public CoreWebView2 Web { get; protected set; }
    public bool Ready { get; protected set; }
    public event Action<SurfaceBase, JsonObject> Received;
    protected readonly App App;
    protected SurfaceBase(App app) => App = app;

    protected void Hook(CoreWebView2 wv)
    {
        Web = wv;
        wv.WebMessageReceived += (_, e) =>
        {
            try
            {
                if (JsonNode.Parse(e.WebMessageAsJson) is JsonObject o)
                {
                    if ((string)o["type"] == "ready") { Ready = true; App.SendInitialState(this); }
                    Received?.Invoke(this, o);
                }
            }
            catch { }
        };
        wv.ProcessFailed += (_, e) =>
        {
            // Renderer crashed or was killed: reload rather than leave a blank surface.
            if (e.ProcessFailedKind is CoreWebView2ProcessFailedKind.RenderProcessExited or CoreWebView2ProcessFailedKind.RenderProcessUnresponsive)
                try { wv.Reload(); } catch { }
        };
    }

    public void Post(JsonObject msg)
    {
        if (!Ready || Web == null) return;
        try { Web.PostWebMessageAsJson(msg.ToJsonString()); } catch { }
    }

    public abstract void Close();
}

/// <summary>
/// Transparent, per-pixel-alpha top-level window: WebView2 rendered through DirectComposition (visual hosting).
/// No redirection bitmap, so the transparent parts genuinely show what's beneath.
/// </summary>
internal abstract class CompositionSurface : SurfaceBase
{
    protected readonly SurfaceForm Form;
    protected CoreWebView2CompositionController Controller;
    private IDCompositionDevice _dev;
    private IDCompositionTarget _target;
    private IDCompositionVisual _visual;
    private bool _tracking;
    private IntPtr _cursor;
    private readonly bool _clickThrough;
    private static readonly bool Debug = Environment.GetEnvironmentVariable("FROST_DEBUG") == "1";

    protected CompositionSurface(App app, bool clickThrough) : base(app)
    {
        _clickThrough = clickThrough;
        int ex = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP;
        if (clickThrough) ex |= WS_EX_LAYERED | WS_EX_TRANSPARENT;
        Form = new SurfaceForm(ex, WndProcHook);
    }

    public IntPtr Handle => Form.Handle;

    protected async Task InitAsync(string page, Rectangle screenBounds)
    {
        Form.Bounds = screenBounds;
        var h = Form.Handle;
        if (_clickThrough) SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA);

        _dev = DComp.CreateDevice();
        Marshal(_dev.CreateTargetForHwnd(h, true, out _target));
        Marshal(_dev.CreateVisual(out _visual));
        Marshal(_target.SetRoot(_visual));

        var env = await WebEnv.Get();
        Controller = await env.CreateCoreWebView2CompositionControllerAsync(h);
        Controller.DefaultBackgroundColor = Color.Transparent;
        Controller.RootVisualTarget = _visual;
        Controller.Bounds = new Rectangle(0, 0, screenBounds.Width, screenBounds.Height);
        Controller.IsVisible = true;
        Controller.CursorChanged += (_, _) => { _cursor = Controller.Cursor; SetCursor(_cursor); };
        Marshal(_dev.Commit());

        WebEnv.Configure(Controller.CoreWebView2, !_clickThrough);
        Hook(Controller.CoreWebView2);
        Controller.CoreWebView2.Navigate($"https://{WebEnv.Host}/{page}");
        ShowNoActivate();
    }

    private static void Marshal(int hr) => System.Runtime.InteropServices.Marshal.ThrowExceptionForHR(hr);

    protected void ShowNoActivate()
    {
        if (!Form.Visible) Form.Show();
        SetWindowPos(Form.Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }

    public void SetBounds(Rectangle r)
    {
        SetWindowPos(Form.Handle, HWND_TOPMOST, r.X, r.Y, r.Width, r.Height, SWP_NOACTIVATE);
        if (Controller != null) Controller.Bounds = new Rectangle(0, 0, r.Width, r.Height);
    }

    public bool Visible
    {
        get => Controller?.IsVisible ?? false;
        set
        {
            if (Controller == null) return;
            if (value) { Controller.IsVisible = true; ShowNoActivate(); }
            else { Form.Hide(); Controller.IsVisible = false; }
        }
    }

    // Forward mouse input into the composition-hosted WebView.
    private bool WndProcHook(ref Message m)
    {
        if (Controller == null) return false;
        switch (m.Msg)
        {
            case WM_MOUSEACTIVATE: m.Result = (IntPtr)MA_NOACTIVATE; return true;
            case WM_SETCURSOR when _cursor != IntPtr.Zero && ((int)m.LParam & 0xFFFF) == 1 /*HTCLIENT*/:
                SetCursor(_cursor); m.Result = (IntPtr)1; return true;
            case WM_MOUSEMOVE: case WM_LBUTTONDOWN: case WM_LBUTTONUP: case WM_LBUTTONDBLCLK:
            case WM_RBUTTONDOWN: case WM_RBUTTONUP: case WM_MBUTTONDOWN: case WM_MBUTTONUP:
            case WM_MOUSEWHEEL: case WM_MOUSEHWHEEL: case WM_MOUSELEAVE:
                ForwardMouse(ref m); return true;
        }
        return false;
    }

    private void ForwardMouse(ref Message m)
    {
        var kind = (CoreWebView2MouseEventKind)m.Msg;
        uint mouseData = 0;
        var pt = new POINT { X = (short)((long)m.LParam & 0xFFFF), Y = (short)(((long)m.LParam >> 16) & 0xFFFF) };
        if (m.Msg is WM_MOUSEWHEEL or WM_MOUSEHWHEEL)
        {
            mouseData = (uint)(short)(((long)m.WParam >> 16) & 0xFFFF);
            ScreenToClient(Form.Handle, ref pt); // wheel messages carry screen coordinates
        }
        if (m.Msg == WM_MOUSELEAVE) { EndHover(); return; }
        if (!_tracking)
        {
            var t = new TRACKMOUSEEVENT { cbSize = System.Runtime.InteropServices.Marshal.SizeOf<TRACKMOUSEEVENT>(), dwFlags = TME_LEAVE, hwndTrack = Form.Handle };
            TrackMouseEvent(ref t);
            _tracking = true;
            // WM_MOUSELEAVE is unreliable for NOREDIRECTIONBITMAP + region windows; confirm the exit ourselves.
            // This timer only runs while the pointer is over the island.
            _leaveTimer ??= CreateLeaveTimer();
            _leaveTimer.Start();
        }
        var keys = (CoreWebView2MouseEventVirtualKeys)((long)m.WParam & 0xFFFF);
        if (m.Msg is WM_MOUSEWHEEL or WM_MOUSEHWHEEL) keys = CoreWebView2MouseEventVirtualKeys.None;
        try { Controller.SendMouseInput(kind, keys, mouseData, new Point(pt.X, pt.Y)); } catch { }
    }

    private System.Windows.Forms.Timer _leaveTimer;
    private System.Windows.Forms.Timer CreateLeaveTimer()
    {
        var t = new System.Windows.Forms.Timer { Interval = 120 };
        t.Tick += (_, _) =>
        {
            GetCursorPos(out var p);
            var under = WindowFromPoint(p);
            if (under != Form.Handle) EndHover();
        };
        return t;
    }

    private void EndHover()
    {
        _leaveTimer?.Stop();
        if (!_tracking) return;
        _tracking = false;
        try { Controller?.SendMouseInput(CoreWebView2MouseEventKind.Leave, CoreWebView2MouseEventVirtualKeys.None, 0, new Point(0, 0)); } catch { }
    }

    public override void Close()
    {
        Ready = false;
        _leaveTimer?.Dispose();
        try { Controller?.Close(); } catch { }
        Controller = null;
        Form.Dispose();
        _target = null; _visual = null; _dev = null;
    }
}

/// <summary>Bare top-level window with caller-chosen extended styles and a WndProc hook.</summary>
internal sealed class SurfaceForm : Form
{
    public delegate bool Hook(ref Message m);
    private readonly int _ex;
    private readonly Hook _hook;
    private readonly int _style;

    public SurfaceForm(int exStyle, Hook hook, int style = WS_POPUP | WS_CLIPCHILDREN)
    {
        _ex = exStyle; _hook = hook; _style = style;
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        BackColor = Color.Black;
        AutoScaleMode = AutoScaleMode.None;
        Text = "WinIsland";
    }

    protected override CreateParams CreateParams
    {
        get { var cp = base.CreateParams; cp.ExStyle = _ex; cp.Style = _style; return cp; }
    }

    protected override bool ShowWithoutActivation => true;
    protected override void OnPaintBackground(PaintEventArgs e) { /* WebView covers everything */ }

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == WM_ERASEBKGND) { m.Result = (IntPtr)1; return; }
        if (_hook != null && _hook(ref m)) return;
        base.WndProc(ref m);
    }
}
