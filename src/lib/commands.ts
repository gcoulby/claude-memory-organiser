import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  AppInfo,
  ChatState,
  ConversationDetail,
  ConversationRow,
  Estimate,
  LibraryStats,
  MemoryRow,
  MemoryStatus,
  Preferences,
  SegmentInfo,
  TaggingStats,
  TagScope,
  ThemeRow,
  ThemeSegment,
  SearchResults,
  StateCounts,
  VaultStatus,
} from "@/types/app";

/** A failed command. `kind` comes from Rust, or is "bridge" when the call never reached it. */
export class CommandError extends Error {
  readonly kind: string;

  constructor({ kind, message }: AppError) {
    super(message);
    this.name = "CommandError";
    this.kind = kind;
  }
}

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    typeof value.kind === "string" &&
    "message" in value &&
    typeof value.message === "string"
  );
}

export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw new CommandError(isAppError(err) ? err : { kind: "bridge", message: String(err) });
  }
}

export const commands = {
  appInfo: () => call<AppInfo>("app_info"),
  vaultStatus: () => call<VaultStatus>("vault_status"),
  vaultCreate: (password: string) => call<null>("vault_create", { password }),
  vaultUnlock: (password: string) => call<null>("vault_unlock", { password }),
  vaultLock: () => call<null>("vault_lock"),
  importChooseAndStart: () => call<boolean>("import_choose_and_start"),
  importCancel: () => call<null>("import_cancel"),
  libraryStats: () => call<LibraryStats>("library_stats"),
  listConversations: (filter: ChatState | null) =>
    call<ConversationRow[]>("list_conversations", { filter }),
  stateCounts: () => call<StateCounts>("state_counts"),
  getConversation: (id: number, via: string | null) =>
    call<ConversationDetail>("get_conversation", { id, via }),
  setConversationState: (ids: number[], newState: ChatState) =>
    call<null>("set_conversation_state", { ids, newState }),
  getPreferences: () => call<Preferences>("get_preferences"),
  setPreferences: (preferences: Preferences) => call<null>("set_preferences", { preferences }),
  backupNow: () => call<string>("backup_now"),
  exportEdits: () => call<boolean>("export_edits"),
  apiKeyPresent: () => call<boolean>("api_key_present"),
  setApiKey: (key: string) => call<null>("set_api_key", { key }),
  clearApiKey: () => call<null>("clear_api_key"),
  taggingEstimate: (scope: TagScope) => call<Estimate>("tagging_estimate", { scope }),
  taggingStart: (scope: TagScope) => call<null>("tagging_start", { scope }),
  taggingCancel: () => call<null>("tagging_cancel"),
  taggingStats: () => call<TaggingStats>("tagging_stats"),
  listThemes: () => call<ThemeRow[]>("list_themes"),
  themeSegments: (themeId: number) => call<ThemeSegment[]>("theme_segments", { themeId }),
  conversationSegments: (conversationId: number) =>
    call<SegmentInfo[]>("conversation_segments", { conversationId }),
  listMemory: () => call<MemoryRow[]>("list_memory"),
  setMemoryStatus: (id: number, status: MemoryStatus) =>
    call<null>("set_memory_status", { id, status }),
  setConversationExcluded: (ids: number[], excluded: boolean) =>
    call<null>("set_conversation_excluded", { ids, excluded }),
  searchMessages: (query: string) => call<SearchResults>("search_messages", { query }),
};

export const events = {
  importProgress: "import://progress",
  importFinished: "import://finished",
  importFailed: "import://failed",
  taggingProgress: "tagging://progress",
  taggingFinished: "tagging://finished",
  taggingFailed: "tagging://failed",
} as const;
