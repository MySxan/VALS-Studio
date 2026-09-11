# Synthetic WAV fixtures

Generated in this repository for tests; no recorded voice or external dataset.
PCM signed 16-bit little endian, 48,000 Hz, RIFF/WAVE; sample i is `(i % 32) * 128 - 2048`.

| File | Channels | Frames | Duration | SHA-256 |
| --- | --- | --- | --- | --- |
| mono-48000.wav | 1 | 480 | 0.01 s | 0ff52963ef493d16ca18d47d1bd782fb7d06801f8f4d2e3847cbea3680bc86bd |
| stereo-48000.wav | 2 | 960 | 0.02 s | 3bf26cbf3c9f632a2935804ac55f9fd3484ae3e5fc10fadb503ec86d8a0a41b5 |

Dataset license: CC0-1.0 for these generated numeric fixtures; no model weights or third-party samples.
