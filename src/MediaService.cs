using System.Text.Json.Nodes;
using Windows.Media.Control;
using Windows.Storage.Streams;

namespace Frost;

/// <summary>
/// Now-playing info + transport controls via Windows' Global System Media Transport Controls. Event-driven.
/// Every session (Spotify, each browser, …) is tracked so the island can offer a source switcher; the one being
/// controlled is the user's pick, or Windows' "current" session when there is no pick.
/// </summary>
internal sealed class MediaService
{
    private GlobalSystemMediaTransportControlsSessionManager _mgr;
    private GlobalSystemMediaTransportControlsSession _session;
    private readonly List<GlobalSystemMediaTransportControlsSession> _watched = new();
    private string _pinned; // source id the user picked; null = follow Windows' current session
    private string _sessionId;
    private readonly SynchronizationContext _ui;
    private string _lastArtKey;
    private string _lastArt;
    public JsonObject Current { get; private set; } = new() { ["type"] = "media", ["active"] = false };
    public event Action<JsonObject> Changed;

    public MediaService(SynchronizationContext ui) => _ui = ui;

    public async Task StartAsync()
    {
        try
        {
            _mgr = await GlobalSystemMediaTransportControlsSessionManager.RequestAsync();
            // Windows switches its current session when another app starts playing: follow that, dropping a stale pick.
            _mgr.CurrentSessionChanged += (_, _) => _ui.Post(_ => { _pinned = null; Attach(); }, null);
            _mgr.SessionsChanged += (_, _) => _ui.Post(_ => { Watch(); Attach(); }, null);
            Watch();
            Attach();
        }
        catch { /* media integration unavailable */ }
    }

    /// <summary>Ids are "aumid#n" (n = occurrence), so two tabs of the same browser stay distinguishable.</summary>
    private List<(string id, GlobalSystemMediaTransportControlsSession s)> Sessions()
    {
        var list = new List<(string, GlobalSystemMediaTransportControlsSession)>();
        try
        {
            var seen = new Dictionary<string, int>();
            foreach (var s in _mgr.GetSessions())
            {
                var a = s.SourceAppUserModelId ?? "";
                seen[a] = seen.GetValueOrDefault(a) + 1;
                list.Add(($"{a}#{seen[a]}", s));
            }
        }
        catch { }
        return list;
    }

    // Props/playback of *all* sessions keep the switcher's list fresh; timeline only matters for the controlled one.
    private void Watch()
    {
        foreach (var s in _watched) { try { s.MediaPropertiesChanged -= OnProps; s.PlaybackInfoChanged -= OnPlayback; } catch { } }
        _watched.Clear();
        if (_mgr == null) return;
        foreach (var (_, s) in Sessions()) { s.MediaPropertiesChanged += OnProps; s.PlaybackInfoChanged += OnPlayback; _watched.Add(s); }
    }

    private void Attach()
    {
        if (_mgr == null) return;
        var all = Sessions();
        GlobalSystemMediaTransportControlsSession s = null;
        if (_pinned != null) s = all.FirstOrDefault(x => x.id == _pinned).s;
        if (s == null) { _pinned = null; s = _mgr.GetCurrentSession(); }
        var id = s != null ? IdOf(s, all) : null;
        if (s != null && _session != null && id == _sessionId) { _ = RefreshAsync(); return; }
        if (_session != null) { try { _session.TimelinePropertiesChanged -= OnTimeline; } catch { } }
        _session = s; _sessionId = id;
        if (_session != null) _session.TimelinePropertiesChanged += OnTimeline;
        _ = RefreshAsync();
    }

    private static string IdOf(GlobalSystemMediaTransportControlsSession s, List<(string id, GlobalSystemMediaTransportControlsSession s)> all)
    {
        foreach (var x in all) if (ReferenceEquals(x.s, s)) return x.id;
        var first = all.FirstOrDefault(x => x.s.SourceAppUserModelId == s.SourceAppUserModelId);
        return first.id;
    }

    /// <summary>Control a specific source (from the island's switcher).</summary>
    public void Select(string id)
    {
        if (_mgr == null || string.IsNullOrEmpty(id)) return;
        if (Sessions().All(x => x.id != id)) return;
        _pinned = id;
        Attach();
    }

    private void OnProps(GlobalSystemMediaTransportControlsSession s, MediaPropertiesChangedEventArgs e) => _ui.Post(_ => _ = RefreshAsync(), null);
    private void OnPlayback(GlobalSystemMediaTransportControlsSession s, PlaybackInfoChangedEventArgs e) => _ui.Post(_ => _ = RefreshAsync(), null);
    private void OnTimeline(GlobalSystemMediaTransportControlsSession s, TimelinePropertiesChangedEventArgs e) => _ui.Post(_ => _ = RefreshAsync(), null);

    /// <summary>All sessions with something loaded, for the switcher.</summary>
    private async Task<JsonArray> SourcesAsync()
    {
        var arr = new JsonArray();
        var all = Sessions();
        var selId = _sessionId;
        foreach (var (id, s) in all)
        {
            try
            {
                var p = await s.TryGetMediaPropertiesAsync();
                if (string.IsNullOrEmpty(p?.Title)) continue;
                var aumid = s.SourceAppUserModelId ?? "";
                arr.Add(new JsonObject
                {
                    ["id"] = id,
                    ["app"] = FriendlyApp(aumid) is { Length: > 0 } n ? n : "Media",
                    ["browser"] = IsBrowser(aumid),
                    ["title"] = p.Title,
                    ["artist"] = p.Artist ?? "",
                    ["playing"] = s.GetPlaybackInfo()?.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing,
                    ["selected"] = id == selId,
                });
            }
            catch { }
        }
        return arr;
    }

    // Firefox-family browsers (Firefox, Zen, LibreWolf, Floorp…) register a 16-hex-digit hash of their install folder.
    private static bool IsGeckoHash(string aumid) => System.Text.RegularExpressions.Regex.IsMatch(aumid ?? "", "^[0-9A-Fa-f]{16}$");
    private static string GeckoName()
    {
        foreach (var (proc, name) in new[] { ("zen", "Zen"), ("firefox", "Firefox"), ("librewolf", "LibreWolf"), ("floorp", "Floorp"), ("waterfox", "Waterfox") })
        {
            var ps = System.Diagnostics.Process.GetProcessesByName(proc);
            bool any = ps.Length > 0; foreach (var p in ps) p.Dispose();
            if (any) return name;
        }
        return "Browser";
    }

    private static bool IsBrowser(string aumid)
    {
        var a = aumid.ToLowerInvariant();
        return IsGeckoHash(aumid) || a.Contains("chrome") || a.Contains("msedge") || a.Contains("firefox") || a.Contains("zen") || a.Contains("brave") || a.Contains("opera") || a.Contains("vivaldi");
    }

    private int _refreshing;
    private async Task RefreshAsync()
    {
        if (Interlocked.Exchange(ref _refreshing, 1) == 1) { _again = true; return; }
        try
        {
            do
            {
                _again = false;
                var msg = new JsonObject { ["type"] = "media", ["active"] = false };
                var s = _session;
                if (s != null)
                {
                    var props = await s.TryGetMediaPropertiesAsync();
                    var pb = s.GetPlaybackInfo();
                    var tl = s.GetTimelineProperties();
                    msg["active"] = !string.IsNullOrEmpty(props?.Title);
                    msg["title"] = props?.Title ?? "";
                    msg["artist"] = props?.Artist ?? "";
                    msg["album"] = props?.AlbumTitle ?? "";
                    msg["app"] = FriendlyApp(s.SourceAppUserModelId);
                    msg["browser"] = IsBrowser(s.SourceAppUserModelId ?? "");
                    msg["playing"] = pb?.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing;
                    msg["canPrev"] = pb?.Controls?.IsPreviousEnabled ?? false;
                    msg["canNext"] = pb?.Controls?.IsNextEnabled ?? false;
                    msg["position"] = tl?.Position.TotalSeconds ?? 0;
                    msg["duration"] = tl != null ? (tl.EndTime - tl.StartTime).TotalSeconds : 0;
                    msg["posStamp"] = tl != null ? tl.LastUpdatedTime.ToUnixTimeMilliseconds() : 0;
                    var key = $"{props?.Title}|{props?.Artist}|{props?.AlbumTitle}";
                    if (key != _lastArtKey)
                    {
                        _lastArtKey = key;
                        _lastArt = await ReadThumbAsync(props?.Thumbnail);
                    }
                    msg["art"] = _lastArt;
                }
                msg["sources"] = await SourcesAsync();
                Current = msg;
                Changed?.Invoke(msg);
            } while (_again);
        }
        catch { }
        finally { Interlocked.Exchange(ref _refreshing, 0); }
    }
    private volatile bool _again;

    private static async Task<string> ReadThumbAsync(IRandomAccessStreamReference r)
    {
        if (r == null) return null;
        try
        {
            using var s = await r.OpenReadAsync();
            if (s.Size == 0 || s.Size > 4_000_000) return null;
            var buf = new byte[s.Size];
            using var rd = new DataReader(s);
            await rd.LoadAsync((uint)s.Size);
            rd.ReadBytes(buf);
            var mime = s.ContentType is { Length: > 0 } ct ? ct : "image/png";
            return $"data:{mime};base64,{Convert.ToBase64String(buf)}";
        }
        catch { return null; }
    }

    private static string _geckoName;
    private static string FriendlyApp(string aumid)
    {
        if (string.IsNullOrEmpty(aumid)) return "";
        if (IsGeckoHash(aumid)) return _geckoName ??= GeckoName();
        var a = aumid.ToLowerInvariant();
        if (a.Contains("spotify")) return "Spotify";
        if (a.Contains("chrome")) return "Chrome";
        if (a.Contains("msedge")) return "Edge";
        if (a.Contains("firefox")) return "Firefox";
        if (a.Contains("brave")) return "Brave";
        if (a.Contains("opera")) return "Opera";
        if (a.Contains("zen")) return "Browser";
        if (a.Contains("zunemusic") || a.Contains("media")) return "Media Player";
        if (a.Contains("discord")) return "Discord";
        if (a.Contains("vlc")) return "VLC";
        if (a.Contains("foobar")) return "foobar2000";
        if (a.Contains("tidal")) return "TIDAL";
        if (a.Contains("applemusic") || a.Contains("itunes")) return "Apple Music";
        var n = aumid.Split('!', '\\', '/').Last();
        if (n.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)) n = n[..^4];
        // Browsers/PWAs often register opaque hashes as their media id; don't show those.
        if (System.Text.RegularExpressions.Regex.IsMatch(n, "^[0-9A-Fa-f]{10,}$") || n.Length > 28 || n.Contains('{')) return "";
        return n;
    }

    public async void Seek(double seconds)
    {
        var s = _session;
        if (s == null) return;
        try
        {
            var tl = s.GetTimelineProperties();
            await s.TryChangePlaybackPositionAsync((tl.StartTime + TimeSpan.FromSeconds(Math.Max(0, seconds))).Ticks);
        }
        catch { }
    }

    public async void Command(string action)
    {
        var s = _session;
        if (s == null) return;
        try
        {
            switch (action)
            {
                case "toggle": await s.TryTogglePlayPauseAsync(); break;
                case "next": await s.TrySkipNextAsync(); break;
                case "prev": await s.TrySkipPreviousAsync(); break;
            }
        }
        catch { }
    }
}
