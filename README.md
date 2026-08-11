# ANNO

A minimal desktop image annotator built with Rust and egui.

## Run

```sh
cargo run
```

Open or drop a PNG, JPEG, WebP, BMP, or TIFF image. Drag over the image to draw a
region, then type its label in the inspector. `Cmd/Ctrl+S` exports the image path,
dimensions, labels, and pixel bounds as JSON.

Shortcuts:

- `Cmd/Ctrl+O` — open an image
- `Cmd/Ctrl+S` — save annotations
- `Delete` / `Backspace` — remove the selected region
- `Escape` — cancel drawing or deselect
