# Bundled redistributables

The installer embeds the **.NET 8 Desktop Runtime (x64)** so end users never
have to download or install anything manually.

This `.exe` is git-ignored (it's ~56 MB). Fetch it before building the installer:

```powershell
curl -L -o windowsdesktop-runtime-8.0-win-x64.exe `
  "https://aka.ms/dotnet/8.0/windowsdesktop-runtime-win-x64.exe"
```

The installer runs it silently (`/install /quiet /norestart`) only when the
runtime is not already present on the target machine.
