import type { SegmentInfo } from "@/types/app";

/** Marks where a topic segment starts in the reader. */
export function SegmentHeader({
  segment,
  index,
  total,
}: {
  segment: SegmentInfo;
  index: number;
  total: number;
}) {
  return (
    <div className="border-y border-sky-500/20 bg-sky-500/5 px-5 py-2">
      <p className="flex items-baseline gap-2 text-xs">
        {total > 1 && (
          <span className="text-muted-foreground tabular-nums">
            {index + 1}/{total}
          </span>
        )}
        <span className="font-semibold">{segment.title}</span>
        <span className="text-muted-foreground">{segment.intent}</span>
      </p>
      {segment.summary && <p className="mt-0.5 text-xs text-muted-foreground">{segment.summary}</p>}
      {(segment.themes.length > 0 || segment.entities.length > 0) && (
        <p className="mt-1 flex flex-wrap gap-1">
          {segment.themes.map((t) => (
            <span
              key={t}
              className="rounded-sm bg-sky-500/15 px-1.5 py-0.5 text-[11px] text-sky-200"
            >
              {t}
            </span>
          ))}
          {segment.entities.map((e) => (
            <span
              key={e}
              className="rounded-sm bg-secondary px-1.5 py-0.5 text-[11px] text-muted-foreground"
            >
              {e}
            </span>
          ))}
        </p>
      )}
    </div>
  );
}
