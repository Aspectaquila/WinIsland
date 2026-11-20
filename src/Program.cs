using static Frost.Native;

namespace Frost;

internal static class Program
{
    [System.Runtime.InteropServices.DllImport("user32.dll")] private static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);

    [STAThread]
    private static void Main(string[] args)
    {
        using var mutex = new Mutex(true, @"Local\FrostDesktop.Singleton", out bool first);
        int cmd = args.Contains("--restore") ? 2 : args.Contains("--quit") ? 1 : 0;
        if (!first)
        {
            // Already running: forward the request (open settings / quit / uninstall) to that instance.
            var h = FindWindow(null, "FrostDesktopMessageWindow");
            if (h != IntPtr.Zero) PostMessage(h, cmd == 0 ? MessageWindow.OpenPanelMsg : MessageWindow.CommandMsg, (IntPtr)cmd, IntPtr.Zero);
            return;
        }
        if (cmd != 0)
        {
            if (cmd == 2) DesktopState.SetAutostart(false); // not running: just remove autostart
            return;
        }

        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.SetColorMode(SystemColorMode.Dark);
        Application.ThreadException += (_, e) => App.Log("UI exception: " + e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) => App.Log("fatal: " + e.ExceptionObject);

        // Install the UI sync context *before* any async work so every continuation returns to the UI thread.
        SynchronizationContext.SetSynchronizationContext(new WindowsFormsSynchronizationContext());
        var app = new App();
        if (args.Contains("--panel")) app.ShowPanel();
        Application.Run(app);
    }
}
