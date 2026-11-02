"""Make the Tag toy's spray sound (`toy_tag`, SJK's own).

`spray.wav`: a spray can putting a tag up: the can shaken (the mixing ball rattling
three times), then two hisses of the nozzle, a short one and a longer one, with the
valve's click at each start. Nothing taken from another game: filtered noise and a few
clicks, 22050 Hz mono 16-bit, deterministic (fixed seed).

    python scripts/tag_sounds.py crates/sjk-viewer/assets/tags

Needs numpy (pip install numpy).
"""

import math
import sys
import wave
from pathlib import Path

import numpy as np

RATE = 22050


def write_wav(path: Path, samples: np.ndarray) -> None:
    peak = np.max(np.abs(samples))
    if peak > 0:
        samples = samples / peak * 0.9
    data = (np.clip(samples, -1, 1) * 32767).astype("<i2").tobytes()
    with wave.open(str(path), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(data)


def lowpass(signal: np.ndarray, cutoff: float) -> np.ndarray:
    """A one-pole low pass at `cutoff` Hz."""
    a = 1.0 - math.exp(-2 * math.pi * cutoff / RATE)
    out = np.empty_like(signal)
    y = 0.0
    for i, x in enumerate(signal):
        y += a * (x - y)
        out[i] = y
    return out


def highpass(signal: np.ndarray, cutoff: float) -> np.ndarray:
    return signal - lowpass(signal, cutoff)


def rattle(rng, length: float) -> np.ndarray:
    """The mixing ball knocking about the can: a burst of bright metal ticks."""
    n = int(RATE * length)
    out = np.zeros(n)
    t = np.arange(int(RATE * 0.03)) / RATE
    for _ in range(9):
        at = int(rng.random() * (n - len(t)))
        pitch = 2600 + rng.random() * 1400
        tick = (np.sin(2 * np.pi * pitch * t) + 0.5 * np.sin(2 * np.pi * pitch * 1.7 * t))
        out[at:at + len(t)] += tick * np.exp(-t * 180) * (0.5 + 0.5 * rng.random())
    return out


def hiss(rng, length: float) -> np.ndarray:
    """The nozzle: bright noise swelling in, steady, then cut, with the valve's click."""
    n = int(RATE * length)
    t = np.arange(n) / RATE
    noise = highpass(lowpass(rng.standard_normal(n), 9000), 1800)
    # A little flutter of the jet.
    noise *= 1 + 0.08 * np.sin(2 * np.pi * 23 * t) + 0.05 * lowpass(rng.standard_normal(n), 40) * 8
    env = np.clip(t / 0.025, 0, 1) * np.clip((length - t) / 0.04, 0, 1)
    click = highpass(rng.standard_normal(n), 3000) * np.exp(-t * 400) * 1.2
    return noise * env * 0.7 + click


def sounds(output: Path) -> None:
    rng = np.random.default_rng(66)
    parts = [
        (rattle(rng, 0.11), 0.0),
        (rattle(rng, 0.11), 0.16),
        (rattle(rng, 0.11), 0.32),
        (hiss(rng, 0.22), 0.55),
        (hiss(rng, 0.6), 0.86),
    ]
    total = max(start + len(part) / RATE for part, start in parts) + 0.02
    out = np.zeros(int(RATE * total))
    for part, start in parts:
        at = int(RATE * start)
        out[at:at + len(part)] += part
    write_wav(output / "spray.wav", out)


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    output = Path(sys.argv[1])
    output.mkdir(parents=True, exist_ok=True)
    sounds(output)


if __name__ == "__main__":
    main()
