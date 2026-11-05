# Dango

Dango is a Tauri v2 desktop video-editor prototype. Rust owns native dialogs/window operations and probes media with FFprobe; the editor UI runs in Tauri's webview.

## Run on Windows

Install Rust, Node.js 22+, the Visual Studio C++ Build Tools workload, CMake, and WebView2. Install FFmpeg and make `ffprobe` available on `PATH` for imported duration metadata. Run:

```powershell
npm install
npm run dev
```

## Editor prototype

- Video tracks V1 and V2 and audio track A1 start visible. Dragging into the outermost lane reveals V3 or A2/A3. Magnetic placement is always on.
- Effects are overlay blocks attached to video clips. Fade starts as a black overlay and can be switched between fade-in and fade-out; duration and intensity controls have individual reset buttons.
- Preview quality changes playback rate; imported video can preview through the Tauri asset protocol.
- The pane dividers show column/row resize cursors and can be dragged.

Frame-accurate FFmpeg decoding/export, native project persistence, and full implementations for every visual effect remain future work.
