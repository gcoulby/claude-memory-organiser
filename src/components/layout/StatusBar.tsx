import { useEffect, useState } from "react";
import { commands } from "@/lib/commands";
import type { AppInfo } from "@/types/app";

export function StatusBar() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    commands
      .appInfo()
      .then(setInfo)
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : String(err));
      });
  }, []);

  return (
    <footer className="flex h-7 shrink-0 items-center gap-4 border-t px-3 text-xs text-muted-foreground">
      {/* Placeholder until settings exist: the mode must always be visible. */}
      <span className="rounded-sm bg-secondary px-1.5 py-0.5 text-secondary-foreground">
        Local only
      </span>
      <span className="flex-1">No jobs running</span>
      {error ? (
        <span className="text-destructive">Backend unavailable: {error}</span>
      ) : (
        <span>{info ? `v${info.version}` : "…"}</span>
      )}
    </footer>
  );
}
