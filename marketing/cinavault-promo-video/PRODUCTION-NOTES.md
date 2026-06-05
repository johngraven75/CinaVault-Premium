# CinaVault Promo Video v2

Date: 2026-06-04
Source branch: `codex/cinavault-promo-video-v2`
Project folder in repo: `marketing\cinavault-promo-video`
Composition: `CinaVaultPromo`
Runtime: 68 seconds at 30 fps, 1920x1080
Primary export: `output\cinavault-promo-v2.mp4`

## Final Narration Script

Your media library ought to feel calm, fast, and under control.

CinaVault Premium takes local drives, NAS shares, cloud folders, live TV, and remote access, and brings it all into one polished command center.

It indexes big collections, keeps posters out front, recovers metadata, applies embedded titles, checks providers, and shows real diagnostics while work is running.

No guessing. No waiting on a frozen library screen. Just clear source health, safer duplicate cleanup, responsive paging, stable poster caches, and live task progress.

The dashboard tells you what the server is doing, where the sources are, and how your library is improving in real time.

Don't settle for yesterday's media server. Step into CinaVault, and give your library the vault it deserves.

## Scene Timing

| Time | Scene | Visual Treatment | Knowledge Points |
| --- | --- | --- | --- |
| 0:00-0:07 | Opening wow | Holographic core, particle field, animated counter, scanlines, light sweeps | Big libraries should stay fast, calm, and under control |
| 0:07-0:16 | Command center | Orbiting source nodes, pulsing hub, glitzy source telemetry | Local, NAS, cloud, live TV, remote access, security, plugins |
| 0:16-0:28 | Library intelligence | Locked diagnostic lanes, provider checks, poster recovery, metadata status | Metadata gather, embedded-title fallback, provider health checks |
| 0:28-0:40 | Scale architecture | Legacy-vs-CinaVault progress bars and responsive flow metrics | Paged loading, poster cache stability, safer duplicate cleanup |
| 0:40-0:51 | Real UI proof | Product dashboard screenshot with cinematic push-in and proof badges | Real CinaVault UI, diagnostics, enrichment, active progress |
| 0:51-1:01 | Final CTA | Brand lockup, neon shard transitions, future-tech color hits | Premium control, cleaner libraries, ready for growth |
| 1:01-1:08 | Platform availability | Full-screen Build 130 logo shot, glowing frame, shimmer pass, large download copy | Available on Android, Windows, and iOS systems; check applicable app store |

## Assets

- `public\assets\cinavault-premium-mark.png`
- `public\assets\cinavault-premium-brand-full.png`
- `public\assets\cinavault-build-130-logo.png`
- `public\assets\cinavault-ui-build-127.png`
- `public\audio\cinavault-promo-voiceover-v2.mp3`
- `public\audio\cinavault-energy-bed-v2.wav`

## Commands

- Install dependencies: `npm i`
- Lint and typecheck: `npm run lint`
- Still-frame check: `npm run still`
- Render MP4: `npm run render`
- Contact sheet: `ffmpeg -y -i output\cinavault-promo-v2.mp4 -vf "fps=1/6.8,scale=384:-1,tile=5x2" -update 1 output\cinavault-promo-v2-contact-sheet.jpg`

## Verification Results

- `npm run lint`: passed.
- `npm run still`: passed; generated `output\cinavault-promo-v2-check.png`.
- `npm run render`: passed; generated `output\cinavault-promo-v2.mp4`.
- Voiceover: generated with `edge-tts` using `en-US-ChristopherNeural`, rate `-8%`, pitch `-10Hz`; duration 60.648 seconds.
- Music bed: generated locally with FFmpeg synthesis, extended to 68.000 seconds with a final fadeout.
- Final MP4: 39,662,435 bytes.
- MP4 probe: H.264 video, 1920x1080, 30 fps, 2040 frames, AAC stereo audio, 68.053333 seconds.
- Contact sheet: `output\cinavault-promo-v2-contact-sheet.jpg` regenerated and visually checked across the runtime.
- Platform end-card proof: `output\cinavault-promo-v2-platform-end.png` generated and visually checked before final render.
- Final frame from rendered MP4: `output\cinavault-promo-v2-final-frame-from-mp4.png` extracted at 65 seconds and visually checked.
- Build 130 logo asset: copied from tag `build-130`; SHA256 `702CBAC06539A7E0A8230FFED56EE3998AEEE32110903EAAE1D0D1902143D9AB`.
- Music bed SHA256: `A2857ECC2023A636F1546ADA45DD9191C434543028BD364C90D5B5A5B942CF0F`.
- Final MP4 SHA256: `8ED42C84B9248D79C4E50E9D74D4FB47CA5DECA455BA9A15A3D3C36958CC0EAB`.

## Notes

- This v2 pass replaces the calmer v1 approach with a heavier cinematic technology look: animated HUD layers, diagonal light blasts, source telemetry, data ribbons, transition flashes, and a more aggressive motion cadence.
- The final render uses the Build 130 blue CineVault Media Server Pro Edition logo asset rather than the older red promo mark.
- A final full-screen platform availability card was added with: "Available on Android, Windows, and iOS systems. For downloads, check your applicable app store."
- The requested regional Southern voice was not directly available in the installed Windows voices or the fetched Edge TTS voice list. The final voiceover uses the deepest clear US neural male voice found locally, slowed and pitched down for a steadier, easier cadence.
- `edge-tts` was acquired with Python pip to improve voice quality over the earlier Windows desktop speech synthesis.
- `npm install` reported two low-severity audit findings from scaffold dependencies. No force fix was applied because that could change generated Remotion dependency versions.
