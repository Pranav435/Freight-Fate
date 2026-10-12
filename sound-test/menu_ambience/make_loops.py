"""Cut the rendered menu ambience beds into seamless loops.

Render first (`genny render sound-test/menu_ambience/batch.json`), then run
this from the repository root. Each 20 s render drops its first 2 s (the
reverb and engines settling) and keeps 17 s; the 1 s after the kept span is
crossfaded over its head, so the end flows straight back into the start.
Writes `<name>.ogg` beside the WAVs, ready to pack as `ambient/<name>.ogg`.
"""

import subprocess
import sys
import wave
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
SKIP_S, KEEP_S, FADE_S = 2.0, 17.0, 1.0


def make_loop(src: Path) -> Path:
    with wave.open(str(src)) as w:
        rate, channels, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
        frames = w.readframes(w.getnframes())
    if width != 2:
        sys.exit(f"{src.name}: expected 16-bit audio, got {8 * width}-bit")
    x = np.frombuffer(frames, dtype=np.int16).reshape(-1, channels).astype(np.float64)
    a, n, c = int(SKIP_S * rate), int(KEEP_S * rate), int(FADE_S * rate)
    loop = x[a : a + n].copy()
    # Equal-power crossfade: the head fades in while the audio that would
    # follow the end fades out, so sample n-1 is followed by its true successor.
    t = np.linspace(0.0, np.pi / 2, c)[:, None]
    loop[:c] = x[a : a + c] * np.sin(t) + x[a + n : a + n + c] * np.cos(t)
    out_wav = src.with_name(src.stem + "_loop.wav")
    with wave.open(str(out_wav), "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(np.clip(loop, -32768, 32767).astype(np.int16).tobytes())
    out_ogg = src.with_suffix(".ogg")
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-loglevel",
            "error",
            "-i",
            str(out_wav),
            "-c:a",
            "libvorbis",
            "-q:a",
            "4",
            str(out_ogg),
        ],
        check=True,
    )
    out_wav.unlink()
    return out_ogg


if __name__ == "__main__":
    for src in sorted(HERE.glob("menu_*.wav")):
        if not src.stem.endswith("_loop"):
            print("wrote", make_loop(src))
