import type { TrackSummary, UploadProgress, UploadStage } from "./types";
import { NOT_CONFIGURED } from "./types";

export interface QueueItem {
  path: string;
  fileName: string;
  stage: UploadStage;
  errorCode?: string;
  message?: string;
  track?: TrackSummary;
}

export interface QueueSummary {
  total: number;
  done: number;
  duplicate: number;
  error: number;
  active: number;
}

export const isTerminal = (stage: UploadStage): boolean =>
  stage === "done" || stage === "duplicate" || stage === "error";

/**
 * Upserts by path, keeping arrival order. A terminal stage is final: late
 * non-terminal events are ignored. A new "queued" event restarts the path
 * (the same file uploaded again).
 */
export function applyProgress(items: QueueItem[], ev: UploadProgress): QueueItem[] {
  const next: QueueItem = {
    path: ev.path,
    fileName: ev.fileName,
    stage: ev.stage,
    ...(ev.errorCode ? { errorCode: ev.errorCode } : {}),
    ...(ev.message ? { message: ev.message } : {}),
    ...(ev.track ? { track: ev.track } : {}),
  };
  const i = items.findIndex((it) => it.path === ev.path);
  if (i < 0) return [...items, next];
  if (isTerminal(items[i].stage) && ev.stage !== "queued") return items;
  return items.map((it, idx) => (idx === i ? next : it));
}

export function summarize(items: QueueItem[]): QueueSummary {
  const count = (stage: UploadStage) => items.filter((i) => i.stage === stage).length;
  const done = count("done");
  const duplicate = count("duplicate");
  const error = count("error");
  return { total: items.length, done, duplicate, error, active: items.length - done - duplicate - error };
}

export function clearFinished(items: QueueItem[]): QueueItem[] {
  return items.filter((i) => !isTerminal(i.stage));
}

const ERROR_TEXT: Record<string, string> = {
  INVALID_FORMAT: "Nội dung không phải MP3 hợp lệ.",
  TOO_LARGE: "File quá lớn (tối đa 200 MB, dài tối đa 2 giờ).",
  TOO_SHORT: "File ngắn hơn 1 giây.",
  SILENT: "File chỉ có im lặng.",
  TIMEOUT: "Phân tích quá 5 phút, đã huỷ.",
  INTERNAL: "Lỗi nội bộ khi xử lý file.",
  STORAGE: "Không ghi được lên R2. Thư viện chưa thay đổi.",
  CATALOG_CORRUPT: "Catalog trên R2 bị lỗi. Không ghi gì lên bucket cho tới khi catalog được sửa.",
};

/** Vietnamese text for an upload error code (docs/design/README.md §4.7). */
export function errorText(code: string | null | undefined, message: string | null | undefined): string {
  if (message === NOT_CONFIGURED) return "Chưa cấu hình R2. Mở Cài đặt để nhập thông tin kết nối.";
  if (code && ERROR_TEXT[code]) return ERROR_TEXT[code];
  return message || "Lỗi không xác định.";
}
