using System.Drawing;
using static Frost.Native;

namespace Frost;

/// <summary>
/// Works out, per monitor, whether the desktop is visible, covered by a maximized window, or covered by a
/// fullscreen app. Entirely event-driven (WinEvent hooks) and debounced. No polling.
/// </summary>
internal sealed class OcclusionMonitor : IDisposable
{
    public enum State { Free = 0, Maximized = 1, Fullscreen = 2 }

    public sealed record MonitorInfo(Rectangle Bounds, Rectangle Work, bool Primary, State State);

    public event Action<IReadOnlyList<MonitorInfo>> Changed;
    public IReadOnlyList<MonitorInfo> Current { get; private set; } = Array.Empty<MonitorInfo>();

    private readonly List<IntPtr> _hooks = new();
    private readonly WinEventDelegate _cb; // kept alive for the hooks' lifetime
    private readonly System.Windows.Forms.Timer _debounce = new() { Interval = 220 };
    private readonly uint _myPid = (uint)Environment.ProcessId;
    private bool _forced;
    private static readonly bool Debug = Environment.GetEnvironmentVariable("FROST_DEBUG") == "1";

    public OcclusionMonitor()
    {
        _cb = OnWinEvent;
        _debounce.Tick += (_, _) => { _debounce.Stop(); Evaluate(); };
        void hook(uint a, uint b) => _hooks.Add(SetWinEventHook(a, b, IntPtr.Zero, _cb, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS));
        hook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND);
        hook(EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND);
        hook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND);
        hook(EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE);          // destroy, show, hide
        hook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE);
        hook(EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED);     // virtual desktop switches
        Evaluate();
    }

    private void OnWinEvent(IntPtr hook, uint evt, IntPtr hwnd, int idObject, int idChild, uint thread, uint time)
    {
        if (idObject != 0 || idChild != 0 || hwnd == IntPtr.Zero) return; // only whole windows
        if (evt is EVENT_OBJECT_LOCATIONCHANGE or EVENT_OBJECT_SHOW or EVENT_OBJECT_HIDE or EVENT_OBJECT_DESTROY)
        {
            // These fire constantly for child controls; only top-level windows matter.
            if (evt != EVENT_OBJECT_DESTROY && GetAncestor(hwnd, 1 /*GA_PARENT*/) != GetDesktopWindow()) return;
        }
        if (!_debounce.Enabled) _debounce.Start();
    }

    public void Invalidate() { _forced = true; _debounce.Stop(); _debounce.Start(); }

    private void Evaluate()
    {
        var screens = Screen.AllScreens;
        var states = new State[screens.Length];
        var shell = GetShellWindow();

        EnumWindows((h, _) =>
        {
            if (!IsWindowVisible(h) || IsIconic(h) || IsCloaked(h)) return true;
            GetWindowThreadProcessId(h, out var pid);
            if (pid == _myPid || h == shell) return true;
            int ex = ExStyle(h);
            if ((ex & WS_EX_TRANSPARENT) != 0 && (ex & WS_EX_LAYERED) != 0) return true; // click-through overlays (Steam/NVIDIA/etc.)
            if ((ex & WS_EX_TOOLWINDOW) != 0 && (ex & WS_EX_APPWINDOW) == 0) return true;
            var cls = ClassName(h);
            if (cls is "Progman" or "WorkerW" or "Shell_TrayWnd" or "Shell_SecondaryTrayWnd" or "Windows.UI.Core.CoreWindow"
                or "XamlExplorerHostIslandWindow" or "TopLevelWindowForOverflowXamlIsland") return true;
            if (!GetWindowRect(h, out var r)) return true;
            var wr = Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom);
            for (int i = 0; i < screens.Length; i++)
            {
                var b = screens[i].Bounds; var w = screens[i].WorkingArea;
                var before = states[i];
                if (wr.Contains(b)) states[i] = State.Fullscreen;
                else if (states[i] == State.Free && Covers(wr, w)) states[i] = State.Maximized;
                if (states[i] != before && Debug) App.Log($"occlusion: mon{i} {states[i]} by {cls} pid={pid} ex={ex:X} rect={wr}");
            }
            return true;
        }, IntPtr.Zero);

        // Exclusive fullscreen D3D apps may not report a covering window rect reliably.
        if (SHQueryUserNotificationState(out int q) == 0 && q is QUNS_RUNNING_D3D_FULL_SCREEN or QUNS_PRESENTATION_MODE)
        {
            var fg = GetForegroundWindow();
            var mon = Screen.FromHandle(fg);
            for (int i = 0; i < screens.Length; i++) if (screens[i].DeviceName == mon.DeviceName) states[i] = State.Fullscreen;
        }

        var list = screens.Select((s, i) => new MonitorInfo(s.Bounds, s.WorkingArea, s.Primary, states[i])).ToList();
        bool changed = _forced || list.Count != Current.Count || list.Where((m, i) => m != Current[i]).Any();
        _forced = false;
        Current = list;
        if (changed) Changed?.Invoke(list);
    }

    // Window covers the work area (allowing the few px of invisible resize border that maximized windows overhang).
    private static bool Covers(Rectangle win, Rectangle work)
    {
        var shrunk = Rectangle.Inflate(work, -12, -12);
        return win.Contains(shrunk);
    }

    public void Dispose()
    {
        foreach (var h in _hooks) UnhookWinEvent(h);
        _hooks.Clear();
        _debounce.Dispose();
    }
}
