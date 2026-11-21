import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { mockApi } from "./api.mock";
import type {
  Settings,
  TagEdit,
  TrackRecord,
  TrackSummary,
  UploadProgress,
} from "./types";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function listTracks(): Promise<TrackSummary[]> {
  return inTauri ? invoke("list_tracks") : mockApi.listTracks();
}

export function getTrack(id: string): Promise<TrackRecord> {
  return inTauri ? invoke("get_track", { id }) : mockApi.getTrack(id);
}

export function getSettings(): Promise<Settings> {
  return inTauri ? invoke("get_settings") : mockApi.getSettings();
}

export function saveSettings(s: Settings): Promise<void> {
  return inTauri ? invoke("save_settings", { s }) : mockApi.saveSettings(s);
}

/** Rejects with a string message on failure. */
export function testConnection(s: Settings): Promise<void> {
  return inTauri ? invoke("test_connection", { s }) : mockApi.testConnection(s);
}

export function uploadFiles(paths: string[]): Promise<void> {
  return inTauri ? invoke("upload_files", { paths }) : mockApi.uploadFiles(paths);
}

export function updateTrack(
  id: string,
  edit: TagEdit,
  coverPath?: string | null,
): Promise<TrackSummary> {
  return inTauri
    ? invoke("update_track", { id, edit, coverPath: coverPath ?? null })
    : mockApi.updateTrack(id, edit, coverPath);
}

export function deleteTracks(ids: string[]): Promise<void> {
  return inTauri ? invoke("delete_tracks", { ids }) : mockApi.deleteTracks(ids);
}

/** Subscribes to upload progress; returns a function that unsubscribes. */
export function onUploadProgress(cb: (p: UploadProgress) => void): () => void {
  if (!inTauri) return mockApi.onUploadProgress(cb);
  const pending = listen<UploadProgress>("upload-progress", (e) => cb(e.payload));
  return () => {
    void pending.then((unlisten) => unlisten());
  };
}

export async function pickFiles(): Promise<string[]> {
  if (!inTauri) return mockApi.pickFiles();
  const picked = await open({
    multiple: true,
    filters: [{ name: "MP3", extensions: ["mp3"] }],
  });
  return picked ?? [];
}

export async function pickImage(): Promise<string | null> {
  if (!inTauri) return mockApi.pickImage();
  const picked = await open({
    multiple: false,
    filters: [{ name: "Ảnh", extensions: ["jpg", "jpeg", "png"] }],
  });
  return picked ?? null;
}
