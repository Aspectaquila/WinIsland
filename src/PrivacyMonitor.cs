using System.Runtime.InteropServices;
using Microsoft.Win32;
using Microsoft.Win32.SafeHandles;

namespace Frost;

/// <summary>
/// macOS-style privacy indicators: is any app using the camera or microphone right now?
/// Windows records this under CapabilityAccessManager\ConsentStore (LastUsedTimeStop == 0 while in use).
/// We wait on registry change notifications, so this costs nothing while nothing changes.
/// </summary>
internal sealed class PrivacyMonitor : IDisposable
{
    private const string Base = @"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\";
    private readonly SynchronizationContext _ui;
    private readonly List<Watch> _watches = new();
    public bool Camera { get; private set; }
    public bool Mic { get; private set; }
    public string CameraApp { get; private set; } = "";
    public string MicApp { get; private set; } = "";
    public event Action Changed;

    public PrivacyMonitor(SynchronizationContext ui)
    {
        _ui = ui;
        foreach (var cap in new[] { "webcam", "microphone" })
        {
            try { _watches.Add(new Watch(Base + cap, () => _ui.Post(_ => Evaluate(), null))); } catch { }
        }
        Evaluate();
    }

    private void Evaluate()
    {
        var (cam, camApp) = InUse("webcam");
        var (mic, micApp) = InUse("microphone");
        if (cam == Camera && mic == Mic && camApp == CameraApp && micApp == MicApp) return;
        Camera = cam; Mic = mic; CameraApp = camApp; MicApp = micApp;
        Changed?.Invoke();
    }

    private static (bool, string) InUse(string cap)
    {
        try
        {
            using var root = Registry.CurrentUser.OpenSubKey(Base + cap);
            if (root == null) return (false, "");
            HashSet<string> running = null; // lazily built, only when a candidate exists
            foreach (var name in root.GetSubKeyNames())
            {
                using var k = root.OpenSubKey(name);
                if (name == "NonPackaged")
                {
                    foreach (var sub in k.GetSubKeyNames())
                    {
                        using var s = k.OpenSubKey(sub);
                        if (!Active(s)) continue;
                        // Windows leaves "in use" behind when an app crashes or updates; confirm it's actually running.
                        running ??= RunningImages();
                        if (running.Contains(sub.Replace('#', '\\'))) return (true, Friendly(sub));
                    }
                }
                else if (Active(k))
                {
                    running ??= RunningImages();
                    var pkg = name.Split('_')[0];
                    if (running.Any(p => p.Contains(@"\WindowsApps\" + pkg + "_", StringComparison.OrdinalIgnoreCase))) return (true, Friendly(name));
                }
            }
        }
        catch { }
        return (false, "");
    }

    [DllImport("kernel32.dll")] private static extern IntPtr OpenProcess(int access, bool inherit, int pid);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern bool QueryFullProcessImageName(IntPtr h, int flags, System.Text.StringBuilder sb, ref int size);
    [DllImport("kernel32.dll")] private static extern bool CloseHandle(IntPtr h);

    private static HashSet<string> RunningImages()
    {
        var set = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var p in System.Diagnostics.Process.GetProcesses())
        {
            var h = OpenProcess(0x1000 /*PROCESS_QUERY_LIMITED_INFORMATION*/, false, p.Id);
            if (h != IntPtr.Zero)
            {
                var sb = new System.Text.StringBuilder(1024); int n = sb.Capacity;
                if (QueryFullProcessImageName(h, 0, sb, ref n)) set.Add(sb.ToString());
                CloseHandle(h);
            }
            p.Dispose();
        }
        return set;
    }

    private static bool Active(RegistryKey k) =>
        k != null && k.GetValue("LastUsedTimeStop") is long stop && stop == 0 && k.GetValue("LastUsedTimeStart") is long start && start > 0;

    private static string Friendly(string id)
    {
        var n = id.Split('#', '_').Last(p => p.Length > 0);
        if (n.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)) n = n[..^4];
        return n.Length > 24 ? n[..24] : n;
    }

    public void Dispose() { foreach (var w in _watches) w.Dispose(); }

    /// <summary>One registry subtree change notification, re-armed after each signal.</summary>
    private sealed class Watch : IDisposable
    {
        [DllImport("advapi32.dll")] private static extern int RegNotifyChangeKeyValue(SafeRegistryHandle h, bool subtree, int filter, SafeWaitHandle ev, bool async);
        private readonly RegistryKey _key;
        private readonly AutoResetEvent _ev = new(false);
        private readonly RegisteredWaitHandle _wait;
        private readonly Action _onChange;
        private bool _disposed;

        public Watch(string path, Action onChange)
        {
            _onChange = onChange;
            _key = Registry.CurrentUser.OpenSubKey(path) ?? throw new InvalidOperationException();
            Arm();
            _wait = ThreadPool.RegisterWaitForSingleObject(_ev, (_, _) => { if (_disposed) return; Arm(); _onChange(); }, null, Timeout.Infinite, false);
        }

        // REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET
        private void Arm() => RegNotifyChangeKeyValue(_key.Handle, true, 0x1 | 0x4, _ev.SafeWaitHandle, true);

        public void Dispose() { _disposed = true; _wait?.Unregister(null); _key.Dispose(); _ev.Dispose(); }
    }
}
