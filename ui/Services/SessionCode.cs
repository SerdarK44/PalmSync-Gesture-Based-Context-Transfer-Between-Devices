using System.Net;
using System.Text;

namespace PalmSync.Tray.Services;

/// Encodes an IPv4 address + port into a short human-shareable code so the
/// user never sees or types an IP. 6 bytes (4 IP + 2 port) → Crockford Base32
/// (10 chars), formatted as XXXXX-XXXXX. Ambiguous letters (I,L,O,U) excluded.
public static class SessionCode
{
    private const string Alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // Crockford

    public static string Encode(IPAddress ip, ushort port)
    {
        var ipBytes = ip.GetAddressBytes(); // 4 bytes for IPv4
        if (ipBytes.Length != 4)
            throw new ArgumentException("Only IPv4 is supported", nameof(ip));

        Span<byte> data = stackalloc byte[6];
        ipBytes.CopyTo(data);
        data[4] = (byte)(port >> 8);
        data[5] = (byte)(port & 0xFF);

        var raw = Base32Encode(data); // 10 chars
        return $"{raw[..5]}-{raw[5..]}";
    }

    public static bool TryDecode(string code, out IPAddress ip, out ushort port)
    {
        ip = IPAddress.None;
        port = 0;

        var cleaned = Normalize(code);
        if (cleaned.Length != 10) return false;

        try
        {
            var data = Base32Decode(cleaned); // 6 bytes
            if (data.Length != 6) return false;

            ip = new IPAddress(new[] { data[0], data[1], data[2], data[3] });
            port = (ushort)((data[4] << 8) | data[5]);
            return port != 0;
        }
        catch
        {
            return false;
        }
    }

    /// Uppercase, strip separators/spaces, map common look-alikes to the
    /// Crockford alphabet (I/L→1, O→0, U→V) so users can be sloppy.
    private static string Normalize(string code)
    {
        var sb = new StringBuilder(code.Length);
        foreach (var raw in code.ToUpperInvariant())
        {
            var c = raw switch
            {
                'I' or 'L' => '1',
                'O' => '0',
                'U' => 'V',
                _ => raw,
            };
            if (Alphabet.IndexOf(c) >= 0) sb.Append(c);
        }
        return sb.ToString();
    }

    private static string Base32Encode(ReadOnlySpan<byte> data)
    {
        var sb = new StringBuilder();
        int buffer = 0, bits = 0;
        foreach (var b in data)
        {
            buffer = (buffer << 8) | b;
            bits += 8;
            while (bits >= 5)
            {
                bits -= 5;
                sb.Append(Alphabet[(buffer >> bits) & 0x1F]);
            }
        }
        if (bits > 0)
            sb.Append(Alphabet[(buffer << (5 - bits)) & 0x1F]);
        return sb.ToString();
    }

    private static byte[] Base32Decode(string s)
    {
        var bytes = new List<byte>();
        int buffer = 0, bits = 0;
        foreach (var c in s)
        {
            var val = Alphabet.IndexOf(c);
            if (val < 0) throw new FormatException($"bad char '{c}'");
            buffer = (buffer << 5) | val;
            bits += 5;
            if (bits >= 8)
            {
                bits -= 8;
                bytes.Add((byte)((buffer >> bits) & 0xFF));
            }
        }
        return bytes.ToArray();
    }
}
