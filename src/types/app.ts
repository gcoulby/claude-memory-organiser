/** Mirrors `AppError` in src-tauri/src/error.rs. */
export interface AppError {
  kind: string;
  message: string;
}

export interface AppInfo {
  name: string;
  version: string;
}

export type VaultStatus = "new" | "locked" | "unlocked";

export interface LibraryStats {
  conversations: number;
  messages: number;
  lastImportAt: number | null;
}

export interface ImportProgress {
  file: string;
  bytesDone: number;
  bytesTotal: number;
  conversations: number;
}

export interface ImportSummary {
  conversationsSeen: number;
  conversationsAdded: number;
  conversationsUpdated: number;
  conversationsUnchanged: number;
  conversationsSkipped: number;
  messagesAdded: number;
  messagesUpdated: number;
  messagesSkipped: number;
  filesStored: number;
  durationMs: number;
}
