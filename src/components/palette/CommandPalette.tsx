import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  CheckCircle2,
  Inbox,
  Layers,
  Lock,
  MessageSquare,
  Search,
  Settings,
  Upload,
} from "lucide-react";
import { useEffect } from "react";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { useConversations } from "@/features/chats/queries";
import { useStartImport } from "@/features/import/useStartImport";
import { commands } from "@/lib/commands";
import { queryKeys } from "@/lib/queryClient";
import { useJobRunning } from "@/store/jobs";
import { focusSearch, useUiStore, type InboxFilter } from "@/store/uiStore";

const FILTERS: { value: InboxFilter; label: string; icon: typeof Inbox }[] = [
  { value: "untriaged", label: "Show untriaged chats", icon: Inbox },
  { value: "organised", label: "Show organised chats", icon: CheckCircle2 },
  { value: "archived", label: "Show archived chats", icon: Archive },
  { value: "all", label: "Show all chats", icon: Layers },
];

export function CommandPalette() {
  const open = useUiStore((s) => s.paletteOpen);
  const setOpen = useUiStore((s) => s.setPaletteOpen);
  const queryClient = useQueryClient();
  const chats = useConversations("all");
  const startImport = useStartImport();
  const importing = useJobRunning();
  const lock = useMutation({
    mutationFn: commands.vaultLock,
    onSuccess: () => {
      queryClient.removeQueries({
        predicate: (q) => q.queryKey[0] !== queryKeys.vaultStatus[0],
      });
      void queryClient.invalidateQueries({ queryKey: queryKeys.vaultStatus });
    },
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen(!useUiStore.getState().paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [setOpen]);

  const run = (fn: () => void) => () => {
    setOpen(false);
    fn();
  };

  return (
    <CommandDialog
      open={open}
      onOpenChange={setOpen}
      title="Command palette"
      description="Run a command or jump to a chat"
    >
      <CommandInput placeholder="Type a command or chat name…" />
      <CommandList>
        <CommandEmpty>Nothing matches.</CommandEmpty>
        <CommandGroup heading="Actions">
          <CommandItem
            onSelect={run(() => {
              // Wait for the dialog to close, or it takes focus back.
              setTimeout(focusSearch, 0);
            })}
          >
            <Search /> Search messages
          </CommandItem>
          <CommandItem
            disabled={importing}
            onSelect={run(() => {
              startImport.mutate();
            })}
          >
            <Upload /> Import an export
          </CommandItem>
          {FILTERS.map((f) => (
            <CommandItem
              key={f.value}
              onSelect={run(() => {
                const ui = useUiStore.getState();
                ui.setQuery("");
                ui.setFilter(f.value);
              })}
            >
              <f.icon /> {f.label}
            </CommandItem>
          ))}
          <CommandItem
            onSelect={run(() => {
              useUiStore.getState().setSettingsOpen(true);
            })}
          >
            <Settings /> Settings and updates
          </CommandItem>
          <CommandItem
            disabled={importing}
            onSelect={run(() => {
              lock.mutate();
            })}
          >
            <Lock /> Lock
          </CommandItem>
        </CommandGroup>
        {chats.data && chats.data.length > 0 && (
          <CommandGroup heading="Chats">
            {chats.data.map((c) => (
              <CommandItem
                key={c.id}
                value={`${c.name || "Untitled"} #${c.id}`}
                onSelect={run(() => {
                  useUiStore.getState().select({ conversationId: c.id, via: null, focus: null });
                })}
              >
                <MessageSquare /> <span className="truncate">{c.name || "Untitled"}</span>
              </CommandItem>
            ))}
          </CommandGroup>
        )}
      </CommandList>
    </CommandDialog>
  );
}
