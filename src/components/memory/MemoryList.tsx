import { AlertTriangle, Check, Lightbulb, X } from "lucide-react";
import { EmptyState } from "@/components/layout/EmptyState";
import { useMemory, useSetMemoryStatus } from "@/features/tagging/queries";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/store/uiStore";
import type { MemoryKind, MemoryRow } from "@/types/app";

const GROUPS: { kind: MemoryKind; title: string }[] = [
  { kind: "open_question", title: "Open loops" },
  { kind: "decision", title: "Decisions" },
  { kind: "project", title: "Projects" },
  { kind: "preference", title: "Preferences" },
  { kind: "fact", title: "Facts" },
];

/** Plain, honest age: "3 months", "12 days". */
function age(iso: string | null): string {
  if (!iso) return "";
  const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);
  if (Number.isNaN(days)) return "";
  if (days < 1) return "today";
  if (days < 60) return `${days} day${days === 1 ? "" : "s"}`;
  return `${Math.floor(days / 30)} months`;
}

function Item({ item }: { item: MemoryRow }) {
  const setStatus = useSetMemoryStatus();
  const confirmed = item.status === "confirmed";
  return (
    <li className="group flex gap-2 border-b px-3 py-2">
      <div className="min-w-0 flex-1">
        <p className={cn("text-sm", confirmed && "text-emerald-300")}>{item.text}</p>
        <button
          type="button"
          className="mt-0.5 truncate text-left text-[11px] text-muted-foreground hover:underline"
          onClick={() => {
            useUiStore.getState().select({
              conversationId: item.conversationId,
              via: item.sourceOnMainPath ? null : item.sourceUuid,
              focus: item.sourceUuid,
            });
          }}
        >
          {item.conversationName || "Untitled"}
          {item.kind === "open_question" && item.sourceAt ? ` · open ${age(item.sourceAt)}` : ""}
        </button>
      </div>
      <div className="flex shrink-0 items-start gap-0.5">
        <button
          type="button"
          aria-label={confirmed ? "Confirmed. Click to undo" : "Confirm"}
          title={confirmed ? "Confirmed. Click to undo" : "Keep this"}
          disabled={setStatus.isPending}
          onClick={() => {
            setStatus.mutate({ id: item.id, status: confirmed ? "suggested" : "confirmed" });
          }}
          className={cn(
            "rounded-sm p-1 hover:bg-accent",
            confirmed ? "text-emerald-400" : "text-muted-foreground",
          )}
        >
          <Check className="size-3.5" />
        </button>
        <button
          type="button"
          aria-label="Dismiss"
          title="Dismiss. It will not be suggested again"
          disabled={setStatus.isPending}
          onClick={() => {
            setStatus.mutate({ id: item.id, status: "dismissed" });
          }}
          className="rounded-sm p-1 text-muted-foreground hover:bg-accent"
        >
          <X className="size-3.5" />
        </button>
      </div>
    </li>
  );
}

export function MemoryList() {
  const memory = useMemory();
  if (memory.isPending) return <div className="h-full" aria-busy />;
  if (memory.isError)
    return (
      <EmptyState
        icon={AlertTriangle}
        title="Could not load memory"
        description={memory.error.message}
      />
    );
  if (memory.data.length === 0)
    return (
      <EmptyState
        icon={Lightbulb}
        title="Nothing remembered yet"
        description="Decisions, open questions, projects and facts appear here after tagging, each linked to the messages it came from."
      />
    );
  return (
    <div>
      {GROUPS.map((g) => {
        const items = memory.data.filter((m) => m.kind === g.kind);
        if (items.length === 0) return null;
        return (
          <section key={g.kind}>
            <h3 className="sticky top-0 z-10 border-b bg-background px-3 py-1.5 text-xs font-semibold text-muted-foreground">
              {g.title} <span className="font-normal opacity-60">{items.length}</span>
            </h3>
            <ul>
              {items.map((i) => (
                <Item key={i.id} item={i} />
              ))}
            </ul>
          </section>
        );
      })}
    </div>
  );
}
