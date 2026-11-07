import { AlertTriangle, Inbox, Search, Upload, X } from "lucide-react";
import { useEffect, useMemo } from "react";
import { EmptyState } from "@/components/layout/EmptyState";
import { Pane } from "@/components/layout/Pane";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useConversations, useSearch, useSetState, useStateCounts } from "@/features/chats/queries";
import { useStartImport } from "@/features/import/useStartImport";
import { count } from "@/lib/format";
import { hasModifier, isTyping } from "@/lib/keys";
import { cn } from "@/lib/utils";
import { useImportStore } from "@/store/importStore";
import { SEARCH_INPUT_ID, useUiStore, type InboxFilter } from "@/store/uiStore";
import type { ChatState, SearchHit } from "@/types/app";
import { ConversationList } from "./ConversationList";
import { SearchResultsList } from "./SearchResultsList";

const FILTERS: { value: InboxFilter; label: string }[] = [
  { value: "untriaged", label: "Untriaged" },
  { value: "organised", label: "Organised" },
  { value: "archived", label: "Archived" },
  { value: "all", label: "All" },
];

const TRIAGE_KEYS: Record<string, ChatState> = {
  u: "untriaged",
  o: "organised",
  e: "archived",
};

function selectHit(hit: SearchHit) {
  useUiStore.getState().select({
    conversationId: hit.conversationId,
    // Hits on another branch open that branch, so the message is visible.
    via: hit.onMainPath ? null : hit.messageUuid,
    focus: hit.messageUuid,
  });
}

export function InboxPane() {
  const filter = useUiStore((s) => s.filter);
  const setFilter = useUiStore((s) => s.setFilter);
  const query = useUiStore((s) => s.query);
  const setQuery = useUiStore((s) => s.setQuery);
  const selection = useUiStore((s) => s.selection);
  const select = useUiStore((s) => s.select);

  const searching = query.trim().length > 0;
  const conversations = useConversations(filter);
  const counts = useStateCounts();
  const search = useSearch(query);
  const setState = useSetState();
  const startImport = useStartImport();
  const importing = useImportStore((s) => s.state.phase === "running");

  const rows = useMemo(() => conversations.data ?? [], [conversations.data]);
  const hits = useMemo(() => search.data?.hits ?? [], [search.data]);

  // Keyboard: j/k or arrows move through the list, u/o/e set the state.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (hasModifier(e)) return;
      const inSearch = e.target instanceof HTMLElement && e.target.id === SEARCH_INPUT_ID;
      if (isTyping(e) && !(inSearch && (e.key === "ArrowDown" || e.key === "ArrowUp"))) return;

      const step =
        e.key === "ArrowDown" || e.key === "j" ? 1 : e.key === "ArrowUp" || e.key === "k" ? -1 : 0;
      if (step !== 0) {
        e.preventDefault();
        if (searching) {
          const i = hits.findIndex((h) => h.messageUuid === selection?.focus);
          const next = hits[Math.min(Math.max(i + step, 0), hits.length - 1)];
          if (next) selectHit(next);
        } else {
          const i = rows.findIndex((r) => r.id === selection?.conversationId);
          const next = rows[Math.min(Math.max(i + step, 0), rows.length - 1)];
          if (next) select({ conversationId: next.id, via: null, focus: null });
        }
        return;
      }

      const target = TRIAGE_KEYS[e.key];
      if (target && selection) {
        e.preventDefault();
        // In a filtered list the chat is about to disappear, so move on first.
        if (!searching && filter !== "all" && filter !== target) {
          const i = rows.findIndex((r) => r.id === selection.conversationId);
          const next = rows[i + 1] ?? rows[i - 1];
          select(next ? { conversationId: next.id, via: null, focus: null } : null);
        }
        setState.mutate({ ids: [selection.conversationId], state: target });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [filter, hits, rows, searching, select, selection, setState]);

  const importButton = (
    <Button
      size="sm"
      variant="outline"
      className="h-7"
      disabled={importing || startImport.isPending}
      onClick={() => {
        startImport.mutate();
      }}
    >
      <Upload className="size-3.5" />
      Import
    </Button>
  );

  let body;
  if (searching) {
    if (search.isError) {
      body = (
        <EmptyState icon={AlertTriangle} title="Search failed" description={search.error.message} />
      );
    } else if (search.isPending) {
      body = <div className="h-full" aria-busy />;
    } else if (hits.length === 0) {
      body = (
        <EmptyState
          icon={Search}
          title="No matches"
          description="Search matches whole words and word beginnings. Put words in quotes to match a phrase."
        />
      );
    } else {
      body = (
        <SearchResultsList
          hits={hits}
          selectedUuid={selection?.focus ?? null}
          onSelect={selectHit}
        />
      );
    }
  } else if (conversations.isError) {
    body = (
      <EmptyState
        icon={AlertTriangle}
        title="Could not load chats"
        description={conversations.error.message}
      />
    );
  } else if (conversations.isPending) {
    body = <div className="h-full" aria-busy />;
  } else if (rows.length === 0) {
    const total = counts.data
      ? counts.data.untriaged + counts.data.organised + counts.data.archived
      : 0;
    body =
      total === 0 ? (
        <EmptyState
          icon={Inbox}
          title="No chats imported"
          description="Choose Import and select every zip from your Claude export, or its conversations.json."
        />
      ) : (
        <EmptyState
          icon={Inbox}
          title={`Nothing ${filter === "all" ? "here" : filter}`}
          description={
            filter === "untriaged" ? "Every chat has been triaged." : "Try another filter."
          }
        />
      );
  } else {
    body = (
      <ConversationList
        rows={rows}
        selectedId={selection?.conversationId ?? null}
        onSelect={(id) => {
          select({ conversationId: id, via: null, focus: null });
        }}
      />
    );
  }

  return (
    <Pane title="Inbox" actions={importButton}>
      <div className="flex h-full flex-col">
        <div className="relative shrink-0 border-b p-2">
          <Search className="pointer-events-none absolute top-1/2 left-4 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            id={SEARCH_INPUT_ID}
            value={query}
            placeholder="Search all messages  /"
            className="h-8 pr-8 pl-7 text-sm"
            onChange={(e) => {
              setQuery(e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setQuery("");
                e.currentTarget.blur();
              }
            }}
          />
          {query && (
            <button
              type="button"
              aria-label="Clear search"
              className="absolute top-1/2 right-4 -translate-y-1/2 rounded-sm p-0.5 text-muted-foreground hover:bg-accent"
              onClick={() => {
                setQuery("");
              }}
            >
              <X className="size-3.5" />
            </button>
          )}
        </div>

        {searching ? (
          <div className="shrink-0 border-b px-3 py-1.5 text-xs text-muted-foreground">
            {search.data
              ? search.data.total > hits.length
                ? `Best ${count(hits.length)} of ${count(search.data.total)} matching messages, all states`
                : `${count(search.data.total)} matching ${search.data.total === 1 ? "message" : "messages"}, all states`
              : "Searching…"}
          </div>
        ) : (
          <div className="flex shrink-0 gap-1 border-b px-2 py-1.5" role="tablist">
            {FILTERS.map((f) => {
              const n =
                f.value === "all"
                  ? counts.data
                    ? counts.data.untriaged + counts.data.organised + counts.data.archived
                    : null
                  : (counts.data?.[f.value] ?? null);
              return (
                <button
                  key={f.value}
                  type="button"
                  role="tab"
                  aria-selected={filter === f.value}
                  onClick={() => {
                    setFilter(f.value);
                  }}
                  className={cn(
                    "rounded-sm px-2 py-0.5 text-xs",
                    filter === f.value
                      ? "bg-secondary text-secondary-foreground"
                      : "text-muted-foreground hover:bg-accent",
                  )}
                >
                  {f.label}
                  {n !== null && <span className="ml-1 opacity-60">{count(n)}</span>}
                </button>
              );
            })}
          </div>
        )}

        <div className="min-h-0 flex-1">{body}</div>
      </div>
    </Pane>
  );
}
