import { AlertTriangle, ChevronDown, ChevronRight, Layers } from "lucide-react";
import { useState } from "react";
import { EmptyState } from "@/components/layout/EmptyState";
import { Button } from "@/components/ui/button";
import { usePreferences, useThemes, useThemeSegments } from "@/features/tagging/queries";
import { count, shortDate } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/store/uiStore";

function ThemeSegments({ themeId }: { themeId: number }) {
  const segments = useThemeSegments(themeId);
  const selection = useUiStore((s) => s.selection);
  if (segments.isPending) return <div className="h-6" aria-busy />;
  if (segments.isError)
    return <p className="px-6 py-1 text-xs text-destructive">{segments.error.message}</p>;
  return (
    <ul className="pb-1">
      {segments.data.map((s) => {
        const selected =
          selection?.conversationId === s.conversationId && selection.focus === s.startUuid;
        return (
          <li key={s.segmentId}>
            <button
              type="button"
              title={s.summary}
              onClick={() => {
                useUiStore.getState().select({
                  conversationId: s.conversationId,
                  via: null,
                  focus: s.startUuid,
                });
              }}
              className={cn(
                "flex w-full flex-col py-1 pr-3 pl-8 text-left",
                selected ? "bg-accent" : "hover:bg-accent/50",
              )}
            >
              <span className="truncate text-xs">{s.title}</span>
              <span className="truncate text-[11px] text-muted-foreground">
                {s.conversationName || "Untitled"} · {shortDate(s.conversationUpdatedAt)}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

export function ThemeList() {
  const themes = useThemes();
  const prefs = usePreferences();
  const [open, setOpen] = useState<number | null>(null);

  if (themes.isPending) return <div className="h-full" aria-busy />;
  if (themes.isError)
    return (
      <EmptyState
        icon={AlertTriangle}
        title="Could not load themes"
        description={themes.error.message}
      />
    );
  if (themes.data.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
        <Layers className="size-8 text-muted-foreground" aria-hidden />
        <p className="text-sm font-medium">No themes yet</p>
        <p className="max-w-64 text-xs text-muted-foreground">
          {prefs.data?.aiMode === "api"
            ? "Tag a sample of 20 chats to see how your history groups into themes."
            : "Themes come from tagging, which sends chat text to the Anthropic API. Turn on API mode in Settings to start."}
        </p>
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            useUiStore.getState().setSettingsOpen(true);
          }}
        >
          Open Settings
        </Button>
      </div>
    );
  }

  return (
    <ul className="py-1">
      {themes.data.map((t) => {
        const expanded = open === t.id;
        const Icon = expanded ? ChevronDown : ChevronRight;
        return (
          <li key={t.id}>
            <button
              type="button"
              aria-expanded={expanded}
              onClick={() => {
                setOpen(expanded ? null : t.id);
              }}
              className="flex w-full items-center gap-1.5 px-2 py-1.5 text-left hover:bg-accent/50"
            >
              <Icon className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate text-sm">{t.name}</span>
              <span
                className="shrink-0 text-xs text-muted-foreground tabular-nums"
                title={`${count(t.segments)} segments in ${count(t.conversations)} chats`}
              >
                {count(t.conversations)}
              </span>
            </button>
            {expanded && <ThemeSegments themeId={t.id} />}
          </li>
        );
      })}
    </ul>
  );
}
