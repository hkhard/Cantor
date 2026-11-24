"""Capture homr's per-stage outputs on a page, as references for the Rust
port in crates/homr (each stage there is tested against these).

Run with homr's Python (0.7.1):
  python capture_stages.py page.png outdir

Writes into outdir:
  preprocessed.png         the autocropped, resized, CLAHE image fed to segnet
  pred_raw_classes.png     segnet's merged class map (0..5) before any filter,
                           ONNX Runtime CPU provider with the fp16 model (what
                           the Rust port runs); segnet_raw.json: agreement of
                           the CoreML provider's map (homr's default on a Mac)
  pred_<name>.png          the five segmentation masks (0/255), after filtering
  staffs.json              staffs: grid points (x, 5 line ys, angle), grandstaff flags
  staff_<i>.png            the dewarped staff image given to the transformer
  tokens_<i>.json          decoded symbols per staff (rhythm, pitch, lift, articulation)
  page.musicxml            homr's final MusicXML

  python capture_stages.py page.png outdir --raw-only   only the first two
"""

import json
import os
import sys

import cv2
import numpy as np

from homr import color_adjust
from homr.autocrop import autocrop_with_offset
from homr.debug import Debug
from homr.main import ProcessingConfig, detect_staffs_in_image, get_predictions
from homr.music_xml_generator import XmlGeneratorArguments, generate_xml
from homr.noise_filtering import filter_predictions
from homr.resize import resize_image
from homr.segmentation import inference_segnet
from homr.staff_detection import make_lines_stronger
from homr.system_repair import repair_systems
from homr.transformer.configs import Config
from homr import staff_parsing

page, out = sys.argv[1], sys.argv[2]
raw_only = "--raw-only" in sys.argv
os.makedirs(out, exist_ok=True)

# Stage 1: preprocessing and segmentation masks.
image = cv2.imread(page)
image, _ = autocrop_with_offset(image)
image = resize_image(image)
pre = color_adjust.apply_clahe(image)
cv2.imwrite(os.path.join(out, "preprocessed.png"), pre)


def merged_classes() -> np.ndarray:
    """inference_segnet.inference, but return the merged class map itself."""
    staff, symbols, stems_rests, notehead, clefs_keys = inference_segnet.inference(
        pre, use_gpu_inference=True, batch_size=8, step_size=320, win_size=320
    )
    classes = np.zeros(pre.shape, np.uint8)
    for value, mask in [(1, stems_rests), (2, notehead), (3, clefs_keys), (4, staff), (5, symbols)]:
        classes[mask > 0] = value
    return classes


# Raw class map on the CPU provider (fp16 model, as the Rust port).
import onnxruntime as ort  # noqa: E402

cpu = inference_segnet.Segnet.__new__(inference_segnet.Segnet)
cpu.model = ort.InferenceSession(inference_segnet.segnet_path_onnx_fp16, providers=["CPUExecutionProvider"])
cpu.fp16 = True
cpu.use_gpu = False
cpu.io_binding = cpu.model.io_binding()
cpu.device_id = 0
inference_segnet._segnet_inference = cpu
raw_cpu = merged_classes()
cv2.imwrite(os.path.join(out, "pred_raw_classes.png"), raw_cpu)
# homr's default on a Mac: CoreML (the masks below come from this one).
inference_segnet._segnet_inference = None
raw_coreml = merged_classes()
json.dump(
    {
        "provider": "CPUExecutionProvider, fp16 model, batch 8, step 320",
        "coreml_pixels_differing": int((raw_cpu != raw_coreml).sum()),
        "pixels": int(raw_cpu.size),
    },
    open(os.path.join(out, "segnet_raw.json"), "w"),
    indent=1,
)
if raw_only:
    sys.exit(0)
pred = get_predictions(image, pre, page, False, True)
pred = filter_predictions(pred, Debug(pred.original, page, False))
pred.staff = make_lines_stronger(pred.staff, (1, 2))
for name in ["notehead", "symbols", "staff", "clefs_keys", "stems_rest"]:
    cv2.imwrite(os.path.join(out, f"pred_{name}.png"), (getattr(pred, name) > 0).astype(np.uint8) * 255)

# Stage 2: staff detection (the full pipeline, as main.py does it).
config = ProcessingConfig(False, False, False, False, -1, False, True, True, False)
multi, img, debug, _title, _n, _map = detect_staffs_in_image(page, config)
multi = repair_systems(debug, multi)
staffs = []
for m in multi:
    for s in m.staffs:
        staffs.append(
            {
                "grandstaff": bool(s.is_grandstaff),
                "min_x": float(s.min_x), "max_x": float(s.max_x),
                "min_y": float(s.min_y), "max_y": float(s.max_y),
                "grid": [{"x": float(p.x), "y": [float(v) for v in p.y], "angle": float(p.angle)} for p in s.grid],
            }
        )
json.dump({"multi": [len(m.staffs) for m in multi], "staffs": staffs}, open(os.path.join(out, "staffs.json"), "w"), indent=1)

# Stage 3: per-staff images and decoded tokens. Hook the transformer call.
captured = []
orig_predict = staff_parsing.Staff2Score.predict if hasattr(staff_parsing, "Staff2Score") else None
from homr.transformer import staff2score

real = staff2score.Staff2Score.predict


def spy(self, image):  # type: ignore[no-untyped-def]
    i = len(captured)
    cv2.imwrite(os.path.join(out, f"staff_{i}.png"), image)
    res = real(self, image)
    captured.append([str(t) for t in res])
    json.dump(captured[-1], open(os.path.join(out, f"tokens_{i}.json"), "w"), indent=0)
    return res


staff2score.Staff2Score.predict = spy
cfg = Config()
cfg.use_gpu_inference = False
result = staff_parsing.parse_staffs(debug, multi, img, selected_staff=-1, config=cfg, page_to_input_image=_map)
xml = generate_xml(XmlGeneratorArguments(False, None, None), result, "")
import xml.etree.ElementTree as ET

ET.ElementTree(xml).write(os.path.join(out, "page.musicxml"), encoding="unicode", xml_declaration=True)
print(f"{len(staffs)} staffs, {len(captured)} staff images, written to {out}")
