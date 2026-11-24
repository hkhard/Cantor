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
| 3. Filter masks, bounding boxes | `noise_filtering`, `bounding_boxes`, `staff_detection.make_lines_stronger` | `boxes` | done: from the 5 Lacrymosa masks, every box homr's `predict_symbols` makes (159 noteheads, 428 staff fragments, 8 clefs, 496 stems/rests, 209 bar lines) within 0.0003 px and 0.01 degrees, in homr's order (`lib_06_homr_boxes_match_python`, references from `tools/omr/homr_port/capture_boxes.py`). The OpenCV geometry is ported in `boxes/contours.rs` (`findContours`, same contour and point order as cv2 4.14) and `boxes/geom.rs` (`minAreaRect`, `fitEllipse`, `intersectConvexConvex`, `pointPolygonTest`, `rotatedRectangleIntersection`, `fillPoly`, `warpAffine`, ...). Degenerate ellipse fits (points on a line) differ: OpenCV jitters them with its global RNG, so homr itself is not reproducible there; homr drops those boxes by size either way. Noise filter: `lib_06_homr_filter_keeps_a_clean_page`, `lib_06_homr_noise_grid_filters_a_noisy_half` (synthetic, no Python reference) |
| 4. Staff detection, notes on staffs, braces, system repair | `staff_detection`, `note_detection`, `brace_dot_detection`, `bar_line_detection`, `system_repair` | `staffs` | done: from the masks, 10 staffs, systems [5, 5], the same grand staffs and every grid point of homr's `staffs.json`, line ys within 0.0004 px, x exact (`lib_06_homr_staffs_match_python`). `add_notes_to_staffs` is not ported: it only attaches notes to staffs, which nothing downstream reads. Mac, release: about 0.15 s per page |
| 5. Dewarp and cut staff images | `staff_dewarping`, `staff_parsing` (`prepare_staff_image`) | `staff_images` | done: all 10 Lacrymosa strips (1280x256) equal homr's `staff_{i}.png` pixel for pixel, from homr's staffs (`lib_06_homr_staff_images_match_python`) and from our own stage 4 (`lib_06_homr_staff_images_from_our_staffs_match_python`); `cv::Subdiv2D` is ported in `staff_images/delaunay.rs`. Strips come in `musicxml::strip_order`. Not ported: the point mappings back to the page (homr's `imgpos` comments) |
| 6. Transformer (encoder + decoder) | `transformer/*` | `transformer` | done: all 10 Lacrymosa staffs give homr's exact symbols (`lib_06_homr_transformer_reads_lacrymosa_staffs_like_python`); vocabulary equals Python's (`lib_06_homr_vocabulary_equals_python_vocabulary`). Mac CPU, release: about 0.6 s per staff, 6.0 s for 10 staffs and 418 symbols, measured on a loaded machine |
| 7. MusicXML | `music_xml_generator`, with the token part of `staff_parsing.parse_staffs`, `vocabulary.py` (`remove_duplicated_symbols`) and `triplet_restoration` | `musicxml` | done: from the 10 Lacrymosa token files and the staff layout, the output equals homr's `page.musicxml` byte for byte apart from homr's `imgpos` comments, which need stages 5 and 6 (`lib_06_homr_musicxml_matches_python`); 6 synthetic cases (triplets marked and unmarked, repeats, voltas, dynamics, grace notes, ties, two voices, clef change in a chord, multi-measure rests, a hidden part) equal homr too (`lib_06_homr_musicxml_cases_match_python`, references from `tools/omr/homr_port/musicxml_cases.py`); `cantor_score::parse_score_file` reads all of them. `musicxml::strip_order` gives homr's reading order of the strips (voice by voice), which stage 5 must follow |

Models (129 MB) are not in git. `tools/omr/homr_port/fetch_models.sh` gets
them from homr's GitHub release and checks their SHA-256.

## Testing

```sh
bash tools/omr/homr_port/fetch_models.sh            # once, into models/homr/
cargo test -p cantor-homr --release                 # host ONNX Runtime (feature `download`, on by default)
```

The `download` feature is on by default so `cargo test --workspace` links
on the host. The app depends on this crate with `default-features = false`.
The model tests skip when `models/homr/` is missing.

The decoder's vocabularies are built in code, like homr's `Vocabulary()`.
The `tokenizer_*.json` files are an older tokenizer that the decoder does
not use.

Pitfall from the spike: `ort` 2.x silently uses CoreML in fp16 on Apple.
Pin `AutoDevicePolicy::PreferCPU` for the decoder, or the tokens change.
