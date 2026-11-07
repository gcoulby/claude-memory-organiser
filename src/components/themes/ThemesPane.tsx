import { useState } from "react";
import { Pane } from "@/components/layout/Pane";
import { MemoryList } from "@/components/memory/MemoryList";
import { cn } from "@/lib/utils";
import { ThemeList } from "./ThemeList";

type Tab = "themes" | "memory";

export function ThemesPane() {
  const [tab, setTab] = useState<Tab>("themes");
  const tabButton = (value: Tab, label: string) => (
    <button
      type="button"
      role="tab"
      aria-selected={tab === value}
      onClick={() => {
        setTab(value);
      }}
      className={cn(
        "rounded-sm px-2 py-0.5 text-xs",
        tab === value
          ? "bg-secondary text-secondary-foreground"
          : "text-muted-foreground hover:bg-accent",
      )}
    >
      {label}
    </button>
  );
  return (
    <Pane
      title="Organise"
      actions={
        <div className="flex gap-1" role="tablist">
          {tabButton("themes", "Themes")}
          {tabButton("memory", "Memory")}
        </div>
      }
    >
      {tab === "themes" ? <ThemeList /> : <MemoryList />}
    </Pane>
  );
}
