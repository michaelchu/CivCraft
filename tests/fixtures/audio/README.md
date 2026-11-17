Synthetic 440 Hz, mono, 8 kHz tones generated with FFmpeg for decoder tests:

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=8000:duration=0.06' -ac 1 -c:a libvorbis tone.ogg
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=8000:duration=0.06' -ac 1 -c:a libmp3lame -write_xing 0 tone.mp3
```

OGA uses the same Ogg Vorbis bytes with the alternate extension. Tests validate
decoding rather than exact lossy sample values. No Civ III assets are included.
