"""Pitch-check a rendered part against its SoulX target JSON.

For each sung note at least 0.15 s long, take the median pYIN F0 over the
middle half of the note and compare with the written MIDI pitch.
Prints median absolute error (cents), share within 50 cents, octave errors,
and voicing (share of notes with any detected pitch).

Usage: python pitch_check.py target.json rendered.wav
"""
import json, sys
import numpy as np, librosa


def main(target, wav):
    y, sr = librosa.load(wav, sr=22050, mono=True)
    segs = json.load(open(target))
    errs, voiced, total = [], 0, 0
    for seg in segs:
        t = seg["time"][0] / 1000
        durs = [float(d) for d in seg["duration"].split()]
        pits = [int(p) for p in seg["note_pitch"].split()]
        lo = max(0, int((t - 0.1) * sr)); hi = int((t + sum(durs) + 0.1) * sr)
        chunk = y[lo:hi]
        if len(chunk) < 2048:
            t += sum(durs); continue
        f0, v, _ = librosa.pyin(chunk, fmin=65, fmax=1000, sr=sr, frame_length=2048)
        tt = librosa.times_like(f0, sr=sr) + lo / sr
        for d, p in zip(durs, pits):
            a, b = t + d * .25, t + d * .75
            t += d
            if p == 0 or d < .15:
                continue
            total += 1
            m = (tt >= a) & (tt < b) & v
            if not m.any():
                continue
            voiced += 1
            errs.append(100 * (librosa.hz_to_midi(np.nanmedian(f0[m])) - p))
    e = np.array(errs)
    ae = np.abs(e)
    print(f"notes checked {total}, voiced {voiced} ({voiced / max(total, 1):.0%})")
    print(f"median |error| {np.median(ae):.0f} cents; within 50 cents {np.mean(ae < 50):.0%}; "
          f"octave errors {np.mean(np.abs(np.abs(e) - 1200) < 100):.0%}; mean signed {np.mean(e[ae < 300]):+.0f} cents")


if __name__ == "__main__":
    main(*sys.argv[1:3])
