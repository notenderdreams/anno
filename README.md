# anno

![anno interface](assets/anno-demo.png)

A focused desktop image annotator built with Rust and egui for preparing structured
computer-vision datasets. Draw precise regions, organize them into visual
hierarchies, and export nested JSON.

## Run

```sh
cargo run
```

Open or drop an image, then drag over it to create a region. Use the inspector to
edit its label and color.

## Controls

- `Cmd/Ctrl+O` — open an image
- `Cmd/Ctrl+S` — save project (.anno)
- `Cmd/Ctrl+E` — export annotations JSON
- `Cmd/Ctrl+Z` — undo
- `Cmd/Ctrl+Shift+Z` / `Cmd/Ctrl+Y` — redo
- `Delete` / `Backspace` — remove the selected region
- `Escape` — cancel drawing or deselect
- Scroll — zoom toward the cursor
- `Space` + drag or middle-drag — pan

## Export

Annotations are exported as nested JSON. Regions inside another region appear in
its `children` array:

```json
{
  "image": "/path/to/image.jpg",
  "image_width": 1024,
  "image_height": 1080,
  "annotations": [
    {
      "id": 1,
      "label": "notenderdreams",
      "x": 358.0,
      "y": 491.0,
      "width": 438.0,
      "height": 456.0,
      "color": [255, 0, 0]
    },
    {
      "id": 2,
      "label": "Demo",
      "x": 73.0,
      "y": 138.0,
      "width": 947.0,
      "height": 239.0,
      "color": [41, 121, 255],
      "children": [
        {
          "id": 3,
          "label": "object_03",
          "x": 99.0,
          "y": 155.0,
          "width": 146.0,
          "height": 208.0,
          "color": [0, 230, 118]
        },
        {
          "id": 4,
          "label": "object_04",
          "x": 286.0,
          "y": 165.0,
          "width": 711.0,
          "height": 194.0,
          "color": [255, 0, 0],
          "children": [
            {
              "id": 5,
              "label": "object_05",
              "x": 318.0,
              "y": 193.0,
              "width": 91.0,
              "height": 145.0,
              "color": [255, 145, 0]
            },
            {
              "id": 6,
              "label": "object_06",
              "x": 443.0,
              "y": 195.0,
              "width": 406.0,
              "height": 135.0,
              "color": [64, 64, 64]
            }
          ]
        }
      ]
    },
    {
      "id": 7,
      "label": "Maybe Ghost",
      "x": 68.0,
      "y": 553.0,
      "width": 162.0,
      "height": 112.0,
      "color": [190, 190, 190]
    }
  ]
}
```
