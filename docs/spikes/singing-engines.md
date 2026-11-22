# Singing engine choice: SoulX-Singer vs DiffSinger vs NNSVS

## Decision (revised 2026-11-22, after listening)

**SoulX-Singer is the default engine.** The product owner compared the clips by
ear: SoulX sounds like a singer; our DiffSinger render sounded synthetic with a
breath artefact. Sound quality is the product, so it outranks size and speed.

The speed objection below no longer holds. With 8 flow-matching steps and no
classifier-free guidance SoulX renders at **RTF 1.1 on the M2 GPU** (MPS),
about the same as DiffSinger on CPU, with 13 cents median pitch error:

| SoulX setting (M2, MPS, 6.9 s phrase) | Render time | RTF | Pitch error (median) | Clip in `scores/out/` |
| --- | --- | --- | --- | --- |
| 32 steps, cfg 3 (default) | 38 s (131 s in one run under memory pressure) | 5.5 | 17 cents | `_soulx_steps32_cfg3.wav` |
| 16 steps, cfg 3 | 21 s | 3.0 | 11 cents | `_soulx_steps16_cfg3.wav` |
| 8 steps, cfg 3 | 14 s | 2.0 | 13 cents | `_soulx_steps8_cfg3.wav` |
| 16 steps, no cfg | 17 s | 2.5 | 22 cents | `_soulx_steps16_cfg0.wav` |
| **8 steps, no cfg** | **8 s** | **1.1** | 13 cents | `_soulx_steps8_cfg0.wav` |

Next: pick the lowest setting that still sounds right by ear (listen to the
five clips), then answer where it runs:

1. **Mac / iPad with M-series**: PyTorch MPS works today; for the app, convert
   to Core ML (fp16, about 1.4 GB). Feasible on iPads with 8 GB or more.
2. **iPhone and Android**: 1.4 GB at fp16 is heavy; options are int8 (~0.7 GB,
   quality to verify by ear), a user-run server, or rendering on the Mac/iPad
   and syncing the stems.
3. **Swedish**: still unsolved; SoulX has no Swedish phonemes. Options:
   ARPAbet approximation (quick test), or fine-tune SoulX on Swedish choir
   recordings.

### Note on the "DiffSinger + ONNX Runtime Mobile" research (2026-11-22)

It describes the same pipeline we measured (OpenVPI ONNX acoustic model +
NSF-HiFiGAN-class vocoder). Two fair points for a re-test: our render used
flat per-note f0 and no variance model (no pitch curve, breathiness), which
makes any engine sound more synthetic, and voice quality varies a lot between
banks. Caveats: `DiffSingerMiniEngine` is **AGPL-3.0** (reference only, not
linkable into an App Store build), and the ONNX CoreML provider ran slower than
CPU here. Its other advice (offline G2P, phrase-by-phrase rendering with a
stem cache, voicebank licensing) applies equally to SoulX and is already in
the plan (VOX-07).

DiffSinger stays the fallback for devices that cannot hold SoulX, if a
re-test with its variance model and a better bank sounds acceptable.

---

## Original analysis (superseded where it conflicts with the decision above)

2026-11-22. Question from the product owner: can SoulX-Singer run on a modern
iPhone/iPad instead of a user's NVIDIA server, and is NNSVS or DiffSinger
better, maybe as a full replacement?

## Answer

**Replace SoulX-Singer with DiffSinger (OpenVPI) as the default engine.** It is
about 9x smaller, already ships as ONNX, runs on Apple CPU today, sang the
score to within 3 cents in our test, and its voice bank system is the only one
of the three where a **Swedish voice** is realistic (train one with
MakeDiffSinger from ~1-2 h of a Swedish singer). Keep SoulX-Singer as an
optional server engine for zero-shot "sing in my voice" in English. Drop NNSVS.

SoulX-Singer on iPhone/iPad: **not feasible as shipped** (numbers below).

## Measurements on this Mac (Apple M2, 8 GB, macOS 27)

Same machine class as an iPad Air/Pro M2; an iPhone 17 Pro (A19 Pro) is in the
same range for CPU and ANE, with less RAM headroom.

| | SoulX-Singer | DiffSinger (Hanami voice) |
| --- | --- | --- |
| Parameters | **704 M** (flow-matching transformer 22 x 1024, plus vocoder) | **82 M** acoustic + vocoder (titaniagan, 57 MB) + 14 MB duration + 75 MB pitch (optional) |
| Files on disk | 2.8 GB `model.pt` (fp32) | 525 MB total ONNX (fp32); acoustic 328 MB |
| Model load | 58 s CPU, 85 s MPS | 0.8 s CPU, 8 s CoreML |
| Render 7 s of singing | **38 s on MPS (RTF 5.5)**, 76 s on CPU (RTF 11), 32 steps + CFG (2 passes per step) | **10 s on CPU (RTF 1.5)** at 10 steps; 36 s at 20 steps |
| CoreML/ANE | Not tried: PyTorch only, LLaMA blocks + CFG; would need a full CoreML port | ONNX Runtime CoreML provider: slower (RTF 14), most ops fall back to CPU. A native CoreML conversion is the work item |
| Pitch accuracy vs score | median 13 cents (after its own key shift to the prompt's range), 85% of notes within 50 cents | **3 cents** on every note (f0 taken from the score, no pitch model) |
| Runs offline on device today | No | Yes on Mac CPU; iOS via ONNX Runtime (OpenUtau Mobile has an experimental iOS build) |

Render time matters less than it looks: Cantor renders phrase by phrase in
the background and caches results (VOX-07), so RTF 1.5 means a 3-minute
part is ready in about 4.5 minutes and the first phrase in seconds. RTF 5.5 to
11 with a 2.8 GB model loaded in RAM is not workable on a phone.

### Why SoulX-Singer does not fit on device

- 704 M parameters: about 1.4 GB even at fp16; an iPhone app gets roughly
  3 to 6 GB before jetsam, shared with the engraver, PDF and audio engine.
- 32 flow-matching steps x 2 (classifier-free guidance) = 64 transformer
  passes per phrase; that is the RTF 5.5 measured on a desktop-class M2 GPU.
- No export path: PyTorch-only code (custom LLaMA blocks, rotary embeddings,
  torchaudio mel); no ONNX/CoreML exporter exists. Porting is a research
  project (distillation to fewer steps, quantisation, CoreML conversion).
- Needs a transcribed reference clip (prompt with phonemes, notes and f0) for
  every voice, which means shipping its preprocessing stack too.

## DiffSinger vs NNSVS

| | DiffSinger (openvpi) | NNSVS |
| --- | --- | --- |
| Licence | Apache-2.0 (code); each voice bank has its own terms | MIT (code); voices vary |
| Activity | 3.2k stars, pushed 2026-11-19; MakeDiffSinger dataset tools pushed 2026-11-18 | 758 stars, last push 2023-10 (inactive) |
| Model | Diffusion / rectified flow acoustic model + NSF-HiFiGAN vocoder, 44.1 kHz; separate duration, pitch and variance models | Older parametric/neural (WORLD/uSFGAN), some diffusion code borrowed from DiffSinger |
| Deployment | **ONNX export built in** (`scripts/export.py`), used in production by OpenUtau (desktop) and DiffScope; CoreML/CPU providers | Python inference; ENUNU plugin for OpenUtau (Windows-centric) |
| Languages | Any: language = phoneme dictionary + training data. Community voices cover English, Japanese, Chinese, Korean, Spanish, French, German, Polish, Russian, Thai... | Recipes are almost all Japanese (kiritan, namine ritsu, ...) plus Opencpop |
| Swedish | No public voice found. Path: record ~1-2 h of a Swedish singer, label with MakeDiffSinger (forced alignment + SOME note extraction), train (GPU, days), export ONNX | Same work, older stack, weaker results |
| Voice choice | Fixed voice banks, multi-speaker embeddings, gender/breathiness/tension curves | Fixed voices |
| Zero-shot "my voice" | No (needs training) | No |
| Input Cantor must produce | phonemes + per-phoneme frames + f0 curve (from our timemap), speaker embedding | Sinsy-style full-context labels |

DiffSinger wins on every axis that matters for Cantor. NNSVS has no
advantage left: its quality is lower, it is inactive, and its language support
is Japanese-first.

## Voice banks and licences (risk)

Voice terms vary per bank and must be checked one by one (NFR-09):

- Hoshino Hanami (Lotte V), used for the test: commercial use allowed without
  permission, credit required, no redistribution of the original files. So
  users download it themselves; Cantor cannot bundle it.
- Others (TIGER, Miyo, KYANN) need a paid licence for commercial use; Opencpop
  forbids it.

To ship a bundled default voice (and Swedish), Cantor should **commission
its own voice bank**: record one English and one Swedish singer (or the
same bilingual singer) under a contract that allows redistribution in the
app. This is the cleanest licence position and the only way to get Swedish.

## Recommendation for the plan

1. Default engine: DiffSinger via ONNX Runtime in the Rust core (`ort` crate),
   CPU first. Users install a voice bank folder (OpenUtau format) for now.
2. Work item: CoreML conversion of the acoustic model and vocoder (the ONNX
   CoreML provider fell back to CPU and was slower), and fp16 weights to halve
   the 525 MB.
3. Commission voice banks: English + Swedish, budget for recording (~2 h each),
   labelling and training (spike S2 becomes "train a Swedish DiffSinger voice
   from 30 min, measure diction").
4. Keep `cantor-vocal::soulx` for an optional server engine ("sing in my own
   voice", English), not on the critical path.
5. Remove NNSVS from the PRD engine list.

Test clips (git-ignored): `scores/out/_diffsinger_gloria_test.wav` (DiffSinger,
"Gloria, amen" built by hand from the score), `scores/out/_soulx_example_test.wav`
(SoulX's own English example).
