import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef } from "react";
import { count, shortDate } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { ConversationRow } from "@/types/app";
import { StateDot } from "./StateDot";

const ROW_HEIGHT = 52;

interface ConversationListProps {
  rows: ConversationRow[];
  selectedId: number | null;
  onSelect: (id: number) => void;
}

export function ConversationList({ rows, selectedId, onSelect }: ConversationListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });

  const selectedIndex = rows.findIndex((r) => r.id === selectedId);
  useEffect(() => {
    if (selectedIndex >= 0) virtualizer.scrollToIndex(selectedIndex, { align: "auto" });
  }, [selectedIndex, virtualizer]);

  return (
    <div ref={scrollRef} className="h-full overflow-auto" role="listbox" aria-label="Chats">
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const row = rows[item.index];
          if (!row) return null;
          const selected = row.id === selectedId;
          return (
            <button
              key={row.id}
              type="button"
              role="option"
              aria-selected={selected}
              onClick={() => {
                onSelect(row.id);
              }}
              className={cn(
                "absolute top-0 left-0 flex w-full flex-col justify-center gap-0.5 border-b px-3 text-left",
                selected ? "bg-accent" : "hover:bg-accent/50",
              )}
              style={{ height: item.size, transform: `translateY(${item.start}px)` }}
            >
              <span className="flex items-center gap-2">
                <StateDot state={row.state} />
                <span className="min-w-0 flex-1 truncate text-sm">
                  {row.name || <span className="text-muted-foreground italic">Untitled</span>}
                </span>
              </span>
              <span className="flex gap-2 pl-4 text-xs text-muted-foreground">
                <span>{shortDate(row.updatedAt)}</span>
                <span>·</span>
                <span>
                  {count(row.messageCount)} {row.messageCount === 1 ? "message" : "messages"}
                </span>
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
