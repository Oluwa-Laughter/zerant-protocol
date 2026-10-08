# Zerant Demo Videos

## Full narrated walkthrough

**[Watch the full 2:57 Zerant demo](zerant-demo-full.mp4)** — 1280 × 960, 30 fps, H.264 video with AAC narration and burned-in captions.

It walks through real Zerant production browser captures: the landing page, passkey-authenticated credential vault, holder-controlled consent, bounded verifier result, temporary private payout destination, organization payout inbox, reviewed ZIP-321 handoff, and responsive navigation.

- The spoken narration is **synthetically generated**, not a recording of the developer's voice.
- Camera/cursor movement and scene transitions are **editorial illustrations**, not one uninterrupted browser recording.
- It does **not** show or claim a settled Zcash payment or a captured native OS passkey prompt.
- The human-readable [narration script](narration.json), [subtitle file](zerant-demo-full.srt), [motion storyboard](demo-full-config.json), and source [browser frames](frames/) are included for review and adaptation.

## Short silent preview

The **[36-second visual preview](zerant-demo-preview.mp4)** is a compact browser-state montage (1280 × 960, 30 fps, H.264), based on the [short storyboard](demo-config.json). A 36-second high-resolution local archival render is also at `~/Videos/Zerant/zerant-demo-highres.mp4` on the authorized development machine, but is not committed because of its size.

## Rebuilding

Both visual storyboards use the original [MengTo browser-video-recording skill](https://github.com/MengTo/Skills/tree/main/agent-skills/codex/browser-video-recording) and its `scripts/render_browser_demo.py` renderer. The renderer is not vendored into Zerant. From the repository root:

```bash
python3 /path/to/browser-video-recording/scripts/render_browser_demo.py \
  --config docs/assets/demo/demo-full-config.json \
  --output /tmp/zerant-demo-silent.mp4
```

To recreate the narrated version, synthesize or record the narration in `narration.json`, synchronize it using the included SRT cues, and mux the audio and subtitles with FFmpeg. The committed MP4 is the verified reference export.

For an interactive, real-time presentation rather than this edited walkthrough, use [docs/DEMO.md](../../DEMO.md). Never infer payment settlement from a prepared request or wallet-submitted transaction ID.
