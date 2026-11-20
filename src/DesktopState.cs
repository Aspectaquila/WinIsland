using System.Diagnostics;
using Microsoft.Win32;

namespace Frost;

/// <summary>The only things WinIsland touches outside its own folder: one autostart entry. Plus two read-only checks.</summary>
internal static class DesktopState
{
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string RunName = "FrostDesktop";

    public static void SetAutostart(bool on)
    {
        using var k = Registry.CurrentUser.OpenSubKey(RunKey, true);
        if (on) k.SetValue(RunName, $"\"{Environment.ProcessPath}\" --autostart");
        else if (k.GetValue(RunName) != null) k.DeleteValue(RunName);
    }

    /// <summary>Is the Windhawk "Dynamic Island for Windows" mod also running (two islands in the same spot)?</summary>
    public static bool WindhawkIslandActive()
    {
        try
        {
            using var k = Registry.LocalMachine.OpenSubKey(@"SOFTWARE\Windhawk\Engine\Mods\dynamic-island-for-windows");
            return k != null && Convert.ToInt32(k.GetValue("Disabled", 0)) == 0;
        }
        catch { return false; }
    }

    public static bool FluentFlyoutRunning => Process.GetProcessesByName("FluentFlyout").Length > 0;
}
