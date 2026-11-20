using System.Text.Json;
using System.Text.Json.Nodes;

namespace Frost;

/// <summary>
/// Settings live in one JSON document (FrostDesktop\settings.json). The web surfaces read it directly,
/// the host only reads the handful of keys it needs. Unknown keys are preserved; missing keys fall back to defaults.
/// </summary>
internal sealed class Settings
{
    public static readonly string Root = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, ".."));
    /// <summary>Where settings and caches live: the install folder when it is writable (portable / per-user install),
    /// otherwise %LOCALAPPDATA%\WinIsland (e.g. when installed under Program Files).</summary>
    public static readonly string Data = PickDataFolder();
    public static readonly string FilePath = Path.Combine(Data, "settings.json");

    private static string PickDataFolder()
    {
        try
        {
            var probe = Path.Combine(Root, ".write-test");
            File.WriteAllText(probe, ""); File.Delete(probe);
            return Root;
        }
        catch
        {
            var d = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "WinIsland");
            Directory.CreateDirectory(d);
            return d;
        }
    }

    public JsonObject Doc { get; private set; }
    public event Action<JsonObject> Changed; // arg = the patch that was applied

    private const string Defaults = """
    {
      "island":    { "style": "macos", "enabled": true, "time": true, "weather": true, "media": true, "volume": true, "system": true,
                     "notifications": true, "events": true, "collapseDelay": 0, "hideInFullscreen": true, "glow": true,
                     "visualizer": true, "trackPopup": true, "lyrics": true, "lyricsNotch": true, "privacy": true, "hoverExpand": false, "idleInfo": true },
      "weather":   { "city": "", "lat": null, "lon": null, "units": "c" },
      "general":   { "autostart": true, "hotkey": true }
    }
    """;

    public Settings() => Load();

    public void Load()
    {
        var doc = (JsonObject)JsonNode.Parse(Defaults);
        try
        {
            if (File.Exists(FilePath) && JsonNode.Parse(File.ReadAllText(FilePath)) is JsonObject user)
                Merge(doc, user);
        }
        catch { /* corrupt file: keep defaults, it will be rewritten on next save */ }
        Doc = doc;
    }

    public void Save()
    {
        var tmp = FilePath + ".tmp";
        File.WriteAllText(tmp, Doc.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
        File.Move(tmp, FilePath, true);
    }

    public void Apply(JsonObject patch)
    {
        Merge(Doc, patch);
        Save();
        Changed?.Invoke(patch);
    }

    public void ResetToDefaults()
    {
        Doc = (JsonObject)JsonNode.Parse(Defaults);
        Save();
        Changed?.Invoke(Doc);
    }

    private static void Merge(JsonObject target, JsonObject src)
    {
        foreach (var (k, v) in src.ToList())
        {
            if (v is JsonObject so && target[k] is JsonObject to) Merge(to, so);
            else target[k] = v?.DeepClone();
        }
    }

    public T Get<T>(string path, T fallback = default)
    {
        JsonNode n = Doc;
        foreach (var part in path.Split('.')) { n = n?[part]; if (n == null) return fallback; }
        try { return n.GetValue<T>(); } catch { return fallback; }
    }

    public bool B(string path) => Get(path, false);
    public double D(string path) { try { return Get<double>(path); } catch { return 0; } }

    public static bool Touches(JsonObject patch, string section) => patch.ContainsKey(section);
}
