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

export type ChatState = "untriaged" | "organised" | "archived";

export interface ConversationRow {
  id: number;
  name: string;
  createdAt: string;
  updatedAt: string;
  state: ChatState;
  messageCount: number;
}

export interface StateCounts {
  untriaged: number;
  organised: number;
  archived: number;
}

export interface ToolUse {
  name: string;
  count: number;
}

export interface ReaderMessage {
  id: number;
  uuid: string;
  sender: string;
  text: string;
  createdAt: string;
  attachments: string[];
  files: string[];
  tools: ToolUse[];
  hasThinking: boolean;
  siblings: string[];
  siblingIndex: number;
}

export interface ConversationDetail {
  id: number;
  uuid: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  state: ChatState;
  offMainPath: boolean;
  messages: ReaderMessage[];
}

export interface SearchHit {
  messageId: number;
  messageUuid: string;
  conversationId: number;
  conversationName: string;
  sender: string;
  createdAt: string;
  onMainPath: boolean;
  inAttachment: boolean;
  snippet: string;
}

export interface SearchResults {
  hits: SearchHit[];
  total: number;
}

export interface Preferences {
  checkUpdatesOnStartup: boolean;
}
