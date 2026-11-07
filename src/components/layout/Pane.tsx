import type { ReactNode } from "react";

interface PaneProps {
  title: string;
  children: ReactNode;
}

export function Pane({ title, children }: PaneProps) {
  return (
    <section className="flex h-full min-w-0 flex-col" aria-label={title}>
      <header className="flex h-10 shrink-0 items-center border-b px-3">
        <h2 className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
          {title}
        </h2>
      </header>
      <div className="min-h-0 flex-1 overflow-auto">{children}</div>
    </section>
  );
}
