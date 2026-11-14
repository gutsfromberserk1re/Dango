# Dango Modern-only update

Base source: `gutsfromberserk1re/Dango` commit `e13d08b99768105cd7ea724dcac4986c4f6a7cc5` (2026-11-13).

## Interface changes
- Modern is the only workspace; Legacy and Neo selection UI is removed.
- Lapis is removed from the theme picker. Projects using a saved Lapis preference fall back to Cream Cheese.
- Color is a flat page with Basic Correction, Color Wheels, and HSL; curves/graphs and nested color folders are not shown.
- Rotation and HSL hue use the same drag-to-adjust value control as other values; no degree ruler. Value pads are larger.
- A single keyframe strip sits under the viewer and above the timeline. It displays Transform, Crop, Color, and attached-effect keyframes for the selected visual clip. The groups retain separate underlying animation data; the shared control adds/removes them together at the playhead.
- Preview canvas and its overlays stay centered and use the active project's aspect ratio. Export output dimensions now follow that project aspect ratio too.

## Export changes
- The output duration is calculated from timeline media/title/audio clips, not synthetic effect descriptors. This prevents an effect descriptor from extending a short edit into a long black tail.
- Synthetic attached-effect durations are capped to their owning clip when building the export payload.
- Replaced per-pixel opacity `geq` with FFmpeg's alpha channel mixer and avoid generating unused duplicate transition streams for clips that are not part of a valid cross-dissolve.
- Faster x264 presets retain the selected CRF quality target.
- FFmpeg progress is emitted through Tauri and displayed as an export progress bar.

## Validation
- `node --check frontend/app.js`: passed.
- HTML parser: passed.
- CSS parser: passed with zero top-level errors.
- FFmpeg smoke test: passed; the generated MP4 is 3.000 seconds at 640x360 and progress reached `progress=end`.
- Duration/aspect calculations were checked for a 3-second clip plus a 30-second synthetic effect descriptor, and for 16:9, 4:3, and 1:1 project ratios.
- A full Tauri/Rust build could not be run in this environment because `cargo`, `rustc`, and the local Tauri CLI dependency are unavailable. Native Windows build and interactive app testing remain to be done.

The reported 33-second export was not accompanied by its project file, so its exact timeline was not independently reproduced. The source path that lets synthetic effect descriptors extend export duration has been removed, which addresses that specific black-tail failure mode.
