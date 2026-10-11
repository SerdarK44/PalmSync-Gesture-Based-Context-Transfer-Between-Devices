using System.IO;
using System.IO.Pipes;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace PalmSync.Tray.Services;

/// Named-pipe server that receives JPEG frames from palmsync-eye.exe (--preview-pipe flag)
/// and fires FrameReceived for the live camera preview in the Home window.
/// The eye process retries the connection every ~2 s, so the server keeps
/// listening again after each disconnect (window reopened, eye restarted).
public static class CameraPreviewService
{
    private static CancellationTokenSource? _cts;

    public const string PipeName = "palmsync-preview";

    public static event Action<ImageSource>? FrameReceived;

    public static bool IsRunning => _cts is { IsCancellationRequested: false };

    /// Starts listening (no-op if already listening).
    public static Task StartAsync()
    {
        if (IsRunning) return Task.CompletedTask;
        _cts = new CancellationTokenSource();
        _ = RunAsync(_cts.Token);
        return Task.CompletedTask;
    }

    private static async Task RunAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                // Buffer 256 KB – plenty for a 480×360 JPEG at q72 (~20–40 KB).
                using var server = new NamedPipeServerStream(
                    PipeName, PipeDirection.In, 1,
                    PipeTransmissionMode.Byte, PipeOptions.Asynchronous,
                    256 * 1024, 256 * 1024);

                await server.WaitForConnectionAsync(ct);

                var lenBuf = new byte[4];
                while (!ct.IsCancellationRequested)
                {
                    await ReadExactAsync(server, lenBuf, 4, ct);
                    var len = BitConverter.ToInt32(lenBuf, 0);
                    if (len <= 0 || len > 5 * 1024 * 1024) break;

                    var jpeg = new byte[len];
                    await ReadExactAsync(server, jpeg, len, ct);

                    var bi = new BitmapImage();
                    bi.BeginInit();
                    bi.StreamSource = new MemoryStream(jpeg);
                    bi.CacheOption = BitmapCacheOption.OnLoad;
                    bi.EndInit();
                    bi.Freeze();

                    FrameReceived?.Invoke(bi);
                }
            }
            catch (OperationCanceledException) { return; }
            catch
            {
                // eye.exe exited / pipe broken / bad frame – listen again shortly
                try { await Task.Delay(500, ct); } catch (OperationCanceledException) { return; }
            }
        }
    }

    private static async Task ReadExactAsync(Stream s, byte[] buf, int count, CancellationToken ct)
    {
        int offset = 0;
        while (offset < count)
        {
            int n = await s.ReadAsync(buf, offset, count - offset, ct);
            if (n == 0) throw new EndOfStreamException("Pipe disconnected");
            offset += n;
        }
    }

    public static async Task StopAsync()
    {
        // Cancel only (no Dispose): the reader loop may still be observing the token.
        _cts?.Cancel();
        _cts = null;
        await Task.Yield();
    }
}
