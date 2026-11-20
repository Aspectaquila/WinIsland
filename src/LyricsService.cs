using System.Security.Cryptography;
using System.Text;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace Frost;

/// <summary>
/// Time-synced lyrics from LRCLIB (lrclib.net: open-source, free, no account). Only the track's artist, title,
/// album and length are sent. Fetched on demand (when the island asks) and cached on disk, so each song is looked up once.
/// </summary>
internal static class LyricsService
{
    private static readonly HttpClient Http = new() { BaseAddress = new Uri("https://lrclib.net/"), Timeout = TimeSpan.FromSeconds(10) };
    private static readonly string CacheDir = Path.Combine(Settings.Data, "cache", "lyrics");
    private static readonly Dictionary<string, JsonObject> Mem = new();

    static LyricsService() => Http.DefaultRequestHeaders.UserAgent.ParseAdd("WinIsland/1.0");

    public static async Task<JsonObject> GetAsync(string artist, string title, string album, double duration, bool browser)
    {
        artist ??= ""; title ??= "";
        var result = new JsonObject { ["type"] = "lyrics", ["artist"] = artist, ["title"] = title, ["found"] = false };
        if (title.Trim().Length == 0) return result;

        var key = Hash($"{artist}|{title}|{Math.Round(duration)}");
        if (Mem.TryGetValue(key, out var hit)) return Stamp((JsonObject)hit.DeepClone(), result);
        var file = Path.Combine(CacheDir, key + ".json");
        try
        {
            if (File.Exists(file) && JsonNode.Parse(await File.ReadAllTextAsync(file)) is JsonObject cached)
            { Mem[key] = cached; return Stamp((JsonObject)cached.DeepClone(), result); }
        }
        catch { }

        JsonObject best = null;
        static bool Good(JsonObject o) => o != null && (o["syncedLyrics"] != null || o["instrumental"]?.GetValue<bool>() == true);
        try
        {
            // Try the title as the player reports it first (Spotify titles usually match LRCLIB exactly), then cleaned up.
            var tries = new List<(string a, string t)>();
            if (!browser && artist.Length > 0) tries.Add((artist, title.Trim()));
            var c = Clean(artist, title, browser);
            if (!tries.Contains(c)) tries.Add(c);
            foreach (var (a, t) in tries)
            {
                // 1) exact match (LRCLIB tolerates ±2 s of duration)
                if (a.Length > 0 && duration > 0)
                {
                    var q = $"api/get?artist_name={U(a)}&track_name={U(t)}&duration={Math.Round(duration)}" + (string.IsNullOrEmpty(album) || browser ? "" : $"&album_name={U(album)}");
                    using var r = await Http.GetAsync(q);
                    if (r.IsSuccessStatusCode && JsonNode.Parse(await r.Content.ReadAsStringAsync()) is JsonObject o) { if (Good(o)) { best = o; break; } best ??= o; }
                }
                // 2) search, preferring synced results with the closest length
                {
                    var q = a.Length > 0 ? $"api/search?artist_name={U(a)}&track_name={U(t)}" : $"api/search?q={U(t)}";
                    using var r = await Http.GetAsync(q);
                    if (r.IsSuccessStatusCode && JsonNode.Parse(await r.Content.ReadAsStringAsync()) is JsonArray arr)
                    {
                        var pick = arr.OfType<JsonObject>()
                            .Where(o => duration <= 0 || Math.Abs((o["duration"]?.GetValue<double>() ?? duration) - duration) < 12)
                            .OrderByDescending(o => o["syncedLyrics"] != null)
                            .ThenBy(o => duration > 0 ? Math.Abs((o["duration"]?.GetValue<double>() ?? 0) - duration) : 0)
                            .FirstOrDefault();
                        if (Good(pick)) { best = pick; break; }
                        best ??= pick;
                    }
                }
            }
        }
        catch { if (best == null) return result; /* offline: don't cache, try again next time */ }

        var entry = new JsonObject
        {
            ["found"] = best != null && (best["syncedLyrics"] != null || best["plainLyrics"] != null || best["instrumental"]?.GetValue<bool>() == true),
            ["synced"] = best?["syncedLyrics"]?.GetValue<string>(),
            ["plain"] = best?["plainLyrics"]?.GetValue<string>(),
            ["instrumental"] = best?["instrumental"]?.GetValue<bool>() ?? false,
        };
        Mem[key] = entry;
        try { Directory.CreateDirectory(CacheDir); await File.WriteAllTextAsync(file, entry.ToJsonString()); } catch { }
        return Stamp((JsonObject)entry.DeepClone(), result);
    }

    private static JsonObject Stamp(JsonObject entry, JsonObject result)
    {
        foreach (var (k, v) in entry.ToList()) result[k] = v?.DeepClone();
        return result;
    }

    // YouTube & co.: "Artist - Song (Official Video) [4K]" with the channel as artist ("ArtistVEVO", "Artist - Topic").
    private static (string artist, string title) Clean(string artist, string title, bool browser)
    {
        title = Regex.Replace(title, @"\s*[\(\[【][^\)\]】]*(official|video|audio|lyric|visuali[sz]er|mv|m/v|4k|hd|hq|remaster|live performance)[^\)\]】]*[\)\]】]", "", RegexOptions.IgnoreCase);
        title = Regex.Replace(title, @"\s*\|.*$", "").Trim();
        artist = Regex.Replace(artist, @"\s*-\s*Topic$|VEVO$|\s+Official$", "", RegexOptions.IgnoreCase).Trim();
        if (browser || artist.Length == 0)
        {
            var m = Regex.Match(title, @"^(.+?)\s+[-–—]\s+(.+)$");
            if (m.Success) { artist = m.Groups[1].Value.Trim(); title = m.Groups[2].Value.Trim(); }
        }
        title = Regex.Replace(title, @"\s*[\(\[](feat|ft|with|from)\b\.?[^\)\]]*[\)\]]", "", RegexOptions.IgnoreCase).Trim();
        title = Regex.Replace(title, @"\s+(feat|ft)\.\s.*$", "", RegexOptions.IgnoreCase).Trim();
        title = Regex.Replace(title, @"\s+-\s+(\d{4}\s+)?(remaster(ed)?|live|radio edit|mono|stereo|single version|bonus track).*$", "", RegexOptions.IgnoreCase).Trim();
        return (artist, title);
    }

    private static string U(string s) => Uri.EscapeDataString(s);
    private static string Hash(string s) => Convert.ToHexString(SHA1.HashData(Encoding.UTF8.GetBytes(s.ToLowerInvariant())))[..16];
}
