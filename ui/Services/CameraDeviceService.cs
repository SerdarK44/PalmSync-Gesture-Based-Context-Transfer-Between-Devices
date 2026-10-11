using Windows.Devices.Enumeration;
using Windows.Media.Devices;

namespace PalmSync.Tray.Services;

public record CameraDevice(string Id, string Name);

public static class CameraDeviceService
{
    public static async Task<List<CameraDevice>> GetCamerasAsync()
    {
        try
        {
            var selector = MediaDevice.GetVideoCaptureSelector();
            var devices = await DeviceInformation.FindAllAsync(selector);
            return devices.Select(d => new CameraDevice(d.Id, d.Name)).ToList();
        }
        catch (Exception ex)
        {
            return new() { new("", $"[Enumeration error: {ex.Message}]") };
        }
    }
}
