import type { ReactNode } from "react";

interface PaneProps {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}

export function Pane({ title, actions, children }: PaneProps) {
  return (
    <section className="flex h-full min-w-0 flex-col" aria-label={title}>
      <header className="flex h-10 shrink-0 items-center justify-between gap-2 border-b px-3">
        <h2 className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
          {title}
        </h2>
        {actions}
      </header>
      <div className="min-h-0 flex-1 overflow-auto">{children}</div>
    </section>
  );
}
