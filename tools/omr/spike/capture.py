# Run homr on one page, record the model inputs and outputs (shapes, first
# patches, every staff image) and the time spent in ML vs non-ML code.
import sys, time, json, numpy as np
from homr.segmentation import inference_segnet as seg
from homr.transformer import staff2score as s2s, decoder_inference as dec
out = sys.argv[2]
rec = {"segnet_batches": [], "staffs": [], "t_seg": 0.0, "t_enc": 0.0, "t_dec": 0.0, "dec_steps": []}
orig_run = seg.Segnet.run
def run(self, x):
    t = time.perf_counter(); y = orig_run(self, x); rec["t_seg"] += time.perf_counter() - t
    if len(rec["segnet_batches"]) == 0:
        np.save(f"{out}/seg_in.npy", x.astype(np.float16)); np.save(f"{out}/seg_out.npy", y)
    rec["segnet_batches"].append(list(x.shape)); return y
seg.Segnet.run = run
from homr.transformer import encoder_inference as enc
orig_gen = enc.Encoder.generate
def egen(self, x):
    t = time.perf_counter(); y = orig_gen(self, x); rec["t_enc"] += time.perf_counter() - t
    i = len(rec["staffs"]); np.save(f"{out}/enc_in_{i}.npy", x.astype(np.float32))
    if i == 0: np.save(f"{out}/enc_out_0.npy", y)
    rec["staffs"].append(list(x.shape)); return y
enc.Encoder.generate = egen
orig_dg = dec.ScoreDecoder.generate
def dgen(self, *a, **k):
    t = time.perf_counter(); y = orig_dg(self, *a, **k); rec["t_dec"] += time.perf_counter() - t
    rec["dec_steps"].append(len(y))
    if len(rec["dec_steps"]) == 1:
        json.dump([[s.rhythm, s.pitch] for s in y], open(f"{out}/dec_out_0.json", "w"))
    return y
dec.ScoreDecoder.generate = dgen
from homr import main as hm
sys.argv = ["homr", sys.argv[1]]
t0 = time.perf_counter()
try:
    hm.main()
except SystemExit:
    pass
rec["t_total"] = time.perf_counter() - t0
rec["n_seg_batches"] = len(rec.pop("segnet_batches"))
json.dump(rec, open(f"{out}/rec.json", "w"), indent=1)
print(json.dumps({k: v for k, v in rec.items() if k != "staffs"}, indent=1), len(rec["staffs"]), "staffs")
