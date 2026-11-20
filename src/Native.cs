using System.Runtime.InteropServices;
using System.Text;

namespace Frost;

internal static class Native
{
    public const int GWL_STYLE = -16, GWL_EXSTYLE = -20;
    public const int WS_CHILD = 0x40000000, WS_POPUP = unchecked((int)0x80000000), WS_VISIBLE = 0x10000000,
        WS_CLIPCHILDREN = 0x02000000, WS_CLIPSIBLINGS = 0x04000000, WS_MAXIMIZE = 0x01000000, WS_MINIMIZE = 0x20000000,
        WS_CAPTION = 0x00C00000;
    public const int WS_EX_TOPMOST = 0x8, WS_EX_TRANSPARENT = 0x20, WS_EX_TOOLWINDOW = 0x80, WS_EX_LAYERED = 0x80000,
        WS_EX_NOACTIVATE = 0x08000000, WS_EX_NOREDIRECTIONBITMAP = 0x00200000, WS_EX_APPWINDOW = 0x40000;
    public const uint SWP_NOSIZE = 1, SWP_NOMOVE = 2, SWP_NOZORDER = 4, SWP_NOACTIVATE = 0x10, SWP_SHOWWINDOW = 0x40,
        SWP_FRAMECHANGED = 0x20, SWP_NOOWNERZORDER = 0x200;
    public static readonly IntPtr HWND_TOPMOST = new(-1), HWND_BOTTOM = new(1);
    public const int LWA_ALPHA = 2;
    public const int WM_MOUSEMOVE = 0x200, WM_LBUTTONDOWN = 0x201, WM_LBUTTONUP = 0x202, WM_LBUTTONDBLCLK = 0x203,
        WM_RBUTTONDOWN = 0x204, WM_RBUTTONUP = 0x205, WM_MBUTTONDOWN = 0x207, WM_MBUTTONUP = 0x208,
        WM_MOUSEWHEEL = 0x20A, WM_MOUSEHWHEEL = 0x20E, WM_MOUSELEAVE = 0x2A3, WM_SETCURSOR = 0x20,
        WM_MOUSEACTIVATE = 0x21, WM_NCHITTEST = 0x84, WM_DISPLAYCHANGE = 0x7E, WM_DPICHANGED = 0x2E0,
        WM_HOTKEY = 0x312, WM_WTSSESSION_CHANGE = 0x2B1, WM_POWERBROADCAST = 0x218, WM_SETTINGCHANGE = 0x1A,
        WM_ERASEBKGND = 0x14;
    public const int MA_NOACTIVATE = 3;
    public const int SM_CXVIRTUALSCREEN = 78;

    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; public int Width => Right - Left; public int Height => Bottom - Top; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string name);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string name);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll")] public static extern IntPtr SetParent(IntPtr child, IntPtr parent);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hWnd, uint flags);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int idx);
    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")] public static extern IntPtr SetWindowLongPtr(IntPtr hWnd, int idx, IntPtr val);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetLayeredWindowAttributes(IntPtr hWnd, uint key, byte alpha, int flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern int MapWindowPoints(IntPtr from, IntPtr to, ref RECT r, int n);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr hWnd, ref POINT p);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd, StringBuilder sb, int max);
    [DllImport("user32.dll")] public static extern IntPtr GetShellWindow();
    [DllImport("user32.dll")] public static extern IntPtr GetDesktopWindow();
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
    [DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int l, int t, int r, int b);
    [DllImport("gdi32.dll")] public static extern IntPtr CreateRoundRectRgn(int l, int t, int r, int b, int w, int h);
    [DllImport("user32.dll")] public static extern int SetWindowRgn(IntPtr hWnd, IntPtr rgn, bool redraw);
    [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr o);
    [DllImport("user32.dll")] public static extern IntPtr SetCursor(IntPtr c);
    [DllImport("user32.dll")] public static extern bool RegisterHotKey(IntPtr hWnd, int id, uint mods, uint vk);
    [DllImport("user32.dll")] public static extern bool UnregisterHotKey(IntPtr hWnd, int id);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern uint RegisterWindowMessage(string s);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint action, uint param, string pv, uint winIni);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hWnd, uint flags);

    [StructLayout(LayoutKind.Sequential)] public struct TRACKMOUSEEVENT { public int cbSize; public uint dwFlags; public IntPtr hwndTrack; public uint dwHoverTime; }
    [DllImport("user32.dll")] public static extern bool TrackMouseEvent(ref TRACKMOUSEEVENT t);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    public const uint TME_LEAVE = 2;

    // WinEvents
    public delegate void WinEventDelegate(IntPtr hook, uint evt, IntPtr hwnd, int idObject, int idChild, uint thread, uint time);
    [DllImport("user32.dll")] public static extern IntPtr SetWinEventHook(uint min, uint max, IntPtr mod, WinEventDelegate cb, uint pid, uint tid, uint flags);
    [DllImport("user32.dll")] public static extern bool UnhookWinEvent(IntPtr hook);
    public const uint WINEVENT_OUTOFCONTEXT = 0, WINEVENT_SKIPOWNPROCESS = 2;
    public const uint EVENT_SYSTEM_FOREGROUND = 3, EVENT_SYSTEM_MOVESIZEEND = 0xB, EVENT_SYSTEM_MINIMIZESTART = 0x16,
        EVENT_SYSTEM_MINIMIZEEND = 0x17, EVENT_OBJECT_DESTROY = 0x8001, EVENT_OBJECT_SHOW = 0x8002, EVENT_OBJECT_HIDE = 0x8003,
        EVENT_OBJECT_LOCATIONCHANGE = 0x800B, EVENT_OBJECT_CLOAKED = 0x8017, EVENT_OBJECT_UNCLOAKED = 0x8018;

    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hWnd, int attr, out int val, int size);
    [DllImport("dwmapi.dll")] public static extern int DwmSetWindowAttribute(IntPtr hWnd, int attr, ref int val, int size);
    public const int DWMWA_CLOAKED = 14, DWMWA_USE_IMMERSIVE_DARK_MODE = 20, DWMWA_WINDOW_CORNER_PREFERENCE = 33,
        DWMWA_BORDER_COLOR = 34, DWMWA_CAPTION_COLOR = 35, DWMWA_SYSTEMBACKDROP_TYPE = 38;

    [DllImport("shell32.dll")] public static extern int SHQueryUserNotificationState(out int state);
    public const int QUNS_BUSY = 2, QUNS_RUNNING_D3D_FULL_SCREEN = 3, QUNS_PRESENTATION_MODE = 4;

    [DllImport("wtsapi32.dll")] public static extern bool WTSRegisterSessionNotification(IntPtr hWnd, int flags);
    [DllImport("user32.dll")] public static extern IntPtr RegisterPowerSettingNotification(IntPtr h, ref Guid g, int flags);
    public static Guid GUID_CONSOLE_DISPLAY_STATE = new("6FE69556-704A-47A0-8F24-C28D936FDA47");
    [StructLayout(LayoutKind.Sequential, Pack = 4)] public struct POWERBROADCAST_SETTING { public Guid PowerSetting; public int DataLength; public int Data; }

    // System stats
    [StructLayout(LayoutKind.Sequential)] public struct FILETIME { public uint Low, High; public ulong Value => ((ulong)High << 32) | Low; }
    [DllImport("kernel32.dll")] public static extern bool GetSystemTimes(out FILETIME idle, out FILETIME kernel, out FILETIME user);
    [StructLayout(LayoutKind.Sequential)] public struct MEMORYSTATUSEX { public uint dwLength, dwMemoryLoad; public ulong ullTotalPhys, ullAvailPhys, ullTotalPageFile, ullAvailPageFile, ullTotalVirtual, ullAvailVirtual, ullAvailExtendedVirtual; }
    [DllImport("kernel32.dll")] public static extern bool GlobalMemoryStatusEx(ref MEMORYSTATUSEX m);
    [DllImport("kernel32.dll")] public static extern bool SetProcessWorkingSetSize(IntPtr proc, IntPtr min, IntPtr max);
    [DllImport("kernel32.dll")] public static extern IntPtr GetCurrentProcess();
    [StructLayout(LayoutKind.Sequential)] public struct SYSTEM_POWER_STATUS { public byte ACLineStatus, BatteryFlag, BatteryLifePercent, SystemStatusFlag; public int BatteryLifeTime, BatteryFullLifeTime; }
    [DllImport("kernel32.dll")] public static extern bool GetSystemPowerStatus(out SYSTEM_POWER_STATUS s);

    public static string ClassName(IntPtr h) { var sb = new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
    public static bool IsCloaked(IntPtr h) => DwmGetWindowAttribute(h, DWMWA_CLOAKED, out int c, 4) == 0 && c != 0;
    public static int ExStyle(IntPtr h) => (int)GetWindowLongPtr(h, GWL_EXSTYLE);
    public static int Style(IntPtr h) => (int)GetWindowLongPtr(h, GWL_STYLE);
}

// ---------------- DirectComposition (minimal vtable-accurate subset) ----------------
[ComImport, Guid("C37EA93A-E7AA-450D-B16F-9746CB0407F3"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IDCompositionDevice
{
    [PreserveSig] int Commit();
    [PreserveSig] int WaitForCommitCompletion();
    [PreserveSig] int GetFrameStatistics(IntPtr stats);
    [PreserveSig] int CreateTargetForHwnd(IntPtr hwnd, [MarshalAs(UnmanagedType.Bool)] bool topmost, out IDCompositionTarget target);
    [PreserveSig] int CreateVisual(out IDCompositionVisual visual);
}

[ComImport, Guid("eacdd04c-117e-4e17-88f4-d1b12b0e3d89"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IDCompositionTarget
{
    [PreserveSig] int SetRoot(IDCompositionVisual visual);
}

[ComImport, Guid("4d93059d-097b-4651-9a60-f0f25116e2f3"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IDCompositionVisual { }

internal static class DComp
{
    [DllImport("dcomp.dll")] private static extern int DCompositionCreateDevice2(IntPtr renderingDevice, ref Guid iid, [MarshalAs(UnmanagedType.IUnknown)] out object device);
    public static IDCompositionDevice CreateDevice()
    {
        var iid = typeof(IDCompositionDevice).GUID;
        Marshal.ThrowExceptionForHR(DCompositionCreateDevice2(IntPtr.Zero, ref iid, out var dev));
        return (IDCompositionDevice)dev;
    }
}
