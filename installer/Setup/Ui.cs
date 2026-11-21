using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows.Forms;
using Microsoft.Web.WebView2.Core;

namespace FrostSetup
{
    /// <summary>The wizard window: a Mica dark window hosting ui/setup.html in WebView2. All logic lives in Engine.</summary>
    internal static class Ui
    {
        public static int Run(bool uninstall)
        {
            try { CoreWebView2Environment.SetLoaderDllFolderPath(Path.Combine(Program.TempDir, "lib")); } catch { }
            string ver = null;
            try { ver = CoreWebView2Environment.GetAvailableBrowserVersionString(); } catch { }
            if (string.IsNullOrEmpty(ver))
            {
                if (MessageBox.Show("WinIsland needs the Microsoft Edge WebView2 Runtime, which is missing on this PC.\n\nOpen the download page?", "WinIsland Setup",
                    MessageBoxButtons.YesNo, MessageBoxIcon.Information) == DialogResult.Yes)
                    System.Diagnostics.Process.Start("https://developer.microsoft.com/microsoft-edge/webview2/");
                return 2;
            }
            var f = new SetupForm(uninstall);
            Application.Run(f);
            return f.ExitCode;
        }
    }

    internal sealed class SetupForm : Form
    {
        [DllImport("dwmapi.dll")] private static extern int DwmSetWindowAttribute(IntPtr h, int a, ref int v, int s);
        private readonly bool _uninstall;
        private CoreWebView2Controller _ctl;
        private readonly JavaScriptSerializer _js = new JavaScriptSerializer();
        private bool _busy, _finished;
        private string _installedDir;
        public int ExitCode = 1;

        public SetupForm(bool uninstall)
        {
            _uninstall = uninstall;
            Text = uninstall ? "Uninstall WinIsland" : "WinIsland Setup";
            FormBorderStyle = FormBorderStyle.FixedSingle;
            MaximizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;
            BackColor = Color.FromArgb(14, 15, 22);
            AutoScaleMode = AutoScaleMode.Dpi;
            ClientSize = new Size(880, 580);
            try { Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath); } catch { }
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            int on = 1, round = 2, caption = 0x00160F0E; // #0E0F16
            DwmSetWindowAttribute(Handle, 20, ref on, 4);      // DWMWA_USE_IMMERSIVE_DARK_MODE
            DwmSetWindowAttribute(Handle, 33, ref round, 4);   // DWMWA_WINDOW_CORNER_PREFERENCE
            DwmSetWindowAttribute(Handle, 35, ref caption, 4); // DWMWA_CAPTION_COLOR
        }

        protected override async void OnShown(EventArgs e)
        {
            base.OnShown(e);
            try
            {
                var udf = Path.Combine(Path.GetTempPath(), "WinIslandSetup.WebView2");
                var env = await CoreWebView2Environment.CreateAsync(null, udf);
                _ctl = await env.CreateCoreWebView2ControllerAsync(Handle);
                _ctl.DefaultBackgroundColor = BackColor;
                _ctl.Bounds = ClientRectangle;
                var wv = _ctl.CoreWebView2;
                wv.Settings.AreDevToolsEnabled = false;
                wv.Settings.AreDefaultContextMenusEnabled = false;
                wv.Settings.IsStatusBarEnabled = false;
                wv.Settings.IsZoomControlEnabled = false;
                wv.Settings.AreBrowserAcceleratorKeysEnabled = false;
                wv.SetVirtualHostNameToFolderMapping("setup.frost", Path.Combine(Program.TempDir, "ui"), CoreWebView2HostResourceAccessKind.Allow);
                wv.WebMessageReceived += (s, a) => OnMessage(a.TryGetWebMessageAsString());
                var auto = Environment.GetEnvironmentVariable("FROST_SETUP_AUTOTEST");
                if (!string.IsNullOrEmpty(auto) && !_uninstall)
                    wv.NavigationCompleted += async (s, a) => await wv.ExecuteScriptAsync($"autotest({_js.Serialize(auto)}, 'island')");
                wv.Navigate("https://setup.frost/setup.html" + (_uninstall ? "?mode=uninstall" : ""));
            }
            catch (Exception ex) { Engine.Log("ui " + ex); MessageBox.Show(ex.Message, Text); Close(); }
        }

        protected override void OnResize(EventArgs e) { base.OnResize(e); if (_ctl != null) _ctl.Bounds = ClientRectangle; }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            if (_busy) { e.Cancel = true; return; }
            base.OnFormClosing(e);
        }

        private void Post(object o) { try { _ctl?.CoreWebView2.PostWebMessageAsString(_js.Serialize(o)); } catch { } }

        private void OnMessage(string raw)
        {
            Dictionary<string, object> m;
            try { m = _js.Deserialize<Dictionary<string, object>>(raw); } catch { return; }
            string S(string k) => m.TryGetValue(k, out var v) ? Convert.ToString(v) : null;
            bool B(string k) => m.TryGetValue(k, out var v) && v is bool b && b;
            switch (S("cmd"))
            {
                case "init":
                {
                    var existing = Engine.ExistingInstallDir();
                    var dir = existing ?? Engine.DefaultDir;
                    Post(new { type = "init", mode = _uninstall ? "uninstall" : "install", version = Engine.Version, dir, existing = existing != null,
                               existingVersion = Engine.ExistingVersion(), sizeMB = Math.Round(Engine.PayloadBytes() / 1048576.0), freeMB = FreeMB(dir), dirError = Engine.CheckDir(dir) });
                    break;
                }
                case "browse":
                {
                    var picked = FolderPicker.Pick(Handle, S("dir"));
                    if (picked != null)
                    {
                        if (!picked.TrimEnd('\\').EndsWith(Engine.AppName, StringComparison.OrdinalIgnoreCase)) picked = Path.Combine(picked, Engine.AppName);
                        Post(new { type = "dir", dir = picked, freeMB = FreeMB(picked), dirError = Engine.CheckDir(picked) });
                    }
                    break;
                }
                case "checkDir": Post(new { type = "dir", dir = S("dir"), freeMB = FreeMB(S("dir")), dirError = Engine.CheckDir(S("dir")), quiet = true }); break;
                case "install":
                {
                    var o = new InstallOptions { Dir = S("dir"), Style = Program.NormStyle(S("style")), Autostart = B("autostart"), Desktop = B("desktop"),
                                                 StartMenu = B("startMenu"), Lyrics = B("lyrics"), Launch = B("launch") };
                    RunJob(() => Engine.Install(o, Progress), () => { _installedDir = Path.GetFullPath(o.Dir); });
                    break;
                }
                case "uninstall": RunJob(() => Engine.Uninstall(Engine.ExistingInstallDir(), B("keepSettings"), Progress), null); break;
                case "finish":
                    if (!_uninstall && B("launch") && _installedDir != null) { try { Engine.Launch(_installedDir, true); } catch (Exception ex) { Engine.Log("launch " + ex); } }
                    ExitCode = 0; Close(); break;
                case "cancel": Close(); break;
            }
        }

        private DateTime _lastProgress;
        private void Progress(double p, string file)
        {
            var now = DateTime.UtcNow;
            if (p < 1 && (now - _lastProgress).TotalMilliseconds < 40) return;
            _lastProgress = now;
            BeginInvoke(new Action(() => Post(new { type = "progress", pct = Math.Round(p * 100, 1), file })));
        }

        private void RunJob(Action job, Action ok)
        {
            if (_busy || _finished) return;
            _busy = true;
            Task.Run(() =>
            {
                string err = null;
                try { job(); } catch (Exception ex) { err = ex.Message; Engine.Log("job " + ex); }
                BeginInvoke(new Action(() =>
                {
                    _busy = false;
                    if (err == null) { _finished = true; ok?.Invoke(); ExitCode = 0; if (_uninstall) Engine.ScheduleSelfDeleteIfTemp(); }
                    Post(new { type = err == null ? "done" : "error", message = err });
                }));
            });
        }

        private static long FreeMB(string dir) { var b = Engine.FreeBytes(dir ?? Engine.DefaultDir); return b < 0 ? -1 : b / 1048576; }
    }

    /// <summary>The modern Windows folder picker (IFileOpenDialog with FOS_PICKFOLDERS).</summary>
    internal static class FolderPicker
    {
        public static string Pick(IntPtr owner, string start)
        {
            IFileOpenDialog d = null;
            try
            {
                d = (IFileOpenDialog)new FileOpenDialogRCW();
                d.GetOptions(out var o);
                d.SetOptions(o | 0x20 /*PICKFOLDERS*/ | 0x40 /*FORCEFILESYSTEM*/);
                d.SetTitle("Choose where to install WinIsland");
                try
                {
                    var p = start;
                    while (!string.IsNullOrEmpty(p) && !Directory.Exists(p)) p = Path.GetDirectoryName(p);
                    if (!string.IsNullOrEmpty(p) && SHCreateItemFromParsingName(p, IntPtr.Zero, typeof(IShellItem).GUID, out var item) == 0) d.SetFolder(item);
                }
                catch { }
                if (d.Show(owner) != 0) return null;
                d.GetResult(out var res);
                res.GetDisplayName(0x80058000 /*SIGDN_FILESYSPATH*/, out var path);
                return path;
            }
            catch { return null; }
            finally { if (d != null) Marshal.ReleaseComObject(d); }
        }

        [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
        private static extern int SHCreateItemFromParsingName(string path, IntPtr pbc, [MarshalAs(UnmanagedType.LPStruct)] Guid riid, out IShellItem item);

        [ComImport, Guid("DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7")] private class FileOpenDialogRCW { }

        [ComImport, Guid("d57c7288-d4ad-4768-be02-9d969532d960"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IFileOpenDialog
        {
            [PreserveSig] int Show(IntPtr parent);
            void SetFileTypes(); void SetFileTypeIndex(); void GetFileTypeIndex(); void Advise(); void Unadvise();
            void SetOptions(uint fos); void GetOptions(out uint fos);
            void SetDefaultFolder(IShellItem si); void SetFolder(IShellItem si); void GetFolder(); void GetCurrentSelection();
            void SetFileName(); void GetFileName(); void SetTitle([MarshalAs(UnmanagedType.LPWStr)] string title);
            void SetOkButtonLabel(); void SetFileNameLabel(); void GetResult(out IShellItem si);
        }

        [ComImport, Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        internal interface IShellItem
        {
            void BindToHandler(); void GetParent();
            void GetDisplayName(uint sigdn, [MarshalAs(UnmanagedType.LPWStr)] out string name);
        }
    }
}
