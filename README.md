# Dango

Dango is a Tauri v2 desktop video editor prototype. The interface runs in the system webview; Rust handles native dialogs, window actions, project files, and optional FFprobe duration probing.

## Run on Windows

Install Rust, Node.js 22+, the Visual Studio C++ Build Tools workload, CMake, and WebView2. FFmpeg is optional: if `ffprobe` is on `PATH`, Dango uses it for media duration; the webview also probes imported media metadata. Run:

```powershell
npm install
npm run dev
```

## Current editor features

- V1 and V2 plus A1 are visible by default. Dragging above the top video lane reveals another video track; dragging below the audio lanes reveals more audio tracks. Placement is magnetic.
- Import media to the bin, then drag it into the timeline or double-click to add another instance.
- Preview imported video and audio, seek by clicking or dragging the playhead, and change preview playback rate.
- Select, blade, hand, and text tools; title clips can be edited in the Kontrol panel.
- Clip controls and draggable keyframes interpolate X/Y position, size, rotation, speed, and opacity.
- Fade, swirl, wave, blur, glow, warm film, vignette, and sharpen overlays are saved with the project and preview on the clip. Fade direction and duration are adjustable.
- Save, Save As, and Open use native file dialogs; New project resets the current edit. Keyboard shortcuts can be edited in Settings.
- Viewer/inspector and editor/timeline dividers are resizable.

Frame-accurate FFmpeg rendering/export and complete parity between webview previews and encoded output are still future work.
