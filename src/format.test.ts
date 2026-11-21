import { describe, expect, it } from "vitest";
import {
  formatDuration,
  formatLufs,
  formatSize,
  formatTruePeak,
  qualityLabel,
} from "./format";

describe("qualityLabel", () => {
  it("labels medium sources with a high bitrate as re-encodes", () => {
    expect(qualityLabel(320, "medium")).toBe("MP3 320 · nguồn ~128k");
  });
  it("labels medium sources with a low bitrate as Medium", () => {
    expect(qualityLabel(160, "medium")).toBe("MP3 160 · Medium");
  });
  it("labels full, high and low", () => {
    expect(qualityLabel(320, "full")).toBe("MP3 320 · Full band");
    expect(qualityLabel(245, "high")).toBe("MP3 245 · High (~V0–V2)");
    expect(qualityLabel(128, "low")).toBe("MP3 128 · Nguồn kém");
  });
  it("shows only the bitrate when the hint is null", () => {
    expect(qualityLabel(128, null)).toBe("MP3 128");
  });
});

describe("formatDuration", () => {
  it("formats m:ss", () => {
    expect(formatDuration(390000)).toBe("6:30");
  });
  it("formats h:mm:ss from one hour", () => {
    expect(formatDuration(3_725_000)).toBe("1:02:05");
  });
});

describe("formatSize", () => {
  it("formats MiB with one decimal", () => {
    expect(formatSize(15_600_000)).toBe("14.9 MB");
  });
});

describe("formatLufs / formatTruePeak", () => {
  it("formats LUFS with one decimal and the unit", () => {
    expect(formatLufs(-8.14)).toBe("-8.1 LUFS");
  });
  it("adds an explicit plus for positive true peak", () => {
    expect(formatTruePeak(0.3)).toBe("+0.3 dBTP");
    expect(formatTruePeak(-1.1)).toBe("-1.1 dBTP");
  });
});
