# cantor-homr: on-device note recognition (port of homr)

A Rust port of **homr** by Christian Liebhardt and contributors
(<https://github.com/liebharc/homr>, version 0.7.1), so ChoirBird can turn a
PDF or photo of sheet music into MusicXML on the iPad or iPhone, with no
network.

## Licence

- homr is **AGPL-3.0**: code and model weights. This crate is a port, so it
  is a derivative work and is **AGPL-3.0-only**.
- The app store permission in `LICENSE-EXCEPTIONS.md` does **not** apply to
  this crate: it is not ours to give, and homr's authors grant none.
- Decision (product owner, 2026-11-23): accept AGPL for on-device note
  recognition.
- Decision (product owner, 2026-11-24): ship builds with this crate through
  the App Store, free of charge, once the repository is public and the exact
  source of each build is linked from About (AGPL 6 and 13). This follows
  homr's `LICENSE_FAQ.md`: the AGPL allows commercial use as long as its
  terms are met, including publishing the source. Money comes from the
  backend services, not from the app.
- Known residual risk, accepted: the FAQ does not mention app stores.
  Apple's terms of use add restrictions that AGPL section 10 forbids, and
  only homr's copyright holders could waive that. Any one of them could ask
  Apple to remove the app (as happened to VLC in 2011). If that happens,
  ship the app without this crate (the `homr` feature) and convert on the
  server instead. Not legal advice; phase L can review it.
- Model weights: AGPL as published. The segmentation model is trained on
  DeepScoresV2 (CC BY 4.0, which needs attribution: About and
  `THIRD_PARTY_NOTICES.md` credit it). The transformer is trained on
  OpenScore Lieder and StringQuartets (CC0), GrandStaff (MIT on Hugging Face,
  not stated at its source) and the public-domain PDMX subset (CC BY 4.0
  dataset).
- The tokenizer files in `src/tokenizer_*.json` are copied unchanged from
  homr 0.7.1.

## Pipeline and port status

Each stage is checked against homr's Python output on a public-domain page
(`fixtures/omr/lacrymosa-p1/`, captured by
`tools/omr/homr_port/capture_stages.py`).

| Stage | homr module | Rust module | Status |
|---|---|---|---|
| 1. Preprocess: autocrop, resize, CLAHE | `autocrop`, `resize`, `color_adjust` | `preprocess` | done: the Lacrymosa page gives homr's `preprocessed.png` pixel for pixel, 0 of 5205120 px (`lib_06_homr_preprocess_matches_python`); 3 synthetic photos (rotated sheet on a desk, sheet at the left edge, dark desk) crop to homr's offsets and every colour value (`lib_06_homr_autocrop_matches_python_on_photos`). The OpenCV and Pillow ops are in `image_ops.rs` (threshold, morphology, resize nearest/linear, Pillow bicubic, box blur, CLAHE, BGR to grey), each bit-exact against opencv-python 4.14 / Pillow 12 on arm64 (`lib_06_cv_*`, references from `tools/omr/homr_port/cv2_refs.py`). OpenCV's arm64 wheel resizes through the KleidiCV and carotene HALs, which round differently from OpenCV's own code; `image_ops::resize` follows the same dispatch. Mac, release: about 0.25 s per page. Not verified: OpenCV built for x86 (other rounding), colour resize through the HALs |
| 2. Segmentation (segnet, 5 masks) | `segmentation/inference_segnet` | `segnet` | done: on the Lacrymosa page, the merged class map before `filter_predictions` equals homr's on the ONNX Runtime CPU provider, 0 of 5205120 px, every mask exact (`lib_06_homr_segnet_masks_match_python`, reference `pred_raw_classes.png`). homr's quirks are kept: the last tile row and column are pulled back inside the page, and overlapping tiles average the class numbers. homr's Mac default (CoreML) differs from the CPU map on 98 px (`segnet_raw.json`). Mac CPU, release: 4.2 s for 54 tiles, model load 0.2 s. Not verified: CoreML in the Rust port, iPad timing |
| 3. Filter masks, bounding boxes | `noise_filtering`, `bounding_boxes`, `staff_detection.make_lines_stronger` | `boxes` | done: from the 5 Lacrymosa masks, every box homr's `predict_symbols` makes (159 noteheads, 428 staff fragments, 8 clefs, 496 stems/rests, 209 bar lines) within 0.0003 px and 0.01 degrees, in homr's order (`lib_06_homr_boxes_match_python`, references from `tools/omr/homr_port/capture_boxes.py`). The OpenCV geometry is ported in `boxes/contours.rs` (`findContours`, same contour and point order as cv2 4.14) and `boxes/geom.rs` (`minAreaRect`, `fitEllipse`, `intersectConvexConvex`, `pointPolygonTest`, `rotatedRectangleIntersection`, `fillPoly`, `warpAffine`, ...). Degenerate ellipse fits (points on a line) differ: OpenCV jitters them with its global RNG, so homr itself is not reproducible there; homr drops those boxes by size either way. Noise filter: `lib_06_homr_filter_keeps_a_clean_page`, `lib_06_homr_noise_grid_filters_a_noisy_half` (synthetic, no Python reference) |
| 4. Staff detection, notes on staffs, braces, system repair | `staff_detection`, `note_detection`, `brace_dot_detection`, `bar_line_detection`, `system_repair` | `staffs` | done: from the masks, 10 staffs, systems [5, 5], the same grand staffs and every grid point of homr's `staffs.json`, line ys within 0.0004 px, x exact (`lib_06_homr_staffs_match_python`). `add_notes_to_staffs` is not ported: it only attaches notes to staffs, which nothing downstream reads. Mac, release: about 0.15 s per page |
| 5. Dewarp and cut staff images | `staff_dewarping`, `staff_parsing` (`prepare_staff_image`) | `staff_images` | done: all 10 Lacrymosa strips (1280x256) equal homr's `staff_{i}.png` pixel for pixel, from homr's staffs (`lib_06_homr_staff_images_match_python`) and from our own stage 4 (`lib_06_homr_staff_images_from_our_staffs_match_python`); `cv::Subdiv2D` is ported in `staff_images/delaunay.rs`. Strips come in `musicxml::strip_order`. Not ported: the point mappings back to the page (homr's `imgpos` comments) |
| 6. Transformer (encoder + decoder) | `transformer/*` | `transformer` | done: all 10 Lacrymosa staffs give homr's exact symbols (`lib_06_homr_transformer_reads_lacrymosa_staffs_like_python`); vocabulary equals Python's (`lib_06_homr_vocabulary_equals_python_vocabulary`). Mac CPU, release: about 0.6 s per staff, 6.0 s for 10 staffs and 418 symbols, measured on a loaded machine |
| 7. MusicXML | `music_xml_generator`, with the token part of `staff_parsing.parse_staffs`, `vocabulary.py` (`remove_duplicated_symbols`) and `triplet_restoration` | `musicxml` | done: from the 10 Lacrymosa token files and the staff layout, the output equals homr's `page.musicxml` byte for byte apart from homr's `imgpos` comments, which need stages 5 and 6 (`lib_06_homr_musicxml_matches_python`); 6 synthetic cases (triplets marked and unmarked, repeats, voltas, dynamics, grace notes, ties, two voices, clef change in a chord, multi-measure rests, a hidden part) equal homr too (`lib_06_homr_musicxml_cases_match_python`, references from `tools/omr/homr_port/musicxml_cases.py`); `cantor_score::parse_score_file` reads all of them. `musicxml::strip_order` gives homr's reading order of the strips (voice by voice), which stage 5 must follow |

Models (129 MB) are not in git. `tools/omr/homr_port/fetch_models.sh` gets
them from homr's GitHub release and checks their SHA-256.

## Where the port differs from homr 0.7.1 (on purpose)

Every stage matches homr exactly on the Lacrymosa page. Two changes go
beyond homr, because homr's output was wrong for choir scores whose layout
changes on a page (found on *Abide with me*: a soprano solo for three
systems, then S, T and B joined by a bracket, then T and B on a grand staff):

- **System repair** (`staffs/systems.rs`): a system of single staffs joined
  by a found brace or bracket stays whole, even when the page's common layout
  is one single staff. homr split it into single staffs. Systems with grand
  staffs are repaired as before.
- **Parts** (`musicxml/voices.rs`): when systems differ and can't be matched
  by layout, homr read every staff as one line of a single part. The port now
  gives the largest system's staffs one part each and puts every staff (each
  half of a grand staff) in the part with the same clef, in order; parts a
  system doesn't show get measure rests.

Tests: `lib_06_bracketed_system_after_solo_lines_is_kept`,
`lib_06_system_without_connection_is_still_repaired`,
`lib_10_solo_then_three_staffs_then_two_gives_three_parts`.

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
