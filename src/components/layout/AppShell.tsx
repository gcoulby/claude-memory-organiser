import { Layers } from "lucide-react";
import { useEffect } from "react";
import { useDefaultLayout } from "react-resizable-panels";
import { InboxPane } from "@/components/inbox/InboxPane";
import { CommandPalette } from "@/components/palette/CommandPalette";
import { ReaderPane } from "@/components/reader/ReaderPane";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { useImportEvents } from "@/features/import/useImportEvents";
import { hasModifier, isTyping } from "@/lib/keys";
import { focusSearch } from "@/store/uiStore";
import { EmptyState } from "./EmptyState";
import { Pane } from "./Pane";
import { StatusBar } from "./StatusBar";

function useGlobalShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const findKey = e.key.toLowerCase() === "f" && (e.metaKey || e.ctrlKey);
      if ((e.key === "/" && !hasModifier(e) && !isTyping(e)) || findKey) {
        e.preventDefault();
        focusSearch();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);
}

export function AppShell() {
  useImportEvents();
  useGlobalShortcuts();
  // Pane sizes are a per-machine convenience, so browser storage is fine here.
  const layout = useDefaultLayout({ id: "main-panes", storage: localStorage });

  return (
    <div className="flex h-full flex-col">
      <main className="min-h-0 flex-1">
        <ResizablePanelGroup
          orientation="horizontal"
          defaultLayout={layout.defaultLayout}
          onLayoutChanged={layout.onLayoutChanged}
        >
          <ResizablePanel id="themes" defaultSize="20%" minSize="12%">
            <Pane title="Themes">
              <EmptyState
                icon={Layers}
                title="No themes yet"
                description="Themes arrive once tagging is built. Chats can already be triaged in the inbox."
              />
            </Pane>
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel id="inbox" defaultSize="30%" minSize="20%">
            <InboxPane />
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel id="reader" defaultSize="50%" minSize="25%">
            <ReaderPane />
          </ResizablePanel>
        </ResizablePanelGroup>
      </main>
      <StatusBar />
      <CommandPalette />
    </div>
  );
}
