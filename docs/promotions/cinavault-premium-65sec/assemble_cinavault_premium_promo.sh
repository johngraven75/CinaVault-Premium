#!/usr/bin/env bash
set -euo pipefail

ROOT="/home/ubuntu/CinaVault-Premium"
OUT="/home/ubuntu/cross-repo-audit/cinavault-premium-65sec-promo"
VOICE="/home/ubuntu/cross-repo-audit/cinavault-premium-65sec-voiceover.wav"
WORK="/home/ubuntu/cross-repo-audit/.cinavault-premium-promo-work"
FONT="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
BOLD="/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

rm -rf "$WORK"
mkdir -p "$WORK"

# The branding assets are authentic repository artwork. The feature frames are
# explicitly stylized visualizations, not fabricated application screenshots.
make_clip() {
  local idx="$1" duration="$2" image="$3" kicker="$4" title="$5"
  local endfade
  endfade=$(awk "BEGIN { printf \"%.2f\", $duration-0.85 }")
  ffmpeg -y -loglevel error -loop 1 -i "$image" -t "$duration" \
    -vf "scale=1280:720:force_original_aspect_ratio=increase,crop=1280:720,zoompan=z='min(zoom+0.0007,1.08)':d=1:s=1280x720:fps=30,eq=contrast=1.12:saturation=1.2,drawbox=x=0:y=598:w=1280:h=2:color=0x36dce8@0.8:t=fill,drawtext=fontfile=$FONT:text='$kicker':fontcolor=0x9fe5ec:fontsize=18:x=70:y=70,drawtext=fontfile=$BOLD:text='$title':fontcolor=white:fontsize=38:box=1:boxcolor=0x050a14@0.52:boxborderw=18:x=70:y=610,fade=t=in:st=0:d=0.85,fade=t=out:st=$endfade:d=0.85,format=yuv420p" \
    -an "$WORK/clip-$idx.mp4"
}

make_clip 01 5 "$ROOT/public/branding/cinavault-premium-banner.png" "CINAVAULT PREMIUM" "Your library. Reimagined."
make_clip 02 7 "$ROOT/public/branding/cinavault-premium-banner.png" "01 / LIBRARY INTELLIGENCE" "Scan the collection"
make_clip 03 8 "/home/ubuntu/cinavault-holo-metadata.jpg" "02 / METADATA ENRICHMENT" "Make every title count"
make_clip 04 8 "/home/ubuntu/cinavault-holo-sidecars.jpg" "03 / SIDECAR CONTINUITY" "Keep the story with the file"
make_clip 05 8 "/home/ubuntu/cinavault-holo-playback.jpg" "04 / PLAYBACK + REPAIR" "From file to focus"
make_clip 06 8 "/home/ubuntu/cinavault-holo-scale.jpg" "05 / ACCESS + SCALE" "A library that keeps up"
make_clip 07 10 "$ROOT/public/branding/cinavault-premium-banner.png" "CINAVAULT PREMIUM" "Posters. Sidecars. Control."
make_clip 08 5 "$ROOT/public/branding/cinavault-premium-mark.png" "CINAVAULT PREMIUM" "Built for real libraries"
make_clip 09 6 "$ROOT/public/branding/cinavault-premium-banner.png" "EXPLORE THE LATEST BUILD" "github.com/johngraven75/CinaVault-Premium"

: > "$WORK/concat.txt"
for f in "$WORK"/clip-*.mp4; do printf "file '%s'\n" "$f" >> "$WORK/concat.txt"; done
ffmpeg -y -loglevel error -f concat -safe 0 -i "$WORK/concat.txt" -c copy "$WORK/video-only.mp4"

ffmpeg -y -loglevel error -i "$WORK/video-only.mp4" -i "$VOICE" \
  -filter_complex "[1:a]apad=pad_dur=65,atrim=0:65,volume=1.32[a]" \
  -map 0:v:0 -map "[a]" -t 65 -c:v copy -c:a aac -b:a 192k -movflags +faststart \
  "$OUT.mp4"

ffprobe -v error -show_entries format=duration,size -of default=noprint_wrappers=1 "$OUT.mp4"
