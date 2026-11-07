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
  excluded: boolean;
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

export type AiMode = "off" | "api";

export interface Preferences {
  checkUpdatesOnStartup: boolean;
  aiMode: AiMode;
}

export type TagScope = "sample" | "all";

export interface Estimate {
  conversations: number;
  alreadyTagged: number;
  cached: number;
  toSend: number;
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
  calibrated: boolean;
  model: string;
}

export interface TagProgress {
  done: number;
  total: number;
  failed: number;
  costUsd: number;
}

export interface TagSummary {
  runId: number;
  tagged: number;
  skipped: number;
  fromCache: number;
  failed: number;
  cancelled: boolean;
  repairs: number;
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
  errors: string[];
}

export interface ThemeRow {
  id: number;
  name: string;
  origin: string;
  segments: number;
  conversations: number;
  lastActive: string | null;
}

export interface ThemeSegment {
  segmentId: number;
  conversationId: number;
  conversationName: string;
  conversationUpdatedAt: string;
  title: string;
  summary: string;
  intent: string;
  startUuid: string | null;
  confidence: number | null;
}

export interface SegmentInfo {
  id: number;
  startIdx: number;
  endIdx: number;
  title: string;
  summary: string;
  intent: string;
  source: string;
  themes: string[];
  entities: string[];
}

export type MemoryKind = "decision" | "preference" | "open_question" | "project" | "fact";
export type MemoryStatus = "suggested" | "confirmed" | "dismissed";

export interface MemoryRow {
  id: number;
  kind: MemoryKind;
  text: string;
  status: MemoryStatus;
  conversationId: number;
  conversationName: string;
  sourceUuid: string | null;
  sourceOnMainPath: boolean;
  sourceAt: string | null;
}

export interface TaggingStats {
  taggedConversations: number;
  themes: number;
  suggestedItems: number;
  totalCostUsd: number;
}
