import { cn } from "@/lib/utils";
import type { ChatState } from "@/types/app";

const styles: Record<ChatState, string> = {
  untriaged: "bg-sky-500",
  organised: "bg-emerald-500",
  archived: "bg-muted-foreground/40",
};

export function StateDot({ state }: { state: ChatState }) {
  return (
    <span
      className={cn("inline-block size-2 shrink-0 rounded-full", styles[state])}
      title={state.charAt(0).toUpperCase() + state.slice(1)}
    />
  );
}
