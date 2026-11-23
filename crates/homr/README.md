# cantor-homr: on-device note recognition (port of homr)

A Rust port of **homr** by Christian Liebhardt and contributors
(<https://github.com/liebharc/homr>, version 0.7.1), so ChoirBird can turn a
PDF or photo of sheet music into MusicXML on the iPad or iPhone, with no
network.

## Licence

- homr is **AGPL-3.0**: code and model weights. This crate is a port, so it
  is a derivative work and is **AGPL-3.0-only**.
- The app store permission in `LICENSE-EXCEPTIONS.md` does **not** apply to
  this crate: homr's authors did not grant it.
- Decision (product owner, 2026-11-23): accept AGPL for on-device note
  recognition. Before a **public** App Store release of a build that
  contains this crate, the app store question for homr's code must be
  settled (homr's authors, or legal advice). See `docs/ROADMAP.md`.
- The tokenizer files in `src/tokenizer_*.json` are copied unchanged from
  homr 0.7.1.

## Pipeline and port status

Each stage is checked against homr's Python output on a public-domain page
(`fixtures/omr/lacrymosa-p1/`, captured by
`tools/omr/homr_port/capture_stages.py`).

| Stage | homr module | Rust module | Status |
|---|---|---|---|
| 1. Preprocess: autocrop, resize, CLAHE | `autocrop`, `resize`, `color_adjust` | `preprocess` | todo |
| 2. Segmentation (segnet, 5 masks) | `segmentation/inference_segnet` | `segnet` | todo |
| 3. Filter masks, bounding boxes | `noise_filtering`, `bounding_boxes` | `boxes` | todo |
| 4. Staff detection, notes on staffs, braces, system repair | `staff_detection`, `note_detection`, `brace_dot_detection`, `bar_line_detection`, `system_repair` | `staffs` | todo |
| 5. Dewarp and cut staff images | `staff_dewarping`, `staff_parsing` | `staff_images` | todo |
| 6. Transformer (encoder + decoder) | `transformer/*` | `transformer` | todo |
| 7. MusicXML | `music_xml_generator` | `musicxml` | todo |

Models (129 MB) are not in git. `tools/omr/homr_port/fetch_models.sh` gets
them from homr's GitHub release and checks their SHA-256.

## Testing

```sh
bash tools/omr/homr_port/fetch_models.sh            # once, into models/homr/
cargo test -p cantor-homr --features download       # host ONNX Runtime
```

Pitfall from the spike: `ort` 2.x silently uses CoreML in fp16 on Apple.
Pin `AutoDevicePolicy::PreferCPU` for the decoder, or the tokens change.
