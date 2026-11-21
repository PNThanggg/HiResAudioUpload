import { describe, expect, it } from "vitest";
import type { TrackSummary } from "../types";
import { coverCell } from "./library";

const track: TrackSummary = {
  id: "abc",
  sourceSha256: "def",
  audioKey: "audio/abc.mp3",
  coverKey: "cover/c.jpg",
  audioUrl: "https://cdn.example.com/audio/abc.mp3",
  coverUrl: "https://cdn.example.com/cover/c.jpg",
  sizeBytes: 1,
  codec: "mp3",
  bitrate: 320,
  vbr: false,
  sampleRate: 44100,
  channels: 2,
  durationMs: 1000,
  title: "$` onload=x",
  artist: null,
  album: "$` onload=x $& $'",
  albumArtist: null,
  date: null,
  genre: null,
  track: null,
  disc: null,
  integratedLufs: -14,
  truePeakDbtp: -1,
  lra: 5,
  effectiveBandwidthHz: null,
  qualityHint: null,
  trimStart: null,
  trimEnd: null,
  totalSamples: null,
  analysisVersion: 1,
  createdAt: "2026-11-21T00:00:00Z",
  updatedAt: "2026-11-21T00:00:00Z",
};

describe("coverCell", () => {
  it("renders replacement patterns in the alt text as literal escaped text", () => {
    const html = coverCell(track, "cover cover--detail", `Ảnh bìa ${track.album} ${track.title}`);
    expect(html).toBe(
      `<img class="cover cover--detail" src="https://cdn.example.com/cover/c.jpg" alt="Ảnh bìa $\` onload=x $&amp; $&#39; $\` onload=x" loading="lazy">`,
    );
    expect(html.match(/<img/g)).toHaveLength(1);
    expect(html).not.toContain('" onload');
  });

  it("escapes quotes and markup in the alt text", () => {
    const html = coverCell(track, "cover", `"><script>x</script>`);
    expect(html).toContain('alt="&quot;&gt;&lt;script&gt;x&lt;/script&gt;"');
  });

  it("keeps the alt empty by default", () => {
    expect(coverCell(track)).toContain('alt=""');
  });
});
