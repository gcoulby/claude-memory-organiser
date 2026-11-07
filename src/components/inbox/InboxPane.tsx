import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, Inbox, Upload } from "lucide-react";
import { EmptyState } from "@/components/layout/EmptyState";
import { Pane } from "@/components/layout/Pane";
import { Button } from "@/components/ui/button";
import { useStartImport } from "@/features/import/useStartImport";
import { commands } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import { useImportStore } from "@/store/importStore";

export function InboxPane() {
  const stats = useQuery({ queryKey: queryKeys.libraryStats, queryFn: commands.libraryStats });
  const startImport = useStartImport();
  const running = useImportStore((s) => s.state.phase === "running");

  const importButton = (
    <Button
      size="sm"
      variant="outline"
      className="h-7"
      disabled={running || startImport.isPending}
      onClick={() => {
        startImport.mutate();
      }}
    >
      <Upload className="size-3.5" />
      Import
    </Button>
  );

  let body;
  if (stats.isPending) {
    body = <div className="h-full" aria-busy />;
  } else if (stats.isError) {
    body = (
      <EmptyState
        icon={AlertTriangle}
        title="Could not load chats"
        description={stats.error.message}
      />
    );
  } else if (stats.data.conversations === 0) {
    body = (
      <EmptyState
        icon={Inbox}
        title="No chats imported"
        description="Choose Import and select every zip from your Claude export, or its conversations.json."
      />
    );
  } else {
    // The chat list itself arrives in the next build step.
    body = (
      <EmptyState
        icon={Inbox}
        title={`${stats.data.conversations.toLocaleString("en-GB")} chats imported`}
        description={`${stats.data.messages.toLocaleString("en-GB")} messages. The chat list is not built yet.`}
      />
    );
  }

  return (
    <Pane title="Inbox" actions={importButton}>
      {body}
    </Pane>
  );
}
