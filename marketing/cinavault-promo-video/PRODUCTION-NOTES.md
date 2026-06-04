# CinaVault Promo Video v1

Date: 2026-06-03
Source branch: `codex/cinavault-promo-video-v1`
Project folder in repo: `marketing\cinavault-promo-video`
Original working folder: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault-promo-video`
Composition: `CinaVaultPromo`
Runtime: 40 seconds at 30 fps, 1920x1080
Primary export: `output\cinavault-promo-v1.mp4`

## Final Narration Script

Your media library should not feel like a pile of folders. CinaVault turns growth into fast indexing, clean artwork, and searchable control.
CinaVault Premium Media Server brings local drives, NAS shares, cloud folders, live TV, remote access, security, and plugins into one command center.
AI tools enrich metadata, recover posters, apply embedded titles, check providers, and clean duplicates while preserving the details that matter.
From desktop storage to NAS vaults and remote users, CinaVault keeps sources visible, permissions clear, and playback workflows connected.
The difference is architecture: paged loading, stable poster caches, provider-aware enrichment, and diagnostics that show what the server is doing.
Don't settle for outdated technology. Choose the best in class. Join the future with CinaVault Media Server.
Upgrade to CinaVault today and experience the difference for yourself. Learn more at your CinaVault site.

## Scene Timing

| Time | Scene | Flash / Visual Treatment | Knowledge Points |
| --- | --- | --- | --- |
| 0:00-0:05 | Library growth | Animated media tiles, climbing item count, scanline grid, moving spotlight | 10k+ item library proof; responsive paging; poster-first UI |
| 0:05-0:12 | Product reveal | Brand reveal, connected source nodes, pulsing server core | Local + NAS + Cloud; Live TV ready; plugin architecture |
| 0:12-0:19 | AI automation | AI task lanes fill and complete with bright status changes | Embedded-title fallback; provider health checks; safe duplicate removal |
| 0:19-0:26 | Access layer | Storage/source endpoints orbit the server core | Remote access controls; cloud and NAS paths; security status at a glance |
| 0:26-0:31 | Scalability | Performance bars compare legacy workflow against CinaVault path | Paged loading; stable poster cache; live task progress |
| 0:31-0:36 | Interface proof | Real CinaVault interface shot pushes in with proof chips | Real product UI; AI diagnostics; build-proven workflow |
| 0:36-0:40 | CTA | Logo end screen, upgrade message, generated synth bed fadeout | Premium media control; cleaner libraries; ready for growth |

## Assets

- `public\assets\cinavault-premium-mark.png`
- `public\assets\cinavault-premium-brand-full.png`
- `public\assets\cinavault-ui-build-127.png`
- `public\audio\cinavault-promo-voiceover.wav`
- `public\audio\cinavault-energy-bed.wav`

## Commands

- Install: `npm i`
- Lint and typecheck: `npm run lint`
- Still-frame check: `npm run still`
- Render MP4: `npm run render`

## Verification Results

- `npm run lint`: passed.
- Voiceover duration: 34.242358 seconds.
- Music bed duration: 40.000000 seconds.
- Still render: `output\cinavault-promo-check.png` generated at frame 965.
- Final MP4: `output\cinavault-promo-v1.mp4`, 19,189,263 bytes.
- MP4 probe: H.264 video, 1920x1080, 30 fps, 1200 frames, AAC stereo audio, 40.042667 seconds.
- Contact sheet: `output\cinavault-promo-contact-sheet.jpg` generated and visually checked across the runtime.

## Notes

- The public website placeholder from the original concept was intentionally changed to "your CinaVault site" because no production website URL was provided.
- The voiceover was generated locally with Windows speech synthesis.
- The music bed was generated locally with FFmpeg sine sources and does not use copyrighted music.
- `npm install` reported two low-severity audit findings from scaffold dependencies. No force fix was applied because that could change generated Remotion dependency versions.
