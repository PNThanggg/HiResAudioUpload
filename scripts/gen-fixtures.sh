#!/usr/bin/env bash
# Regenerates src-tauri/tests/fixtures/*.mp3 and oracle.json (ffmpeg ebur128 loudness values).
# Run from Git Bash at the repo root:   bash scripts/gen-fixtures.sh
# ffmpeg (with libmp3lame) must be on PATH, or point to it:
#   FFMPEG=/path/to/ffmpeg.exe bash scripts/gen-fixtures.sh
# The ffprobe binary is taken from the same directory as FFMPEG unless FFPROBE is set.
set -euo pipefail

FFMPEG="${FFMPEG:-ffmpeg}"
FFPROBE="${FFPROBE:-$(dirname "$(command -v "$FFMPEG")")/ffprobe}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/src-tauri/tests/fixtures"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$OUT"
rm -f "$OUT"/*.mp3 "$OUT"/oracle.json

ff() { "$FFMPEG" -hide_banner -loglevel error -y "$@"; }
PINK="anoisesrc=color=pink:sample_rate=44100:amplitude=0.1:duration=30:seed=42"
PINK_SRC=(-f lavfi -i "$PINK" -ac 2)

ff "${PINK_SRC[@]}" -c:a libmp3lame -b:a 320k "$OUT/pink_320.mp3"
ff "${PINK_SRC[@]}" -c:a libmp3lame -b:a 128k "$TMP/pink_128_tmp.mp3"
ff -i "$TMP/pink_128_tmp.mp3" -c:a libmp3lame -b:a 320k "$OUT/pink_128to320.mp3"
ff "${PINK_SRC[@]}" -af "volume=30dB,alimiter=limit=1:level=false:attack=0.1:release=1,volume=2dB" -c:a libmp3lame -b:a 320k "$OUT/pink_loud.mp3"
ff "${PINK_SRC[@]}" -c:a libmp3lame -q:a 2 "$OUT/pink_v2.mp3"
ff "${PINK_SRC[@]}" -c:a libmp3lame -b:a 128k "$OUT/pink_128.mp3"

# Covers: JPEG noise > 200 KB, and a 64x64 PNG
ff -f lavfi -i "color=c=gray:s=1600x1600,geq=lum='random(1)*255':cb=128:cr=128" -frames:v 1 -q:v 1 "$TMP/cover_big.jpg"
ff -f lavfi -i "color=c=red:s=64x64" -frames:v 1 "$TMP/cover_small.png"

for kind in big small; do
  if [ "$kind" = big ]; then img="$TMP/cover_big.jpg"; else img="$TMP/cover_small.png"; fi
  tags=()
  if [ "$kind" = big ]; then
    tags=(-metadata "title=Bài hát thử" -metadata "artist=Nghệ sĩ" -metadata "album=Album thử")
  fi
  ff -i "$OUT/pink_320.mp3" -i "$img" -map 0:a -map 1:v -c:a copy -c:v copy \
     -disposition:v attached_pic -id3v2_version 3 "${tags[@]}" "$OUT/cover_$kind.mp3"
done

ff -f lavfi -i "anoisesrc=color=pink:sample_rate=44100:amplitude=0.1:duration=5:seed=42" -ac 2 \
   -c:a libmp3lame -b:a 192k -write_xing 0 -map_metadata -1 -id3v2_version 0 "$OUT/noxing_notag.mp3"
ff -f lavfi -i "anullsrc=r=44100:cl=stereo" -t 5 -c:a libmp3lame -b:a 128k "$OUT/silent.mp3"
ff -f lavfi -i "anoisesrc=color=pink:sample_rate=44100:amplitude=0.1:duration=0.5:seed=42" -ac 2 \
   -c:a libmp3lame -b:a 128k "$OUT/short.mp3"
ff -f lavfi -i "anoisesrc=color=pink:sample_rate=44100:amplitude=0.1:duration=1:seed=42" -ac 2 \
   -f wav "$OUT/fake.mp3"

# Oracle: ebur128 Summary of every analysable fixture
{
  echo "{"
  first=1
  for name in pink_320 pink_128to320 pink_loud pink_v2 pink_128 cover_big cover_small noxing_notag; do
    f="$name.mp3"
    vals=$("$FFMPEG" -hide_banner -nostats -i "$OUT/$f" -af ebur128=peak=true -f null - 2>&1 | awk '
      /Summary:/ {s=1}
      s && /^ *I:/ {lufs=$2}
      s && /^ *LRA:/ {lra=$2}
      s && /^ *Peak:/ {tp=$2}
      END {print lufs, lra, tp}')
    set -- $vals
    [ $# -eq 3 ] || { echo "oracle parse failed for $f: '$vals'" >&2; exit 1; }
    [ $first -eq 1 ] || echo ","
    first=0
    printf '  "%s": { "lufs": %s, "lra": %s, "truePeakDbtp": %s }' "$f" "$1" "$2" "$3"
  done
  echo
  echo "}"
} > "$OUT/oracle.json"

# Report
echo "--- sizes"; ls -l "$OUT" | awk '{print $5, $9}'
"$FFMPEG" -hide_banner -loglevel error -y -i "$OUT/cover_big.mp3" -an -c:v copy "$TMP/extracted.jpg"
echo "--- cover_big embedded JPEG bytes: $(stat -c %s "$TMP/extracted.jpg")"
"$FFPROBE" -hide_banner -show_format "$OUT/cover_big.mp3" 2>/dev/null | grep -i 'TAG:' || true
echo "--- oracle.json"; cat "$OUT/oracle.json"
