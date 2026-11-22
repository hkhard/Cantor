"""Render a Cantor-prepared part with SoulX-Singer.

Input: target JSON from `sing_prepare` (SoulX score-mode segments with absolute
times) plus its `.settings.json`. Each segment is synthesised separately and
placed at its own start time, so the result lines up with Cantor's MIDI.

Usage (run inside the SoulX-Singer checkout, its venv active):
  PYTORCH_ENABLE_MPS_FALLBACK=1 PYTHONPATH=. python render.py \
      target.json out.wav [--prompt-json P.json --prompt-wav P.wav] [--device mps]

SoulX limits: about 30 s per segment works well; longer segments are split at
<SP> rests by Cantor upstream (phrases), so they are passed through as is.
"""
import argparse, json, os, sys, time

import numpy as np
import soundfile as sf
import torch

from soulxsinger.utils.file_utils import load_config
from cli.inference import build_model
from soulxsinger.utils.data_processor import DataProcessor

SR = 24000


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("target")
    ap.add_argument("out")
    ap.add_argument("--prompt-json", default="example/audio/en_prompt.json")
    ap.add_argument("--prompt-wav", default="example/audio/en_prompt.wav")
    ap.add_argument("--device", default="mps" if torch.backends.mps.is_available() else "cpu")
    ap.add_argument("--model", default="pretrained_models/SoulX-Singer/model.pt")
    a = ap.parse_args()

    settings_path = a.target[: -len(".json")] + ".settings.json"
    st = json.load(open(settings_path)) if os.path.exists(settings_path) else {}
    n_steps = st.get("n_steps", 16)
    cfg = st.get("cfg", 0.0)
    control = st.get("control", "score")
    auto_shift = st.get("auto_shift", False)
    pitch_shift = st.get("pitch_shift", 0)

    config = load_config("soulxsinger/config/soulxsinger.yaml")
    t0 = time.time()
    model = build_model(a.model, config, device=a.device)
    dp = DataProcessor(hop_size=config.audio.hop_size, sample_rate=config.audio.sample_rate,
                       phoneset_path="soulxsinger/utils/phoneme/phone_set.json", device=a.device)
    prompt = dp.process(json.load(open(a.prompt_json))[0], a.prompt_wav)
    print(f"model loaded in {time.time() - t0:.0f}s on {a.device}", flush=True)

    segs = json.load(open(a.target))
    total = int(segs[-1]["time"][1] / 1000 * SR) + SR
    mix = np.zeros(total, dtype=np.float32)
    sung = 0.0
    t1 = time.time()
    for i, seg in enumerate(segs):
        start = int(seg["time"][0] / 1000 * SR)
        want = sum(float(d) for d in seg["duration"].split())
        tg = dp.process(seg, None)
        with torch.no_grad():
            y = model.infer({"prompt": prompt, "target": tg}, auto_shift=auto_shift,
                            pitch_shift=pitch_shift, n_steps=n_steps, cfg=cfg, control=control)
        y = y.squeeze().float().cpu().numpy()
        n = min(len(y), total - start)
        mix[start:start + n] += y[:n]
        sung += want
        print(f"JCODE_PROGRESS {json.dumps({'current': i + 1, 'total': len(segs), 'unit': 'phrases'})}", flush=True)
        print(f"  [{i + 1}/{len(segs)}] {seg['time'][0] / 1000:6.1f}s  {want:5.1f}s sung, got {len(y) / SR:5.1f}s", flush=True)
    dt = time.time() - t1
    peak = np.abs(mix).max()
    if peak > 0.99:
        mix *= 0.99 / peak
    sf.write(a.out, mix, SR)
    print(f"wrote {a.out}: {sung:.1f}s sung in {dt:.0f}s (RTF {dt / max(sung, 1e-6):.2f})", flush=True)


if __name__ == "__main__":
    sys.exit(main())
