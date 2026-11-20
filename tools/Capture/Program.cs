// Frost capture tool: renders a page in WebView2 (the same engine as the app) and saves PNG screenshots or
// deterministic frame sequences (virtualised clock), optionally with a transparent background.
//
//   Capture --root <folder> --url <path?query> --size 1280x720 [--scale 2] --out shot.png [--wait 1500] [--transparent]
//   Capture ... --frames 300 --fps 60 --out dir\f_%05d.png [--skip 500]     (frame sequence)
//   Capture ... --js "code to run before capture"
using System.Drawing;
using System.Text.Json;
using System.Text.Json.Nodes;
using Microsoft.Web.WebView2.Core;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        var a = Args.Parse(args);
        ApplicationConfiguration.Initialize();
        var form = new Form { FormBorderStyle = FormBorderStyle.None, ShowInTaskbar = false, StartPosition = FormStartPosition.Manual,
                              Location = new Point(-20000, -20000), Size = new Size(400, 300) };
        int code = 1;
        form.Shown += async (_, _) =>
        {
            try { code = await Run(form, a); }
            catch (Exception ex) { Console.Error.WriteLine(ex); File.AppendAllText(Path.Combine(AppContext.BaseDirectory, "capture-error.log"), ex + "\n"); }
            form.Close();
        };
        Application.Run(form);
        return code;
    }

    private static async Task<int> Run(Form form, Args a)
    {
        var udf = a.Udf ?? Path.Combine(Path.GetTempPath(), "frost-capture-wv2");
        var env = await CoreWebView2Environment.CreateAsync(null, udf, new CoreWebView2EnvironmentOptions("--autoplay-policy=no-user-gesture-required --force-color-profile=srgb --hide-scrollbars --disable-features=CalculateNativeWinOcclusion --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows"));
        var ctl = await env.CreateCoreWebView2ControllerAsync(form.Handle);
        ctl.DefaultBackgroundColor = a.Transparent ? Color.Transparent : Color.Black;
        ctl.Bounds = new Rectangle(0, 0, a.W, a.H);
        form.ClientSize = new Size(a.W, a.H);
        var wv = ctl.CoreWebView2;
        wv.Settings.AreDevToolsEnabled = true;
        wv.SetVirtualHostNameToFolderMapping("frost.local", a.Root, CoreWebView2HostResourceAccessKind.Allow);

        async Task<JsonNode> Cdp(string method, object p = null) =>
            JsonNode.Parse(await wv.CallDevToolsProtocolMethodAsync(method, JsonSerializer.Serialize(p ?? new { })));

        await Cdp("Emulation.setDeviceMetricsOverride", new { width = a.W, height = a.H, deviceScaleFactor = a.Scale, mobile = false });
        if (a.Transparent) await Cdp("Emulation.setDefaultBackgroundColorOverride", new { color = new { r = 0, g = 0, b = 0, a = 0 } });
        if (a.Frames > 0) await wv.AddScriptToExecuteOnDocumentCreatedAsync(VirtualClock);
        await wv.AddScriptToExecuteOnDocumentCreatedAsync("window.__errs=[];addEventListener('error',e=>__errs.push(e.message+' @'+e.lineno));addEventListener('unhandledrejection',e=>__errs.push(String(e.reason)));");

        var loaded = new TaskCompletionSource();
        wv.NavigationCompleted += (_, _) => loaded.TrySetResult();
        wv.Navigate("https://frost.local/" + a.Url.TrimStart('/'));
        await loaded.Task;

        async Task Shot(string path)
        {
            var r = await Cdp("Page.captureScreenshot", new { format = "png", captureBeyondViewport = false, fromSurface = true });
            Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path)));
            await File.WriteAllBytesAsync(path, Convert.FromBase64String((string)r["data"]));
        }

        if (a.Frames <= 0)
        {
            await Task.Delay(a.Wait);
            if (a.Js != null) { await wv.ExecuteScriptAsync(a.Js); await Task.Delay(a.JsWait); }
            if (a.Eval != null) Console.WriteLine("eval: " + await wv.ExecuteScriptAsync(a.Eval));
            await Shot(a.Out);
            Console.WriteLine("saved " + a.Out);
            return 0;
        }

        // Frame sequence: advance the page's virtual clock one frame at a time, then capture.
        double dt = 1000.0 / a.Fps;
        await Task.Delay(300); // let the first layout settle (virtual time is frozen meanwhile)
        for (double t = 0; t < a.Skip; t += dt) await wv.ExecuteScriptAsync($"window.__frostStep && __frostStep({dt.ToString(System.Globalization.CultureInfo.InvariantCulture)})");
        if (a.Js != null) await wv.ExecuteScriptAsync(a.Js);
        for (int i = 0; i < a.Frames; i++)
        {
            await wv.ExecuteScriptAsync($"window.__frostStep && __frostStep({dt.ToString(System.Globalization.CultureInfo.InvariantCulture)})");
            await Shot(string.Format(a.Out.Replace("%05d", "{0:00000}"), i + 1));
            if (i % 60 == 0) Console.WriteLine($"frame {i + 1}/{a.Frames}");
        }
        return 0;
    }

    // Replaces the page's clock (Date.now, performance.now, timers, rAF) with a virtual one, and pins every
    // CSS/WAAPI animation to it, so each captured frame shows exactly t = n / fps regardless of capture speed.
    private const string VirtualClock = """
    (() => {
      const realNow = performance.now.bind(performance), realDateNow = Date.now, base = realDateNow();
      let vt = 0, seq = 0; const timers = new Map(); let rafs = [];
      performance.now = () => vt;
      Date.now = () => base + vt;
      const RealDate = Date;
      window.Date = class extends RealDate { constructor(...a) { super(...(a.length ? a : [base + vt])); } static now() { return base + vt; } };
      window.setTimeout = (fn, ms = 0, ...args) => { const id = ++seq; timers.set(id, { fn, at: vt + Math.max(0, +ms || 0), args }); return id; };
      window.setInterval = (fn, ms = 0, ...args) => { const id = ++seq; timers.set(id, { fn, at: vt + Math.max(1, +ms || 0), every: Math.max(1, +ms || 0), args }); return id; };
      window.clearTimeout = window.clearInterval = (id) => timers.delete(id);
      window.requestAnimationFrame = (fn) => { const id = ++seq; rafs.push({ id, fn }); return id; };
      window.cancelAnimationFrame = (id) => { rafs = rafs.filter((r) => r.id !== id); };
      window.__frostStep = (dt) => {
        const end = vt + dt;
        for (let guard = 0; guard < 10000; guard++) {
          let next = null;
          for (const [id, t] of timers) if (t.at <= end && (!next || t.at < next[1].at || (t.at === next[1].at && id < next[0]))) next = [id, t];
          if (!next) break;
          const [id, t] = next; vt = Math.max(vt, t.at);
          if (t.every) t.at += t.every; else timers.delete(id);
          try { typeof t.fn === 'function' ? t.fn(...t.args) : eval(t.fn); } catch (e) { console.error(e); }
        }
        vt = end;
        const list = rafs; rafs = []; for (const r of list) { try { r.fn(vt); } catch (e) { console.error(e); } }
        for (const an of document.getAnimations()) {
          if (an.__vs === undefined) an.__vs = vt - dt - (an.currentTime || 0);
          try { an.pause(); an.currentTime = vt - an.__vs; } catch {}
        }
      };
    })();
    """;
}

internal sealed class Args
{
    public string Root = Directory.GetCurrentDirectory(), Url = "index.html", Out = "shot.png", Js, Eval, Udf;
    public int W = 1280, H = 720, Wait = 1500, Frames, Fps = 60, Skip, JsWait = 600;
    public double Scale = 2;
    public bool Transparent;

    public static Args Parse(string[] v)
    {
        var a = new Args();
        for (int i = 0; i < v.Length; i++)
        {
            string N() => v[++i];
            switch (v[i])
            {
                case "--root": a.Root = Path.GetFullPath(N()); break;
                case "--url": a.Url = N(); break;
                case "--out": a.Out = N(); break;
                case "--size": { var p = N().Split('x'); a.W = int.Parse(p[0]); a.H = int.Parse(p[1]); break; }
                case "--scale": a.Scale = double.Parse(N(), System.Globalization.CultureInfo.InvariantCulture); break;
                case "--wait": a.Wait = int.Parse(N()); break;
                case "--frames": a.Frames = int.Parse(N()); break;
                case "--fps": a.Fps = int.Parse(N()); break;
                case "--skip": a.Skip = int.Parse(N()); break;
                case "--js": a.Js = N(); break;
                case "--jswait": a.JsWait = int.Parse(N()); break;
                case "--eval": a.Eval = N(); break;
                case "--udf": a.Udf = N(); break;
                case "--transparent": a.Transparent = true; break;
            }
        }
        return a;
    }
}
