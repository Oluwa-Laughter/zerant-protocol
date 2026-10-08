# Zerant product demo

## Final walkthrough

The final local export is `~/Videos/Zerant/zerant-demo-final.mp4`: **3:20,
1920 × 1080, 30 fps, H.264 Main/yuv420p and AAC**. The accompanying
`zerant-demo-preview.mp4` is a shorter preview. Large regenerated videos remain
outside Git; the older committed `zerant-demo-full.mp4` is a historical export,
not the caption-free final version.

This version captures actual Chromium compositor frames and real production
interactions. It shows the real credential, an explicitly reviewed consent
request, the bounded verifier result, an existing private payout record,
reviewed payment preparation, issuer tools, and mobile navigation. Loading and
navigation waits (up to two minutes for readiness) are excluded between sections. It is edited, not one continuous
screen recording, and its English narration is synthetic.

The pointer is a **12 × 18 pixel** overlay following the actual browser inputs.
It has no enlargement, click animation, rotation, or trail. There are no captions
or subtitle streams. The script targets 125 words per minute with reading pauses.
Native passkey approval is performed by the account owner before capture; no OS
passkey dialog is fabricated. No wallet transaction or settlement is simulated.

## Investigation and repair

The reported v4 all-black output was **not reproducible** in the supplied file:
FFmpeg decoded the entire export, blackdetect found no black interval, inspected
frames showed Zerant, and Chromium played it visibly. A player-specific failure
cannot be established without that player's error. Do not describe a guessed
codec failure as the root cause.

A separate, reproducible rendering defect was caught in the new ten-second
preflight: Chromium screenshots and screencast frames had different dimensions,
and the desktop/mobile switch reinitialized FFmpeg's fps filter. The mobile
section disappeared even though duration and codec checks passed. The repaired
encoder normalizes every source frame to a fixed **1920 × 1080 RGB canvas before
concat**, so the filter graph cannot discard a section on resolution change.
Mobile capture is centered with a neutral background, preserving its proportions. A second visual check caught compositor clipping in a tall mobile screencast. Mobile now uses a realistic 390 × 844 viewport and full-viewport screenshot sampling during the actual interactions; desktop uses compositor capture.

The new renderer uses explicit audio/video mapping, libx264, yuv420p, a fixed
30 fps output, and faststart. It writes a `.partial.mp4`, verifies metadata and
fully decodes it, then atomically publishes the destination. An unfinished
encode never replaces the final file. Source captures and old exports are kept.

## Reproduce

Requirements: Node 22+, Python 3, Pillow, Chromium, FFprobe, FFmpeg with
**libx264**, and the `edge-tts` CLI for synthetic narration. These are recording
tools, not production web dependencies. A system FFmpeg without libx264 can use
the executable supplied by the maintained `imageio-ffmpeg` Python package via
`DEMO_FFMPEG`. Do not substitute an encoder silently.

1. Launch a dedicated Chromium profile with a local debugging endpoint, for
   example `--remote-debugging-port=9225`. Sign in normally with the real demo
   passkey. Keep the profile, cookies, and credential material outside Git.
2. Confirm the demo issuer, credential, verifier, and authorized active payout
   record exist. Never manufacture backend success or seed a screenshot.
3. In the verifier UI, create a short-lived Contributor Eligibility request for
   the holder immediately before recording. The capture configuration explicitly
   clicks **Approve this claim** after review: run it only for the demonstration
   request the holder authorizes. Other accounts or purposes require a reviewed
   configuration. The result must genuinely verify.
4. Put the approved demo **testnet receive address** in a local file. Set
   `DEMO_RECIPIENT_FILE` to its path; the address is not committed in the config.
   `Review payment` creates a real prepared record; no spending is invoked.
5. Select the exact tab using `DEMO_TARGET` (its local CDP target ID). This avoids
   recording a restored blank tab or another browser page.

From the repository root:

```bash
python3 scripts/prepare-demo-narration.py \
  docs/assets/demo/demo-final-narration.json /tmp/zerant-demo-voice/final

DEMO_CDP=http://127.0.0.1:9225 DEMO_TARGET=YOUR_TAB_ID \
DEMO_RECIPIENT_FILE=/path/to/approved-testnet-address.txt \
node scripts/record-zerant-demo.mjs \
  docs/assets/demo/demo-final-config.json ~/Videos/Zerant/final-capture

python3 scripts/render-zerant-demo.py \
  ~/Videos/Zerant/final-capture /tmp/zerant-demo-voice/final/narration.wav \
  ~/Videos/Zerant/zerant-demo-final.mp4

python3 scripts/verify-zerant-demo.py \
  ~/Videos/Zerant/zerant-demo-final.mp4 ~/Videos/Zerant/verification
```

**Before a full capture**, use a ten-second configuration containing the landing
page, a mobile viewport, and a return to desktop. Render it, inspect actual frames
from every section, check audible narration, and play it in a browser. Metadata
alone will not catch the dimension-switch bug. Do not reuse an output directory
from an interrupted capture; keep it as evidence and use a new directory.

Completed sections are checkpointed in `capture.json` and `frames.ffconcat`. If a later navigation fails, record only the remaining scenes in a new directory, then join the genuine sections with `python3 scripts/merge-demo-captures.py FIRST_TAKE NEXT_TAKE --output NEW_DIRECTORY`. Keep their narration durations and story order unchanged. This joins recorded frames; it does not invent transitions or successful states.

Run `DEMO_FFMPEG=/path/to/ffmpeg python3 scripts/test-demo-render.py` to verify the regression: three colored technical fixtures at changing dimensions must all survive at their expected timestamps. These fixtures are never used in the product demo.

The final verifier extracts frames at 00:05, 00:30, 01:00, 01:30, 02:00, 02:30,
03:00 and 03:15, decodes the whole file, and scans black/freeze intervals. Freeze
reports include intentional reading pauses; inspect them against capture actions.
Check screenshots for failed/loading states, absent sections, secret exposure,
and accidental captions. Do not call a file verified until visual checks pass.

## Demonstration boundaries

- Existing credentials and organization records are real server-backed records.
- Passkey access is independent of wallet connection.
- The private payout address is temporary and relationship scoped.
- Payment review is not submission or settlement; external spending approval is
  not shown in this demo.
- The live Activity page showed invalid date values during preparation and is
  omitted. Working issuer tools occupy that segment instead; screenshots are
  never edited to conceal the defect.

The older screenshot-based storyboards, captions, and source assets remain for
historical reference. They are not inputs to this recording pipeline. See
[the interactive demo guide](../../DEMO.md) for the product flow.
