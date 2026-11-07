import { BookOpen, Inbox, Layers } from "lucide-react";
import { useDefaultLayout } from "react-resizable-panels";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { EmptyState } from "./EmptyState";
import { Pane } from "./Pane";
import { StatusBar } from "./StatusBar";

export function AppShell() {
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
          <ResizablePanel id="themes" defaultSize="25%" minSize="15%">
            <Pane title="Themes">
              <EmptyState
                icon={Layers}
                title="No themes yet"
                description="Themes appear once chats have been imported and organised."
              />
            </Pane>
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel id="inbox" defaultSize="30%" minSize="20%">
            <Pane title="Inbox">
              <EmptyState
                icon={Inbox}
                title="No chats imported"
                description="Import a Claude data export to see your chats here."
              />
            </Pane>
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel id="reader" defaultSize="45%" minSize="25%">
            <Pane title="Reader">
              <EmptyState
                icon={BookOpen}
                title="Nothing selected"
                description="Choose a chat from the inbox to read it."
              />
            </Pane>
          </ResizablePanel>
        </ResizablePanelGroup>
      </main>
      <StatusBar />
    </div>
  );
}
