using System.Drawing;
using System.Runtime.InteropServices;
using Microsoft.Web.WebView2.Core;
using static Frost.Native;

namespace Frost;

/// <summary>The Control Panel: a normal window (Mica + dark title bar) hosting panel.html. Fully unloaded when closed.</summary>
internal sealed class PanelSurface : SurfaceBase
{
    private PanelForm _form;
    private CoreWebView2Controller _ctl;
    public event Action Closed;

    public PanelSurface(App app) : base(app) { }

    public async Task InitAsync()
    {
        _form = new PanelForm();
        var wa = Screen.PrimaryScreen.WorkingArea;
        double sc = IslandSurface.MonitorScale(Screen.PrimaryScreen.Bounds);
        int w = (int)(Math.Min(1160, wa.Width / sc - 40) * sc), h = (int)(Math.Min(820, wa.Height / sc - 40) * sc);
        _form.Bounds = new Rectangle(wa.X + (wa.Width - w) / 2, wa.Y + (wa.Height - h) / 2, w, h);
        _form.FormClosed += (_, _) => { Ready = false; try { _ctl?.Close(); } catch { } _ctl = null; Closed?.Invoke(); };
        _form.Resize += (_, _) => { if (_ctl != null) _ctl.Bounds = _form.ClientRectangle; };
        _form.Show();

        var env = await WebEnv.Get();
        _ctl = await env.CreateCoreWebView2ControllerAsync(_form.Handle);
        _ctl.DefaultBackgroundColor = Color.Transparent; // let Mica tint through the translucent page
        _ctl.Bounds = _form.ClientRectangle;
        WebEnv.Configure(_ctl.CoreWebView2, true);
        _ctl.CoreWebView2.Settings.AreBrowserAcceleratorKeysEnabled = false;
        Hook(_ctl.CoreWebView2);
        _ctl.CoreWebView2.Navigate($"https://{WebEnv.Host}/panel.html");
        _ctl.MoveFocus(CoreWebView2MoveFocusReason.Programmatic);
    }

    public void Activate()
    {
        if (_form == null) return;
        if (_form.WindowState == FormWindowState.Minimized) _form.WindowState = FormWindowState.Normal;
        _form.Activate();
    }

    public override void Close() => _form?.Close();

    /// <summary>Match the title bar to the page's look (#RRGGBB).</summary>
    public void SetCaption(string hex)
    {
        if (_form == null || hex is not { Length: 7 } || hex[0] != '#') return;
        try
        {
            int r = Convert.ToInt32(hex.Substring(1, 2), 16), g = Convert.ToInt32(hex.Substring(3, 2), 16), b = Convert.ToInt32(hex.Substring(5, 2), 16);
            int colorref = r | (g << 8) | (b << 16);
            DwmSetWindowAttribute(_form.Handle, DWMWA_CAPTION_COLOR, ref colorref, 4);
        }
        catch { }
    }
}

internal sealed class PanelForm : Form
{
    [StructLayout(LayoutKind.Sequential)] private struct MARGINS { public int l, r, t, b; }
    [DllImport("dwmapi.dll")] private static extern int DwmExtendFrameIntoClientArea(IntPtr h, ref MARGINS m);

    public PanelForm()
    {
        Text = "WinIsland — Settings";
        StartPosition = FormStartPosition.Manual;
        BackColor = Color.Black;
        MinimumSize = new Size(940, 640);
        AutoScaleMode = AutoScaleMode.None;
        var ico = Path.Combine(AppContext.BaseDirectory, "frost.ico");
        if (File.Exists(ico)) Icon = new Icon(ico);
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        int on = 1, mica = 2 /*DWMSBT_MAINWINDOW*/, round = 2;
        DwmSetWindowAttribute(Handle, DWMWA_USE_IMMERSIVE_DARK_MODE, ref on, 4);
        DwmSetWindowAttribute(Handle, DWMWA_SYSTEMBACKDROP_TYPE, ref mica, 4);
        DwmSetWindowAttribute(Handle, DWMWA_WINDOW_CORNER_PREFERENCE, ref round, 4);
        int caption = 0x001E1C1C; // COLORREF (BGR) for #1C1C1E, matches the settings window
        DwmSetWindowAttribute(Handle, DWMWA_CAPTION_COLOR, ref caption, 4);
        var m = new MARGINS { l = -1, r = -1, t = -1, b = -1 };
        DwmExtendFrameIntoClientArea(Handle, ref m);
    }

    protected override void OnPaintBackground(PaintEventArgs e) => e.Graphics.Clear(Color.Black);
}
