import { AlertTriangle, BookOpen, GitBranch } from "lucide-react";
import { useEffect } from "react";
import { EmptyState } from "@/components/layout/EmptyState";
import { Pane } from "@/components/layout/Pane";
import { Button } from "@/components/ui/button";
import { useConversation, useSetState } from "@/features/chats/queries";
import { dateTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/store/uiStore";
import type { ChatState } from "@/types/app";
import { MessageView } from "./MessageView";
import { messageDomId } from "./messageDomId";

const STATES: { value: ChatState; label: string; key: string }[] = [
  { value: "untriaged", label: "Untriaged", key: "U" },
  { value: "organised", label: "Organised", key: "O" },
  { value: "archived", label: "Archived", key: "E" },
];

export function ReaderPane() {
  const selection = useUiStore((s) => s.selection);
  const select = useUiStore((s) => s.select);
  const detail = useConversation(selection?.conversationId ?? null, selection?.via ?? null);
  const setState = useSetState();

  const focus = selection?.focus ?? null;
  const loadedId = detail.data?.id;
  useEffect(() => {
    if (loadedId === undefined) return;
    if (focus) {
      document.getElementById(messageDomId(focus))?.scrollIntoView({ block: "start" });
    } else {
      document.getElementById("reader-top")?.scrollIntoView({ block: "start" });
    }
  }, [loadedId, focus]);

  if (!selection) {
    return (
      <Pane title="Reader">
        <EmptyState
          icon={BookOpen}
          title="Nothing selected"
          description="Choose a chat from the inbox to read it. Use J and K to move through the list."
        />
      </Pane>
    );
  }
  if (detail.isError) {
    return (
      <Pane title="Reader">
        <EmptyState
          icon={AlertTriangle}
          title="Could not open the chat"
          description={detail.error.message}
        />
      </Pane>
    );
  }
  if (detail.isPending) {
    return (
      <Pane title="Reader">
        <div className="h-full" aria-busy />
      </Pane>
    );
  }

  const chat = detail.data;
  const switchBranch = (uuid: string) => {
    select({ conversationId: chat.id, via: uuid, focus: uuid });
  };

  return (
    <Pane title="Reader">
      <div id="reader-top" />
      <header className="border-b px-5 py-4">
        <h1 className="text-base font-semibold">{chat.name || "Untitled"}</h1>
        <p className="mt-1 text-xs text-muted-foreground">
          Started {dateTime(chat.createdAt)} · Last updated {dateTime(chat.updatedAt)}
        </p>
        <div className="mt-3 flex gap-1" role="radiogroup" aria-label="State">
          {STATES.map((s) => (
            <button
              key={s.value}
              type="button"
              role="radio"
              aria-checked={chat.state === s.value}
              title={`Shortcut: ${s.key}`}
              disabled={setState.isPending}
              onClick={() => {
                setState.mutate({ ids: [chat.id], state: s.value });
              }}
              className={cn(
                "rounded-sm border px-2 py-0.5 text-xs",
                chat.state === s.value
                  ? "border-transparent bg-secondary text-secondary-foreground"
                  : "text-muted-foreground hover:bg-accent",
              )}
            >
              {s.label}
            </button>
          ))}
        </div>
        {setState.isError && (
          <p className="mt-2 text-xs text-destructive">{setState.error.message}</p>
        )}
      </header>

      {chat.offMainPath && (
        <div className="flex items-center gap-2 border-b bg-muted/40 px-5 py-2 text-xs text-muted-foreground">
          <GitBranch className="size-3.5" />
          <span className="flex-1">You are reading an earlier branch of this chat.</span>
          <Button
            size="sm"
            variant="ghost"
            className="h-6 text-xs"
            onClick={() => {
              select({ conversationId: chat.id, via: null, focus: null });
            }}
          >
            Back to the latest branch
          </Button>
        </div>
      )}

      {chat.messages.length === 0 ? (
        <EmptyState
          icon={BookOpen}
          title="No messages"
          description="This chat is empty in the export."
        />
      ) : (
        chat.messages.map((m) => (
          <MessageView
            key={m.uuid}
            message={m}
            focused={m.uuid === focus}
            onSwitchBranch={switchBranch}
          />
        ))
      )}
    </Pane>
  );
}
