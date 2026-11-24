"""Compare ChoirBird's playback order with a MuseScore MIDI export, bar by bar.

  python3 compare_midi_order.py ref.mid written.txt "1-6 3-5 7-11 ..."

written.txt: one line per written bar, the struck MIDI pitches
(cantor-timeline example bar_pitches). Prints how many MIDI bars differ
from the written bar the order says, as pitch multisets.
"""
import struct
import sys
from collections import Counter


def midi_bars(path):
    data = open(path, 'rb').read()

    def vlq(d, i):
        v = 0
        while True:
            b = d[i]
            i += 1
            v = (v << 7) | (b & 0x7F)
            if b < 0x80:
                return v, i

    _, ntr, div = struct.unpack('>HHH', data[8:14])
    i, notes = 14, []
    for _ in range(ntr):
        ln = struct.unpack('>I', data[i + 4:i + 8])[0]
        j, end, tick, run = i + 8, i + 8 + ln, 0, None
        while j < end:
            dt, j = vlq(data, j)
            tick += dt
            st = data[j]
            if st == 0xFF:
                l, k = vlq(data, j + 2)
                j = k + l
                continue
            if st in (0xF0, 0xF7):
                l, k = vlq(data, j + 1)
                j = k + l
                continue
            if st & 0x80:
                run = st
                j += 1
            ev = run >> 4
            a = data[j]
            b = data[j + 1] if ev not in (0xC, 0xD) else None
            j += 1 if ev in (0xC, 0xD) else 2
            if ev == 9 and b:
                notes.append((tick, a))
        i = end
    bar = div * 4
    n = max(t for t, _ in notes) // bar + 1
    return [Counter(p for t, p in notes if t // bar == k) for k in range(n)]


def expand(order):
    out = []
    for r in order.split():
        a, _, b = r.partition('-')
        out += range(int(a) - 1, int(b or a))
    return out


played = midi_bars(sys.argv[1])
written = [Counter(map(int, l.split())) for l in open(sys.argv[2]).read().splitlines()]
order = expand(sys.argv[3])
bad = [k + 1 for k in range(max(len(order), len(played)))
       if k >= len(order) or k >= len(played) or played[k] != written[order[k]]]
print(f'{len(order)} bars vs MIDI {len(played)}; bars that differ: {len(bad)} {bad[:20]}')
