// WinIsland Setup / Uninstall.
//   WinIslandSetup.exe                         interactive install wizard
//   WinIslandSetup.exe /silent [/dir=PATH] [/style=macos|ios] [/noautostart] [/desktop] [/nostartmenu] [/nolaunch]
//   Uninstall.exe (or /uninstall [/silent] [/keepsettings])
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Windows.Forms;

namespace FrostSetup
{
    internal static class Program
    {
        public static string TempDir;
        public static Dictionary<string, string> Opts = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

        [STAThread]
        private static int Main(string[] args)
        {
            foreach (var a in args)
            {
                var s = a.TrimStart('/', '-'); var i = s.IndexOf('=');
                if (i > 0) Opts[s.Substring(0, i)] = s.Substring(i + 1).Trim('"'); else Opts[s] = "1";
            }
            bool uninstall = Opts.ContainsKey("uninstall") || Path.GetFileNameWithoutExtension(Application.ExecutablePath).Equals("Uninstall", StringComparison.OrdinalIgnoreCase);
            bool silent = Opts.ContainsKey("silent") || Opts.ContainsKey("S");

            // An uninstaller can't delete the folder it runs from: continue from a temp copy.
            if (uninstall && !Opts.ContainsKey("relocated"))
            {
                var dir = Engine.ExistingInstallDir();
                var self = Application.ExecutablePath;
                if (dir != null && self.StartsWith(dir, StringComparison.OrdinalIgnoreCase))
                {
                    var tmp = Path.Combine(Path.GetTempPath(), "WinIslandUninstall-" + Guid.NewGuid().ToString("N").Substring(0, 8) + ".exe");
                    File.Copy(self, tmp, true);
                    Process.Start(new ProcessStartInfo(tmp, "/uninstall /relocated " + string.Join(" ", args.Where(x => !x.TrimStart('/', '-').Equals("uninstall", StringComparison.OrdinalIgnoreCase)).Select(Quote))) { UseShellExecute = false });
                    return 0; // exit right away so this copy can be deleted too
                }
            }

            Engine.Log("start " + string.Join(" ", args));
            try
            {
                if (silent)
                {
                    if (uninstall) Engine.Uninstall(Engine.ExistingInstallDir(), Opts.ContainsKey("keepsettings"), (p, f) => { });
                    else
                    {
                        var o = InstallOptions.FromArgs(Opts);
                        Engine.Install(o, (p, f) => { });
                        if (o.Launch) Engine.Launch(o.Dir, false);
                    }
                    Engine.ScheduleSelfDeleteIfTemp();
                    return 0;
                }

                TempDir = Path.Combine(Path.GetTempPath(), "WinIslandSetup-" + Process.GetCurrentProcess().Id);
                Directory.CreateDirectory(TempDir);
                ExtractResources(TempDir);
                AppDomain.CurrentDomain.AssemblyResolve += (s, e) =>
                {
                    var name = new AssemblyName(e.Name).Name + ".dll";
                    var path = Path.Combine(TempDir, "lib", name);
                    return File.Exists(path) ? Assembly.LoadFrom(path) : null;
                };
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                return Ui.Run(uninstall);
            }
            catch (Exception ex)
            {
                Engine.Log("fatal " + ex);
                if (!silent) MessageBox.Show(ex.Message, "WinIsland Setup", MessageBoxButtons.OK, MessageBoxIcon.Error);
                return 1;
            }
            finally
            {
                try { if (TempDir != null) Directory.Delete(TempDir, true); } catch { }
            }
        }

        private static string Quote(string a) => a.Contains(" ") ? "\"" + a + "\"" : a;

        private static void ExtractResources(string dir)
        {
            var asm = Assembly.GetExecutingAssembly();
            foreach (var n in asm.GetManifestResourceNames())
            {
                if (!(n.StartsWith("ui/") || n.StartsWith("lib/"))) continue;
                var path = Path.Combine(dir, n.Replace('/', Path.DirectorySeparatorChar));
                Directory.CreateDirectory(Path.GetDirectoryName(path));
                using (var s = asm.GetManifestResourceStream(n)) using (var f = File.Create(path)) s.CopyTo(f);
            }
        }
    }

    internal sealed class InstallOptions
    {
        public string Dir = Engine.DefaultDir;
        public string Style = "macos";
        public bool Autostart = true, Desktop, StartMenu = true, Lyrics = true, Launch = true;

        public static InstallOptions FromArgs(Dictionary<string, string> o) => new InstallOptions
        {
            Dir = o.ContainsKey("dir") ? o["dir"] : Engine.DefaultDir,
            Style = o.ContainsKey("style") && o["style"] == "ios" ? "ios" : "macos",
            Autostart = !o.ContainsKey("noautostart"),
            Desktop = o.ContainsKey("desktop"),
            StartMenu = !o.ContainsKey("nostartmenu"),
            Lyrics = !o.ContainsKey("nolyrics"),
            Launch = !o.ContainsKey("nolaunch"),
        };
    }
}
