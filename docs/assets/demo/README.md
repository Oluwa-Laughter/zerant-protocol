# Zerant browser-video preview

This **36-second silent overview** uses real Zerant browser captures of the landing page, private Vault, holder consent, verified bounded claim, organization-scoped payout routing, prepared Zcash payment, and responsive menu. Camera and cursor transitions were rendered from the MengTo browser-video-recording workflow. The screen captures show actual product states, but the composed camera/cursor transitions are illustrative, not a continuous live transaction recording.

- Preview: [`zerant-demo-preview.mp4`](zerant-demo-preview.mp4), 1280 × 960, 30 fps, H.264.
- Reproducible storyboard: [`demo-config.json`](demo-config.json) and [`frames/`](frames/).
- High-resolution local master: `~/Videos/Zerant/zerant-demo-highres.mp4`, 3072 × 2304, 30 fps, H.264, 36 seconds. The master is not committed to Git because of its file size.

This is **not** proof that a testnet payment settled, and it does not reproduce a native OS passkey prompt. See [`docs/DEMO.md`](../../DEMO.md) for the full three-minute interactive demo script and final capture checklist.

To rebuild, run the browser-video-recording skill renderer from the repository root with `--config docs/assets/demo/demo-config.json --output docs/assets/demo/zerant-demo-preview.mp4`. The renderer is supplied by the skill and is not vendored into this project.
