using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Reflection;
using System.Threading;
using System.Web.Script.Serialization;
using Microsoft.Win32;

namespace FrostSetup
{
    /// <summary>Everything that touches the system: files, shortcuts, the Apps &amp; features entry and the autostart value.
    /// Per-user only (no admin): HKCU and the user's own folders.</summary>
    internal static class Engine
    {
        public const string AppName = "WinIsland", Version = "2.1.0", Publisher = "WinIsland";
        private const string UninstallKey = @"Software\Microsoft\Windows\CurrentVersion\Uninstall\WinIsland";
        private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run", RunName = "FrostDesktop";

        public static string DefaultDir => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", AppName);
        private static string StartMenuLink => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs), AppName + ".lnk");
        private static string DesktopLink => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory), AppName + ".lnk");
        private static string DataDir => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), AppName);

        public static void Log(string s)
        {
            try { File.AppendAllText(Path.Combine(Path.GetTempPath(), "WinIslandSetup.log"), DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss ") + s + Environment.NewLine); } catch { }
        }

        public static string ExistingInstallDir()
        {
            using (var k = Registry.CurrentUser.OpenSubKey(UninstallKey))
            {
                var d = k?.GetValue("InstallLocation") as string;
                return !string.IsNullOrEmpty(d) && Directory.Exists(d) ? d.TrimEnd('\\') : null;
            }
        }
        public static string ExistingVersion()
        {
            using (var k = Registry.CurrentUser.OpenSubKey(UninstallKey)) return k?.GetValue("DisplayVersion") as string;
        }

        public static long PayloadBytes()
        {
            using (var s = Assembly.GetExecutingAssembly().GetManifestResourceStream("payload.zip"))
            {
                if (s == null) return 0;
                using (var z = new ZipArchive(s, ZipArchiveMode.Read)) return z.Entries.Sum(e => e.Length);
            }
        }

        public static long FreeBytes(string dir)
        {
            try
            {
                var root = Path.GetPathRoot(Path.GetFullPath(dir));
                return new DriveInfo(root).AvailableFreeSpace;
            }
            catch { return -1; }
        }

        /// <summary>null if fine, otherwise a human-readable reason.</summary>
        public static string CheckDir(string dir)
        {
            try
            {
                if (string.IsNullOrWhiteSpace(dir)) return "Choose a folder.";
                var full = Path.GetFullPath(dir);
                if (full.Length < 4) return "Choose a folder, not the root of a drive.";
                var probe = full;
                while (!Directory.Exists(probe)) probe = Path.GetDirectoryName(probe);
                var test = Path.Combine(probe, ".frost-write-test-" + Guid.NewGuid().ToString("N"));
                File.WriteAllText(test, ""); File.Delete(test);
                return null;
            }
            catch (UnauthorizedAccessException) { return "Windows doesn't let you write there without administrator rights. Pick a folder in your user profile."; }
            catch (Exception ex) { return ex.Message; }
        }

        public static void Install(InstallOptions o, Action<double, string> progress)
        {
            var dir = Path.GetFullPath(o.Dir).TrimEnd('\\');
            Log("install to " + dir);
            var why = CheckDir(dir);
            if (why != null) throw new InvalidOperationException(why);

            progress(0.01, "Closing WinIsland if it's running…");
            StopRunning();
            Directory.CreateDirectory(dir);

            // 0) leftovers from the older WebView2 build (bin\ held the .NET runtime, web\ the UI);
            //    only touched when it really is our install, recognised by its exe
            if (File.Exists(Path.Combine(dir, "bin", "WinIsland.exe")) || File.Exists(Path.Combine(dir, "bin", "Frost.exe")))
            {
                try { if (Directory.Exists(Path.Combine(dir, "web"))) Directory.Delete(Path.Combine(dir, "web"), true); } catch { }
                try { foreach (var f in Directory.GetFiles(Path.Combine(dir, "bin"))) TryDelete(f); } catch { }
                try { var wv = Path.Combine(dir, "bin", "WinIsland.exe.WebView2"); if (Directory.Exists(wv)) Directory.Delete(wv, true); } catch { }
            }

            // 1) files
            using (var s = Assembly.GetExecutingAssembly().GetManifestResourceStream("payload.zip"))
            {
                if (s == null) throw new InvalidOperationException("This setup file is damaged (no payload). Download it again.");
                using (var z = new ZipArchive(s, ZipArchiveMode.Read))
                {
                    long total = Math.Max(1, z.Entries.Sum(e => e.Length)), done = 0;
                    var buf = new byte[1 << 16];
                    foreach (var e in z.Entries)
                    {
                        var target = Path.GetFullPath(Path.Combine(dir, e.FullName));
                        if (!target.StartsWith(dir + "\\", StringComparison.OrdinalIgnoreCase)) continue; // zip-slip guard
                        if (e.FullName.EndsWith("/")) { Directory.CreateDirectory(target); continue; }
                        Directory.CreateDirectory(Path.GetDirectoryName(target));
                        progress(0.04 + 0.82 * done / total, e.FullName.Replace('/', '\\'));
                        using (var src = e.Open())
                        using (var dst = OpenForWrite(target))
                        {
                            int n;
                            while ((n = src.Read(buf, 0, buf.Length)) > 0)
                            {
                                dst.Write(buf, 0, n); done += n;
                                if (done % (1 << 21) < n) progress(0.04 + 0.82 * done / total, e.FullName.Replace('/', '\\'));
                            }
                        }
                        try { File.SetLastWriteTime(target, e.LastWriteTime.DateTime); } catch { }
                    }
                }
            }

            // 2) first-run settings (existing settings from an earlier install are kept; only the chosen look is applied)
            progress(0.88, "Saving your preferences…");
            WriteSettings(dir, o);

            // 3) uninstaller + shortcuts + Apps & features entry + autostart
            progress(0.92, "Creating shortcuts…");
            var exe = Path.Combine(dir, "bin", "WinIsland.exe");
            var uninst = Path.Combine(dir, "Uninstall.exe");
            try { File.Copy(System.Windows.Forms.Application.ExecutablePath, uninst, true); } catch (Exception ex) { Log("copy uninstaller: " + ex.Message); }
            if (o.StartMenu) Shortcut(StartMenuLink, exe, "--panel", "WinIsland settings"); else TryDelete(StartMenuLink);
            if (o.Desktop) Shortcut(DesktopLink, exe, "--panel", "WinIsland settings"); else TryDelete(DesktopLink);

            progress(0.96, "Registering with Windows…");
            using (var k = Registry.CurrentUser.CreateSubKey(UninstallKey))
            {
                k.SetValue("DisplayName", AppName);
                k.SetValue("DisplayVersion", Version);
                k.SetValue("Publisher", Publisher);
                k.SetValue("DisplayIcon", exe + ",0");
                k.SetValue("InstallLocation", dir);
                k.SetValue("InstallDate", DateTime.Now.ToString("yyyyMMdd"));
                k.SetValue("UninstallString", $"\"{uninst}\" /uninstall");
                k.SetValue("QuietUninstallString", $"\"{uninst}\" /uninstall /silent");
                k.SetValue("EstimatedSize", (int)(DirSize(dir) / 1024), RegistryValueKind.DWord);
                k.SetValue("NoModify", 1, RegistryValueKind.DWord);
                k.SetValue("NoRepair", 1, RegistryValueKind.DWord);
            }
            using (var k = Registry.CurrentUser.CreateSubKey(RunKey))
            {
                if (o.Autostart) k.SetValue(RunName, $"\"{exe}\" --autostart");
                else if (k.GetValue(RunName) is string v && v.IndexOf(dir, StringComparison.OrdinalIgnoreCase) >= 0) k.DeleteValue(RunName);
            }
            progress(1, "Done");
            Log("install ok");
        }

        private static Stream OpenForWrite(string path)
        {
            for (int i = 0; ; i++)
            {
                try { return File.Create(path); }
                catch (IOException) when (i < 20) { Thread.Sleep(250); } // a closing process may still hold the file
            }
        }

        private static void WriteSettings(string dir, InstallOptions o)
        {
            var js = new JavaScriptSerializer();
            // the app keeps settings next to itself when the folder is writable (it always is for a per-user install)
            var path = Path.Combine(dir, "settings.json");
            Dictionary<string, object> doc = null;
            try { if (File.Exists(path)) doc = js.Deserialize<Dictionary<string, object>>(File.ReadAllText(path)); } catch { }
            bool fresh = doc == null;
            doc = doc ?? new Dictionary<string, object>();
            Dictionary<string, object> Section(string name)
            {
                if (!(doc.TryGetValue(name, out var v) && v is Dictionary<string, object> d)) doc[name] = d = new Dictionary<string, object>();
                return d;
            }
            var island = Section("island"); var general = Section("general");
            island["style"] = o.Style;
            general["autostart"] = o.Autostart;
            if (fresh) { island["lyrics"] = o.Lyrics; island["lyricsNotch"] = o.Lyrics; }
            File.WriteAllText(path, js.Serialize(doc));
        }

        public static void Uninstall(string dir, bool keepSettings, Action<double, string> progress)
        {
            Log("uninstall " + dir);
            progress(0.05, "Closing WinIsland…");
            StopRunning();
            progress(0.15, "Removing shortcuts…");
            TryDelete(StartMenuLink); TryDelete(DesktopLink);
            using (var k = Registry.CurrentUser.OpenSubKey(RunKey, true))
            {
                var v = k?.GetValue(RunName) as string;
                if (v != null && (dir == null || v.IndexOf(dir, StringComparison.OrdinalIgnoreCase) >= 0)) k.DeleteValue(RunName);
            }
            if (dir != null && Directory.Exists(dir))
            {
                bool Keep(string f) => keepSettings && Path.GetFileName(f).Equals("settings.json", StringComparison.OrdinalIgnoreCase);
                var files = Directory.GetFiles(dir, "*", SearchOption.AllDirectories).Where(f => !Keep(f)).ToArray();
                for (int i = 0; i < files.Length; i++)
                {
                    progress(0.2 + 0.6 * i / Math.Max(1, files.Length), files[i].Substring(dir.Length + 1));
                    try { File.Delete(files[i]); } catch { }
                }
                // files can stay locked for a moment after the island quits: retry a few times
                for (int pass = 0; pass < 8; pass++)
                {
                    var left = Directory.GetFiles(dir, "*", SearchOption.AllDirectories).Where(f => !Keep(f)).ToArray();
                    if (left.Length == 0) break;
                    progress(0.82 + pass * 0.01, "Waiting for files to be released…");
                    Thread.Sleep(500);
                    foreach (var f in left) try { File.Delete(f); } catch { }
                }
                foreach (var d in Directory.GetDirectories(dir, "*", SearchOption.AllDirectories).OrderByDescending(x => x.Length))
                    try { Directory.Delete(d, false); } catch { }
                if (!keepSettings)
                {
                    try { Directory.Delete(dir, true); } catch { }
                    // anything still locked (e.g. the Uninstall.exe that launched us) goes once we've all exited
                    if (Directory.Exists(dir))
                        try { Process.Start(new ProcessStartInfo("cmd.exe", $"/c ping -n 5 127.0.0.1 >nul & rmdir /s /q \"{dir}\"") { CreateNoWindow = true, UseShellExecute = false }); } catch { }
                }
            }
            if (!keepSettings) try { if (Directory.Exists(DataDir)) Directory.Delete(DataDir, true); } catch { }
            progress(0.95, "Unregistering…");
            Registry.CurrentUser.DeleteSubKeyTree(UninstallKey, false);
            progress(1, "Done");
            Log("uninstall ok");
        }

        /// <summary>Asks every running WinIsland to quit, then makes sure.</summary>
        public static void StopRunning()
        {
            var procs = Process.GetProcessesByName("Frost").Concat(Process.GetProcessesByName("WinIsland")).ToArray();
            foreach (var p in procs)
            {
                try
                {
                    var path = p.MainModule?.FileName;
                    if (path != null) using (var q = Process.Start(new ProcessStartInfo(path, "--quit") { UseShellExecute = false, CreateNoWindow = true })) q?.WaitForExit(3000);
                }
                catch { }
            }
            foreach (var p in procs) { try { if (!p.WaitForExit(4000)) { p.Kill(); p.WaitForExit(3000); } } catch { } }
        }

        public static void Launch(string dir, bool openSettings)
        {
            var exe = Path.Combine(dir, "bin", "WinIsland.exe");
            if (File.Exists(exe)) Process.Start(new ProcessStartInfo(exe, openSettings ? "--panel" : "") { UseShellExecute = true, WorkingDirectory = Path.GetDirectoryName(exe) });
        }

        public static void ScheduleSelfDeleteIfTemp()
        {
            var self = System.Windows.Forms.Application.ExecutablePath;
            if (!self.StartsWith(Path.GetTempPath(), StringComparison.OrdinalIgnoreCase) || !Path.GetFileName(self).StartsWith("WinIslandUninstall-")) return;
            try { Process.Start(new ProcessStartInfo("cmd.exe", $"/c ping -n 3 127.0.0.1 >nul & del /f /q \"{self}\"") { CreateNoWindow = true, UseShellExecute = false }); } catch { }
        }

        private static void Shortcut(string lnk, string target, string args, string desc)
        {
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(lnk));
                var t = Type.GetTypeFromProgID("WScript.Shell");
                var sh = Activator.CreateInstance(t);
                var l = t.InvokeMember("CreateShortcut", BindingFlags.InvokeMethod, null, sh, new object[] { lnk });
                var lt = l.GetType();
                lt.InvokeMember("TargetPath", BindingFlags.SetProperty, null, l, new object[] { target });
                lt.InvokeMember("Arguments", BindingFlags.SetProperty, null, l, new object[] { args });
                lt.InvokeMember("WorkingDirectory", BindingFlags.SetProperty, null, l, new object[] { Path.GetDirectoryName(target) });
                lt.InvokeMember("IconLocation", BindingFlags.SetProperty, null, l, new object[] { target + ",0" });
                lt.InvokeMember("Description", BindingFlags.SetProperty, null, l, new object[] { desc });
                lt.InvokeMember("Save", BindingFlags.InvokeMethod, null, l, null);
            }
            catch (Exception ex) { Log("shortcut " + lnk + ": " + ex.Message); }
        }

        private static void TryDelete(string f) { try { if (File.Exists(f)) File.Delete(f); } catch (Exception ex) { Log("delete " + f + ": " + ex.Message); } }
        private static long DirSize(string d) { try { return Directory.GetFiles(d, "*", SearchOption.AllDirectories).Sum(f => new FileInfo(f).Length); } catch { return 0; } }
    }
}
