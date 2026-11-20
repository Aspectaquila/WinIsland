using System.Diagnostics;
using System.Net.Http;
using System.Net.NetworkInformation;
using System.Text.Json.Nodes;
using static Frost.Native;

namespace Frost;

/// <summary>CPU / RAM / network sampling. Runs only while at least one visible surface has subscribed.</summary>
internal sealed class SystemStats : IDisposable
{
    private readonly System.Windows.Forms.Timer _timer = new() { Interval = 2000 };
    private ulong _idle, _total;
    private long _rx, _tx; private long _stamp;
    private readonly HashSet<string> _subs = new();
    public event Action<JsonObject> Sample;
    public JsonObject Last { get; private set; }

    public SystemStats() { _timer.Tick += (_, _) => Tick(); }

    public void Subscribe(string who, bool on)
    {
        if (on) _subs.Add(who); else _subs.Remove(who);
        if (_subs.Count > 0 && !_timer.Enabled) { Prime(); _timer.Start(); Tick(); }
        else if (_subs.Count == 0 && _timer.Enabled) _timer.Stop();
    }

    private void Prime()
    {
        GetSystemTimes(out var i, out var k, out var u);
        _idle = i.Value; _total = k.Value + u.Value;
        (_rx, _tx) = NetBytes(); _stamp = Stopwatch.GetTimestamp();
    }

    private void Tick()
    {
        GetSystemTimes(out var i, out var k, out var u);
        ulong idle = i.Value, total = k.Value + u.Value;
        double cpu = total > _total ? 100.0 * (1 - (double)(idle - _idle) / (total - _total)) : 0;
        _idle = idle; _total = total;

        var m = new MEMORYSTATUSEX { dwLength = (uint)System.Runtime.InteropServices.Marshal.SizeOf<MEMORYSTATUSEX>() };
        GlobalMemoryStatusEx(ref m);

        var (rx, tx) = NetBytes(); var now = Stopwatch.GetTimestamp();
        double secs = Math.Max(0.001, (now - _stamp) / (double)Stopwatch.Frequency);
        double down = Math.Max(0, (rx - _rx) / secs), up = Math.Max(0, (tx - _tx) / secs);
        _rx = rx; _tx = tx; _stamp = now;

        GetSystemPowerStatus(out var ps);
        var msg = new JsonObject
        {
            ["type"] = "stats",
            ["cpu"] = Math.Round(Math.Clamp(cpu, 0, 100), 1),
            ["ram"] = m.dwMemoryLoad,
            ["ramUsed"] = Math.Round((m.ullTotalPhys - m.ullAvailPhys) / 1073741824.0, 1),
            ["ramTotal"] = Math.Round(m.ullTotalPhys / 1073741824.0, 1),
            ["down"] = Math.Round(down), ["up"] = Math.Round(up),
            ["battery"] = ps.BatteryFlag == 128 || ps.BatteryFlag == 255 ? null : ps.BatteryLifePercent,
            ["charging"] = ps.ACLineStatus == 1,
        };
        Last = msg;
        Sample?.Invoke(msg);
    }

    private static (long, long) NetBytes()
    {
        long rx = 0, tx = 0;
        try
        {
            foreach (var n in NetworkInterface.GetAllNetworkInterfaces())
            {
                if (n.OperationalStatus != OperationalStatus.Up || n.NetworkInterfaceType is NetworkInterfaceType.Loopback or NetworkInterfaceType.Tunnel) continue;
                var s = n.GetIPStatistics(); rx += s.BytesReceived; tx += s.BytesSent;
            }
        }
        catch { }
        return (rx, tx);
    }

    public void Dispose() => _timer.Dispose();
}

/// <summary>Weather from Open-Meteo (free, no API key). Cached on disk; refreshed at most every 30 minutes, only while needed.</summary>
internal sealed class WeatherService
{
    private static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(12) };
    private readonly Settings _s;
    private readonly string _cache = Path.Combine(Settings.Data, "cache", "weather.json");
    private readonly System.Windows.Forms.Timer _timer = new() { Interval = 30 * 60 * 1000 };
    public JsonObject Current { get; private set; }
    public event Action<JsonObject> Changed;

    static WeatherService() => Http.DefaultRequestHeaders.UserAgent.ParseAdd("FrostDesktop/1.0");

    public WeatherService(Settings s)
    {
        _s = s;
        _timer.Tick += async (_, _) => await RefreshAsync(false);
        try { if (File.Exists(_cache)) Current = JsonNode.Parse(File.ReadAllText(_cache)) as JsonObject; } catch { }
    }

    public async Task StartAsync() { _timer.Start(); await RefreshAsync(false); }

    public async Task RefreshAsync(bool force)
    {
        try
        {
            double? lat = Num(_s.Doc["weather"]?["lat"]), lon = Num(_s.Doc["weather"]?["lon"]);
            if (lat == null || lon == null) { Publish(new JsonObject { ["type"] = "weather", ["needsCity"] = true }); return; }
            if (!force && Current?["fetched"] is JsonNode f && (DateTimeOffset.UtcNow.ToUnixTimeSeconds() - (long)f) < 29 * 60
                && Math.Abs((double)(Current["lat"] ?? 0) - lat.Value) < 0.01) { Publish(Current); return; }

            bool f2 = _s.Get("weather.units", "c") == "f";
            var url = FormattableString.Invariant(
                $"https://api.open-meteo.com/v1/forecast?latitude={lat:0.###}&longitude={lon:0.###}&current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,relative_humidity_2m,is_day&hourly=temperature_2m,weather_code&daily=temperature_2m_max,temperature_2m_min,sunrise,sunset,weather_code&forecast_days=4&timezone=auto{(f2 ? "&temperature_unit=fahrenheit&wind_speed_unit=mph" : "")}");
            var raw = JsonNode.Parse(await Http.GetStringAsync(url));
            var cur = raw["current"];
            var hours = new JsonArray();
            var ht = raw["hourly"]["time"].AsArray(); var hv = raw["hourly"]["temperature_2m"].AsArray(); var hc = raw["hourly"]["weather_code"].AsArray();
            var nowLocal = DateTime.Parse((string)cur["time"]);
            for (int i = 0, n = 0; i < ht.Count && n < 6; i++)
            {
                var t = DateTime.Parse((string)ht[i]);
                if (t < nowLocal.AddMinutes(-30) || t.Hour % 2 != 0) continue;
                hours.Add(new JsonObject { ["h"] = t.Hour, ["t"] = Math.Round((double)hv[i]), ["c"] = (int)hc[i] }); n++;
            }
            var days = new JsonArray();
            var d = raw["daily"];
            for (int i = 0; i < d["time"].AsArray().Count; i++)
                days.Add(new JsonObject { ["d"] = (string)d["time"][i], ["hi"] = Math.Round((double)d["temperature_2m_max"][i]), ["lo"] = Math.Round((double)d["temperature_2m_min"][i]), ["c"] = (int)d["weather_code"][i] });

            var msg = new JsonObject
            {
                ["type"] = "weather",
                ["city"] = _s.Get("weather.city", ""),
                ["lat"] = lat, ["lon"] = lon,
                ["units"] = f2 ? "F" : "C",
                ["temp"] = Math.Round((double)cur["temperature_2m"]),
                ["feels"] = Math.Round((double)cur["apparent_temperature"]),
                ["code"] = (int)cur["weather_code"],
                ["wind"] = Math.Round((double)cur["wind_speed_10m"]),
                ["humidity"] = (int)cur["relative_humidity_2m"],
                ["isDay"] = (int)cur["is_day"] == 1,
                ["hi"] = days.Count > 0 ? days[0]["hi"]?.DeepClone() : null,
                ["lo"] = days.Count > 0 ? days[0]["lo"]?.DeepClone() : null,
                ["sunrise"] = (string)d["sunrise"][0], ["sunset"] = (string)d["sunset"][0],
                ["hours"] = hours, ["days"] = days,
                ["fetched"] = DateTimeOffset.UtcNow.ToUnixTimeSeconds(),
            };
            Directory.CreateDirectory(Path.GetDirectoryName(_cache));
            File.WriteAllText(_cache, msg.ToJsonString());
            Publish(msg);
        }
        catch
        {
            if (Current != null) Publish(Current); // offline: keep showing the cached reading
        }
    }

    private void Publish(JsonObject m) { Current = m; Changed?.Invoke(m); }
    private static double? Num(JsonNode n) => n is JsonValue v && v.TryGetValue<double>(out var d) ? d : null;

    /// <summary>City name → coordinates (Open-Meteo geocoding).</summary>
    public static async Task<JsonObject> GeocodeAsync(string city)
    {
        var raw = JsonNode.Parse(await Http.GetStringAsync($"https://geocoding-api.open-meteo.com/v1/search?count=1&language=en&name={Uri.EscapeDataString(city)}"));
        var r = raw?["results"]?[0];
        if (r == null) return null;
        var label = (string)r["name"] + (r["country"] != null ? ", " + (string)r["country"] : "");
        return new JsonObject { ["city"] = label, ["lat"] = (double)r["latitude"], ["lon"] = (double)r["longitude"] };
    }

    /// <summary>Windows location services (only when the user presses "Use my location").</summary>
    public static async Task<JsonObject> LocateAsync()
    {
        var access = await Windows.Devices.Geolocation.Geolocator.RequestAccessAsync();
        if (access != Windows.Devices.Geolocation.GeolocationAccessStatus.Allowed) return null;
        var g = new Windows.Devices.Geolocation.Geolocator { DesiredAccuracy = Windows.Devices.Geolocation.PositionAccuracy.Default };
        var pos = await g.GetGeopositionAsync(TimeSpan.FromMinutes(30), TimeSpan.FromSeconds(10));
        double lat = Math.Round(pos.Coordinate.Point.Position.Latitude, 2), lon = Math.Round(pos.Coordinate.Point.Position.Longitude, 2);
        string name = "My location";
        try
        {
            var raw = JsonNode.Parse(await Http.GetStringAsync(FormattableString.Invariant($"https://geocoding-api.open-meteo.com/v1/search?count=1&name={lat},{lon}")));
            if (raw?["results"]?[0]?["name"] is JsonNode n) name = (string)n;
        }
        catch { }
        return new JsonObject { ["city"] = name, ["lat"] = lat, ["lon"] = lon };
    }
}

/// <summary>Windows toast notifications, if the OS grants this unpackaged app access. Degrades silently otherwise.</summary>
internal sealed class NotificationService
{
    private readonly SynchronizationContext _ui;
    private Windows.UI.Notifications.Management.UserNotificationListener _l;
    private readonly HashSet<uint> _seen = new();
    public event Action<JsonObject> Arrived;
    public string Status { get; private set; } = "not started";

    public NotificationService(SynchronizationContext ui) => _ui = ui;

    public async Task StartAsync()
    {
        try
        {
            _l = Windows.UI.Notifications.Management.UserNotificationListener.Current;
            var st = await _l.RequestAccessAsync();
            if (st != Windows.UI.Notifications.Management.UserNotificationListenerAccessStatus.Allowed) { Status = "denied: " + st; return; }
            foreach (var n in await _l.GetNotificationsAsync(Windows.UI.Notifications.NotificationKinds.Toast)) _seen.Add(n.Id);
            _l.NotificationChanged += (_, e) =>
            {
                if (e.ChangeKind != Windows.UI.Notifications.UserNotificationChangedKind.Added) return;
                _ui.Post(async _ => await OnAdded(e.UserNotificationId), null);
            };
            Status = "ok";
        }
        catch (Exception ex) { Status = "unavailable: " + ex.GetType().Name + " " + ex.Message; }
    }

    private async Task OnAdded(uint id)
    {
        try
        {
            if (!_seen.Add(id)) return;
            var n = _l.GetNotification(id);
            if (n == null) return;
            var binding = n.Notification.Visual.GetBinding(Windows.UI.Notifications.KnownNotificationBindings.ToastGeneric);
            var texts = binding?.GetTextElements().Select(t => t.Text).ToList() ?? new();
            string app = "";
            try { app = n.AppInfo.DisplayInfo.DisplayName; } catch { }
            Arrived?.Invoke(new JsonObject
            {
                ["type"] = "notification", ["app"] = app,
                ["title"] = texts.ElementAtOrDefault(0) ?? "", ["body"] = texts.ElementAtOrDefault(1) ?? "",
            });
            await Task.CompletedTask;
        }
        catch { }
    }
}
