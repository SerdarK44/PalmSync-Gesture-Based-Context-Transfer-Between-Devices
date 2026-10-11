# Third-Party Notices

PalmSync itself is licensed under the Apache License 2.0 (`LICENSE`). The
installer also distributes the components below, each under its own license.
PalmSync does not modify any of them.

## Camera 2.0 hand models (OpenCV Zoo, MediaPipe-derived)

The downloaded license text is bundled at `models/LICENSE.opencv-mediapipe.txt`.

### MediaPipe Palm Detection

- Repository: [opencv/palm_detection_mediapipe](https://huggingface.co/opencv/palm_detection_mediapipe)
- Pinned revision: `233e619dcea1759bf6de707b9b904fe30881ea55`
- File: `palm_detection_mediapipe_2023feb.onnx`
- SHA-256: `78FF51C38496B7FC8B8EBDB6CC8C1ABB02FA6C38427C6848254CDABA57FCCE7C`
- License: Apache License 2.0

### MediaPipe Hand Pose Estimation

- Repository: [opencv/handpose_estimation_mediapipe](https://huggingface.co/opencv/handpose_estimation_mediapipe)
- Pinned revision: `4b2a0b446e5cf2f11fb6b2c7251091c035d2c1f7`
- File: `handpose_estimation_mediapipe_2023feb.onnx`
- SHA-256: `DB0898AE717B76B075D9BF563AF315B29562E11F8DF5027A1EF07B02BEF6D81C`
- License: Apache License 2.0

The model repositories identify the assets as MediaPipe-derived models made
available for OpenCV DNN.

## OpenCV 4.x runtime libraries

- Project: [opencv.org](https://opencv.org) — `opencv_*.dll`
- License: Apache License 2.0
- Bundled dependencies built through vcpkg (zlib, libjpeg-turbo, libpng,
  libtiff, libwebp, liblzma, protobuf, abseil) are under their respective
  permissive licenses (zlib, IJG/BSD, libpng, libtiff, BSD-3-Clause,
  public domain/0BSD, BSD-3-Clause, Apache-2.0).

## OpenCV Haar cascades

- Files: `haarcascade_frontalface_default.xml`, `haarcascade_profileface.xml`
- License: Intel License Agreement for Open Source Computer Vision Library
  (BSD-style); the full text is kept in the header of each XML file.

## Microsoft Visual C++ runtime

- Files: `msvcp140*.dll`, `vcruntime140*.dll`
- Redistributed under the Microsoft Visual Studio redistribution terms.

## .NET 8 Desktop Runtime

- Installed by the setup only when missing.
- License: MIT (dotnet/runtime, dotnet/wpf, dotnet/winforms).

## Rust crates and NuGet packages

The daemon and tray app are compiled from open-source Rust crates and NuGet
packages (MIT and/or Apache-2.0 licensed, e.g. tokio, serde, aes-gcm,
tungstenite, Hardcodet.NotifyIcon.Wpf). Their license texts are available in
each package's source distribution.
