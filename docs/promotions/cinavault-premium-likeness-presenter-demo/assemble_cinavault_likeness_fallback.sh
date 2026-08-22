#!/usr/bin/env bash
set -euo pipefail
ROOT=/home/ubuntu/cross-repo-audit
ASSETS="$ROOT/cinavault-revised-assets"
OUTDIR="$ROOT/cinavault-likeness-fallback-work"
OUT="$ROOT/cinavault-premium-likeness-presenter-demo.mp4"
FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
PRESENTER="$ROOT/cinavault-user-likeness-presenter-reference.png"
mkdir -p "$OUTDIR"
rm -f "$OUTDIR"/seg*.mp4 "$OUTDIR/concat.txt" "$OUT"
make_seg(){
  local n="$1" src="$2" dur="$3" title="$4" subtitle="$5"
  ffmpeg -y -loglevel error -loop 1 -framerate 30 -t "$dur" -i "$src" \
    -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=0x040914,setsar=1,eq=saturation=1.1:contrast=1.04,zoompan=z='1+0.025*sin(on/75)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s=1920x1080:fps=30,drawbox=x=0:y=0:w=1920:h=86:color=0x02040dcc:t=fill,drawtext=fontfile=$FONT:text='$title':fontcolor=0xD9FBFF:fontsize=30:x=72:y=25,drawtext=fontfile=$FONT:text='$subtitle':fontcolor=0x7DE8F4:fontsize=18:x=72:y=58" \
    -c:v libx264 -preset medium -crf 18 -pix_fmt yuv420p "$OUTDIR/seg${n}.mp4"
}
make_seg 01 "$PRESENTER" 8 "CINAVAULT PREMIUM" "STEP INTO THE VAULT"
make_seg 02 "$ASSETS/dashboard-authentic-16x9.png" 16 "THE VAULT  |  v2.14 BUILD 1.14" "AUTHENTIC DASHBOARD CAPTURE"
make_seg 03 "$ASSETS/downloads-authentic-16x9.png" 16 "INCOMING MEDIA  |  ACQUISITION STREAM / QUEUE" "AUTHENTIC DOWNLOADS CAPTURE"
make_seg 04 "$PRESENTER" 10 "YOUR GUIDE THROUGH THE VAULT" "USER-LIKENESS PROMOTIONAL PRESENTER"
make_seg 05 "$ASSETS/dashboard-authentic-16x9.png" 12 "AI AUTOPILOT" "SCANNING  ·  ENRICHING  ·  REPAIRING"
make_seg 06 "$ASSETS/downloads-authentic-16x9.png" 10 "MEDIA FILE TOOLS" "RUN MEDIAINFO  ·  RUN MKVTOOLNIX  ·  RECHECK & REPAIR"
make_seg 07 "$PRESENTER" 8 "CINAVAULT PREMIUM" "THE VAULT IS WAITING"
for f in "$OUTDIR"/seg*.mp4; do printf "file '%s'\n" "$f" >> "$OUTDIR/concat.txt"; done
ffmpeg -y -loglevel error -f concat -safe 0 -i "$OUTDIR/concat.txt" -c copy "$OUTDIR/video-only.mp4"
ffmpeg -y -loglevel error -i "$OUTDIR/video-only.mp4" -i "$ROOT/cinavault-premium-65sec-voiceover.wav" \
  -filter_complex "[1:a]apad=pad_dur=11,atrim=0:80,afade=t=in:st=0:d=1,afade=t=out:st=78:d=2[a]" \
  -map 0:v:0 -map "[a]" -t 80 -c:v copy -c:a aac -b:a 192k -shortest -movflags +faststart "$OUT"
ffprobe -v error -show_entries format=duration,size -of default=nw=1 "$OUT"
sha256sum "$OUT" > "$ROOT/cinavault-premium-likeness-presenter-demo-sha256.txt"
echo "$OUT"
