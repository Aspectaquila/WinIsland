using System.Drawing;
using System.Runtime.InteropServices;
using System.Text.Json.Nodes;
using static Frost.Native;

namespace Frost;

/// <summary>
/// Dynamic Island: a fixed transparent canvas at the top-center of the primary monitor.
/// The page reports the pill's current bounds; the window region follows them, so only the pill itself
/// captures the mouse and everything around it stays click-through.
/// </summary>
internal sealed class IslandSurface : CompositionSurface
{
    private const int CanvasW = 760, CanvasH = 520; // CSS px (largest expanded state + glow margin)
    private double _scale = 1;
    private Rectangle _rect;

    public IslandSurface(App app) : base(app, clickThrough: false)
    {
        Received += OnMessage;
    }

    public async Task InitAsync()
    {
        _rect = ComputeRect();
        SetRegion(new Rectangle((_rect.Width - 10) / 2, 0, 10, 10));
        await InitAsync("island.html", _rect);
    }

    private Rectangle ComputeRect()
    {
        var scr = Screen.PrimaryScreen.Bounds;
        _scale = MonitorScale(scr);
        int w = (int)Math.Round(CanvasW * _scale), h = (int)Math.Round(CanvasH * _scale);
        return new Rectangle(scr.X + (scr.Width - w) / 2, scr.Y, w, h);
    }

    public void Reposition()
    {
        _rect = ComputeRect();
        SetBounds(_rect);
    }

    private void OnMessage(SurfaceBase _, JsonObject m)
    {
        if ((string)m["type"] != "bounds") return;
        double x = (double)m["x"], y = (double)m["y"], w = (double)m["w"], h = (double)m["h"];
        SetRegion(new Rectangle((int)Math.Floor(x * _scale), (int)Math.Floor(y * _scale),
                                (int)Math.Ceiling(w * _scale), (int)Math.Ceiling(h * _scale)));
    }

    private void SetRegion(Rectangle r)
    {
        var rgn = CreateRectRgn(r.Left, r.Top, r.Right, r.Bottom);
        if (SetWindowRgn(Handle, rgn, false) == 0) DeleteObject(rgn); // on success the system owns the region
    }

    [DllImport("shcore.dll")] private static extern int GetDpiForMonitor(IntPtr mon, int type, out uint dx, out uint dy);
    [DllImport("user32.dll")] private static extern IntPtr MonitorFromPoint(POINT p, uint flags);

    public static double MonitorScale(Rectangle screenBounds)
    {
        var mon = MonitorFromPoint(new POINT { X = screenBounds.X + 1, Y = screenBounds.Y + 1 }, 2);
        return GetDpiForMonitor(mon, 0, out var dx, out _) == 0 ? dx / 96.0 : 1.0;
    }
}
