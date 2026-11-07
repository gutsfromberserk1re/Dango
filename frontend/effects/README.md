# Adding an effect

Effect code is kept in the effects modules and connected to the editor and exporter:

1. Add the effect name, description, and icon to `index.js`.
2. Add its Controls fields and live preview behavior in `app.js`.
3. Add its FFmpeg filter implementation in `src-tauri/src/effects/mod.rs` so exports match the preview.
4. Add any new keyframeable numeric properties to `effectKeyframeProperties` in `index.js` and read their animated values in the exporter.

The editor loads the registry through `app.js`, and the Rust export command loads the native effects module through `src-tauri/src/main.rs`. A new module file must be registered in those entry points; copying an arbitrary file into this folder alone cannot provide native export behavior.
