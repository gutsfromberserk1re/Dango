# Dango

Dango is a native Windows desktop video editor prototype written in C17. The app UI uses Win32/GDI, Windows system backdrop support, and FFmpeg's C libraries for media probing.

## Build

Requirements: Windows 11 SDK, CMake 3.24+, MSVC Build Tools, and FFmpeg development headers plus `avformat`, `avcodec`, `avutil`, `avfilter`, `swscale`, and `swresample` libraries.

```powershell
cmake -S . -B build -A x64 -DFFMPEG_INCLUDE_DIR="C:/ffmpeg/include" `
  -DFFMPEG_avformat_LIBRARY="C:/ffmpeg/lib/avformat.lib" `
  -DFFMPEG_avcodec_LIBRARY="C:/ffmpeg/lib/avcodec.lib" `
  -DFFMPEG_avutil_LIBRARY="C:/ffmpeg/lib/avutil.lib" `
  -DFFMPEG_avfilter_LIBRARY="C:/ffmpeg/lib/avfilter.lib" `
  -DFFMPEG_swscale_LIBRARY="C:/ffmpeg/lib/swscale.lib" `
  -DFFMPEG_swresample_LIBRARY="C:/ffmpeg/lib/swresample.lib"
cmake --build build --config Release
```

All authored application code is C. Project model, FFmpeg integration, window lifecycle, and UI drawing are separated into distinct source files. The initial editor shell includes resizable panes, a magnetic multi-track model, clip dragging, selectable tools, and keyframe markers. Media decoding, composited preview, project serialization, and final export are not yet implemented.
