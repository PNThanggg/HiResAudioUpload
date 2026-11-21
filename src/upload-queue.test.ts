import { describe, expect, it } from "vitest";
import {
  applyProgress,
  clearFinished,
  errorText,
  summarize,
  type QueueItem,
} from "./upload-queue";
import type { UploadProgress } from "./types";

const ev = (path: string, stage: UploadProgress["stage"], extra: Partial<UploadProgress> = {}): UploadProgress => ({
  path,
  fileName: path.split("/").pop() ?? path,
  stage,
  errorCode: null,
  message: null,
  track: null,
  ...extra,
});

describe("applyProgress", () => {
  it("appends new paths in arrival order and upserts existing ones in place", () => {
    let items: QueueItem[] = [];
    items = applyProgress(items, ev("/a.mp3", "queued"));
    items = applyProgress(items, ev("/b.mp3", "queued"));
    items = applyProgress(items, ev("/a.mp3", "analysing"));
    expect(items.map((i) => [i.path, i.stage])).toEqual([
      ["/a.mp3", "analysing"],
      ["/b.mp3", "queued"],
    ]);
  });

  it("does not mutate its input", () => {
    const before: QueueItem[] = [{ path: "/a.mp3", fileName: "a.mp3", stage: "queued" }];
    applyProgress(before, ev("/a.mp3", "done"));
    expect(before[0].stage).toBe("queued");
  });

  it("keeps a terminal stage when a late non-terminal event arrives", () => {
    let items = applyProgress([], ev("/a.mp3", "done"));
    items = applyProgress(items, ev("/a.mp3", "uploading"));
    expect(items[0].stage).toBe("done");
  });

  it("carries error code, message and track", () => {
    const items = applyProgress([], ev("/a.mp3", "error", { errorCode: "SILENT", message: "m" }));
    expect(items[0]).toMatchObject({ stage: "error", errorCode: "SILENT", message: "m" });
  });

  it("a fresh queued event restarts a finished path (re-upload)", () => {
    let items = applyProgress([], ev("/a.mp3", "error", { errorCode: "SILENT" }));
    items = applyProgress(items, ev("/a.mp3", "queued"));
    expect(items[0].stage).toBe("queued");
    expect(items[0].errorCode).toBeUndefined();
  });
});

describe("summarize", () => {
  it("counts each bucket", () => {
    let items: QueueItem[] = [];
    for (const [p, s] of [
      ["/1", "done"],
      ["/2", "duplicate"],
      ["/3", "error"],
      ["/4", "queued"],
      ["/5", "analysing"],
      ["/6", "uploading"],
    ] as const) items = applyProgress(items, ev(p, s));
    expect(summarize(items)).toEqual({ total: 6, done: 1, duplicate: 1, error: 1, active: 3 });
  });
});

describe("clearFinished", () => {
  it("removes terminal items and keeps active ones", () => {
    let items: QueueItem[] = [];
    for (const [p, s] of [
      ["/1", "done"],
      ["/2", "uploading"],
      ["/3", "error"],
      ["/4", "queued"],
    ] as const) items = applyProgress(items, ev(p, s));
    expect(clearFinished(items).map((i) => i.path)).toEqual(["/2", "/4"]);
  });
});

describe("errorText", () => {
  it("maps known codes to Vietnamese text", () => {
    expect(errorText("TOO_SHORT", null)).toBe("File ngắn hơn 1 giây.");
    expect(errorText("INVALID_FORMAT", null)).toBe("Nội dung không phải MP3 hợp lệ.");
    expect(errorText("STORAGE", "x")).toBe("Không ghi được lên R2. Thư viện chưa thay đổi.");
  });
  it("routes NOT_CONFIGURED to Settings", () => {
    expect(errorText("VALIDATION", "NOT_CONFIGURED")).toContain("Cài đặt");
  });
  it("falls back to the raw message, then a generic text", () => {
    expect(errorText("WEIRD", "boom")).toBe("boom");
    expect(errorText(null, null)).toBe("Lỗi không xác định.");
  });
});
