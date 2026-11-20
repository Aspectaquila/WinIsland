using System.Runtime.InteropServices;

namespace Frost;

/// <summary>System volume via Core Audio, with change *notifications* (no polling). Follows default-device changes.</summary>
internal sealed class AudioService : IAudioEndpointVolumeCallback, IMMNotificationClient, IDisposable
{
    private IMMDeviceEnumerator _enum;
    private IAudioEndpointVolume _vol;
    private IAudioMeterInformation _meter;
    private System.Windows.Forms.Timer _meterTimer;
    private readonly float[] _peaks = new float[2];
    public event Action<float, float> Levels; // left, right peak (0..1), only while someone listens

    /// <summary>Real output levels for the island visualizer. Samples at 25 Hz, only while enabled.</summary>
    public void SetMetering(bool on)
    {
        _meterTimer ??= CreateMeterTimer();
        if (on && !_meterTimer.Enabled) _meterTimer.Start();
        else if (!on && _meterTimer.Enabled) { _meterTimer.Stop(); Levels?.Invoke(0, 0); }
    }

    private System.Windows.Forms.Timer CreateMeterTimer()
    {
        var t = new System.Windows.Forms.Timer { Interval = 66 }; // 15 Hz is plenty for a 5-bar visualizer
        t.Tick += (_, _) =>
        {
            if (_meter == null) return;
            try
            {
                _meter.GetMeteringChannelCount(out var n);
                if (n >= 2) { _meter.GetChannelsPeakValues(2, _peaks); Levels?.Invoke(_peaks[0], _peaks[1]); }
                else { _meter.GetPeakValue(out var p); Levels?.Invoke(p, p); }
            }
            catch { }
        };
        return t;
    }
    private readonly SynchronizationContext _ui;
    private static Guid _ctx = Guid.NewGuid(); // identifies changes we make ourselves
    public event Action<float, bool, bool> Changed; // level, muted, causedByUs

    public float Level { get; private set; }
    public bool Muted { get; private set; }

    public AudioService(SynchronizationContext ui) => _ui = ui;

    public void Start()
    {
        try
        {
            _enum = (IMMDeviceEnumerator)Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")));
            _enum.RegisterEndpointNotificationCallback(this);
            Bind();
        }
        catch { }
    }

    private void Bind()
    {
        try
        {
            if (_vol != null) { try { _vol.UnregisterControlChangeNotify(this); } catch { } Marshal.ReleaseComObject(_vol); _vol = null; }
            if (_enum.GetDefaultAudioEndpoint(0 /*eRender*/, 1 /*eMultimedia*/, out var dev) != 0 || dev == null) return;
            var iid = typeof(IAudioEndpointVolume).GUID;
            dev.Activate(ref iid, 0x17 /*CLSCTX_ALL*/, IntPtr.Zero, out var o);
            _vol = (IAudioEndpointVolume)o;
            _vol.RegisterControlChangeNotify(this);
            _vol.GetMasterVolumeLevelScalar(out var l); _vol.GetMute(out var mu);
            Level = l; Muted = mu;
            try
            {
                var mid = typeof(IAudioMeterInformation).GUID;
                dev.Activate(ref mid, 0x17, IntPtr.Zero, out var mo);
                _meter = (IAudioMeterInformation)mo;
            }
            catch { _meter = null; }
            Marshal.ReleaseComObject(dev);
        }
        catch { }
    }

    public void SetLevel(float v)
    {
        try { _vol?.SetMasterVolumeLevelScalar(Math.Clamp(v, 0, 1), ref _ctx); if (Muted && v > 0) _vol?.SetMute(false, ref _ctx); } catch { }
    }
    public void ToggleMute() { try { _vol?.SetMute(!Muted, ref _ctx); } catch { } }

    // IAudioEndpointVolumeCallback (called on an audio thread)
    int IAudioEndpointVolumeCallback.OnNotify(IntPtr data)
    {
        var ctx = Marshal.PtrToStructure<Guid>(data);
        bool muted = Marshal.ReadInt32(data, 16) != 0;
        float level = BitConverter.Int32BitsToSingle(Marshal.ReadInt32(data, 20));
        bool ours = ctx == _ctx;
        _ui.Post(_ => { Level = level; Muted = muted; Changed?.Invoke(level, muted, ours); }, null);
        return 0;
    }

    // IMMNotificationClient
    int IMMNotificationClient.OnDefaultDeviceChanged(int flow, int role, string id)
    {
        if (flow == 0 && role == 1) _ui.Post(_ => { Bind(); Changed?.Invoke(Level, Muted, false); }, null);
        return 0;
    }
    int IMMNotificationClient.OnDeviceStateChanged(string id, int state) => 0;
    int IMMNotificationClient.OnDeviceAdded(string id) => 0;
    int IMMNotificationClient.OnDeviceRemoved(string id) => 0;
    int IMMNotificationClient.OnPropertyValueChanged(string id, PropertyKey key) => 0;

    public void Dispose()
    {
        try { _vol?.UnregisterControlChangeNotify(this); } catch { }
        try { _enum?.UnregisterEndpointNotificationCallback(this); } catch { }
    }
}

[StructLayout(LayoutKind.Sequential)] internal struct PropertyKey { public Guid fmtid; public int pid; }

[ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IMMDeviceEnumerator
{
    [PreserveSig] int EnumAudioEndpoints(int flow, int mask, out IntPtr devices);
    [PreserveSig] int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice device);
    [PreserveSig] int GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IMMDevice device);
    [PreserveSig] int RegisterEndpointNotificationCallback(IMMNotificationClient client);
    [PreserveSig] int UnregisterEndpointNotificationCallback(IMMNotificationClient client);
}

[ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IMMDevice
{
    [PreserveSig] int Activate(ref Guid iid, int clsCtx, IntPtr p, [MarshalAs(UnmanagedType.IUnknown)] out object iface);
}

[ComImport, Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IAudioEndpointVolume
{
    [PreserveSig] int RegisterControlChangeNotify(IAudioEndpointVolumeCallback cb);
    [PreserveSig] int UnregisterControlChangeNotify(IAudioEndpointVolumeCallback cb);
    [PreserveSig] int GetChannelCount(out uint n);
    [PreserveSig] int SetMasterVolumeLevel(float db, ref Guid ctx);
    [PreserveSig] int SetMasterVolumeLevelScalar(float v, ref Guid ctx);
    [PreserveSig] int GetMasterVolumeLevel(out float db);
    [PreserveSig] int GetMasterVolumeLevelScalar(out float v);
    [PreserveSig] int SetChannelVolumeLevel(uint ch, float db, ref Guid ctx);
    [PreserveSig] int SetChannelVolumeLevelScalar(uint ch, float v, ref Guid ctx);
    [PreserveSig] int GetChannelVolumeLevel(uint ch, out float db);
    [PreserveSig] int GetChannelVolumeLevelScalar(uint ch, out float v);
    [PreserveSig] int SetMute([MarshalAs(UnmanagedType.Bool)] bool mute, ref Guid ctx);
    [PreserveSig] int GetMute([MarshalAs(UnmanagedType.Bool)] out bool mute);
}

[ComImport, Guid("C02216F6-8C67-4B5B-9D00-D008E73E0064"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IAudioMeterInformation
{
    [PreserveSig] int GetPeakValue(out float peak);
    [PreserveSig] int GetMeteringChannelCount(out uint count);
    [PreserveSig] int GetChannelsPeakValues(uint count, [Out, MarshalAs(UnmanagedType.LPArray)] float[] peaks);
    [PreserveSig] int QueryHardwareSupport(out uint mask);
}

[ComImport, Guid("657804FA-D6AD-4496-8A60-352752AF4F89"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IAudioEndpointVolumeCallback
{
    [PreserveSig] int OnNotify(IntPtr data);
}

[ComImport, Guid("7991EEC9-7E89-4D85-8390-6C703CEC60C0"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IMMNotificationClient
{
    [PreserveSig] int OnDeviceStateChanged([MarshalAs(UnmanagedType.LPWStr)] string id, int state);
    [PreserveSig] int OnDeviceAdded([MarshalAs(UnmanagedType.LPWStr)] string id);
    [PreserveSig] int OnDeviceRemoved([MarshalAs(UnmanagedType.LPWStr)] string id);
    [PreserveSig] int OnDefaultDeviceChanged(int flow, int role, [MarshalAs(UnmanagedType.LPWStr)] string id);
    [PreserveSig] int OnPropertyValueChanged([MarshalAs(UnmanagedType.LPWStr)] string id, PropertyKey key);
}
