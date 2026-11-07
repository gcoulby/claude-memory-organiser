import { Brain, ChevronLeft, ChevronRight, FileText, Paperclip, Wrench } from "lucide-react";
import { dateTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { ReaderMessage } from "@/types/app";
import { Markdown } from "./Markdown";
import { messageDomId } from "./messageDomId";

interface MessageViewProps {
  message: ReaderMessage;
  focused: boolean;
  onSwitchBranch: (uuid: string) => void;
}

export function MessageView({ message, focused, onSwitchBranch }: MessageViewProps) {
  const human = message.sender === "human";
  const { siblings, siblingIndex } = message;
  const prev = siblings[siblingIndex - 1];
  const next = siblings[siblingIndex + 1];

  return (
    <article
      id={messageDomId(message.uuid)}
      className={cn(
        "scroll-mt-4 border-b px-5 py-4",
        human && "bg-muted/30",
        focused && "ring-2 ring-yellow-500/50 ring-inset",
      )}
    >
      <header className="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
        <span className="font-semibold text-foreground">{human ? "You" : "Claude"}</span>
        <span>{dateTime(message.createdAt)}</span>
        {siblings.length > 1 && (
          <span
            className="ml-auto flex items-center gap-0.5"
            title="This message was edited or retried. Each version starts its own branch."
          >
            <button
              type="button"
              aria-label="Previous version"
              disabled={!prev}
              className="rounded-sm p-0.5 hover:bg-accent disabled:opacity-30"
              onClick={() => {
                if (prev) onSwitchBranch(prev);
              }}
            >
              <ChevronLeft className="size-3.5" />
            </button>
            <span className="tabular-nums">
              {siblingIndex + 1} / {siblings.length}
            </span>
            <button
              type="button"
              aria-label="Next version"
              disabled={!next}
              className="rounded-sm p-0.5 hover:bg-accent disabled:opacity-30"
              onClick={() => {
                if (next) onSwitchBranch(next);
              }}
            >
              <ChevronRight className="size-3.5" />
            </button>
          </span>
        )}
      </header>

      {message.text ? (
        human ? (
          <p className="text-sm whitespace-pre-wrap">{message.text}</p>
        ) : (
          <Markdown text={message.text} />
        )
      ) : (
        <p className="text-sm text-muted-foreground italic">No text in this message.</p>
      )}

      {(message.attachments.length > 0 ||
        message.files.length > 0 ||
        message.tools.length > 0 ||
        message.hasThinking) && (
        <footer className="mt-3 flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
          {message.attachments.map((name, i) => (
            <span key={`a${i}`} className="flex items-center gap-1">
              <Paperclip className="size-3" />
              {name}
            </span>
          ))}
          {message.files.map((name, i) => (
            <span
              key={`f${i}`}
              className="flex items-center gap-1"
              title="File not included in the export"
            >
              <FileText className="size-3" />
              {name}
            </span>
          ))}
          {message.tools.length > 0 && (
            <span className="flex items-center gap-1">
              <Wrench className="size-3" />
              {message.tools
                .map((t) => (t.count > 1 ? `${t.name} ×${t.count}` : t.name))
                .join(", ")}
            </span>
          )}
          {message.hasThinking && (
            <span className="flex items-center gap-1">
              <Brain className="size-3" />
              Reasoning not shown
            </span>
          )}
        </footer>
      )}
    </article>
  );
}
