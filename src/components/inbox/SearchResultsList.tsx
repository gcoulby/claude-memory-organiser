import { useVirtualizer } from "@tanstack/react-virtual";
import { GitBranch, Paperclip } from "lucide-react";
import { useEffect, useRef } from "react";
import { shortDate } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { SearchHit } from "@/types/app";
import { Highlight } from "./Highlight";

const ROW_HEIGHT = 84;

interface SearchResultsListProps {
  hits: SearchHit[];
  selectedUuid: string | null;
  onSelect: (hit: SearchHit) => void;
}

export function SearchResultsList({ hits, selectedUuid, onSelect }: SearchResultsListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: hits.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
  });

  const selectedIndex = hits.findIndex((h) => h.messageUuid === selectedUuid);
  useEffect(() => {
    if (selectedIndex >= 0) virtualizer.scrollToIndex(selectedIndex, { align: "auto" });
  }, [selectedIndex, virtualizer]);

  return (
    <div
      ref={scrollRef}
      className="h-full overflow-auto"
      role="listbox"
      aria-label="Search results"
    >
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const hit = hits[item.index];
          if (!hit) return null;
          const selected = hit.messageUuid === selectedUuid;
          return (
            <button
              key={hit.messageId}
              type="button"
              role="option"
              aria-selected={selected}
              onClick={() => {
                onSelect(hit);
              }}
              className={cn(
                "absolute top-0 left-0 flex w-full flex-col gap-1 border-b px-3 py-2 text-left",
                selected ? "bg-accent" : "hover:bg-accent/50",
              )}
              style={{ height: item.size, transform: `translateY(${item.start}px)` }}
            >
              <span className="flex items-center gap-2 text-xs text-muted-foreground">
                <span className="min-w-0 flex-1 truncate font-medium text-foreground">
                  {hit.conversationName || "Untitled"}
                </span>
                {hit.inAttachment && <Paperclip className="size-3" aria-label="In an attachment" />}
                {!hit.onMainPath && <GitBranch className="size-3" aria-label="On another branch" />}
                <span>{hit.sender === "human" ? "You" : "Claude"}</span>
                <span>{shortDate(hit.createdAt)}</span>
              </span>
              <span className="line-clamp-2 text-xs text-muted-foreground">
                <Highlight text={hit.snippet} />
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
