interface Segment {
  text: string;
  marked: boolean;
}

/** Splits a snippet on the \u0002 and \u0003 markers Rust puts around matches. */
function segments(text: string): Segment[] {
  const out: Segment[] = [];
  let marked = false;
  let current = "";
  for (const ch of text) {
    if (ch === "\u0002" || ch === "\u0003") {
      if (current) out.push({ text: current, marked });
      current = "";
      marked = ch === "\u0002";
    } else {
      current += ch;
    }
  }
  if (current) out.push({ text: current, marked });
  return out;
}

/**
 * Renders a search snippet. Splitting on markers instead of rendering HTML
 * means chat text can never inject markup.
 */
export function Highlight({ text }: { text: string }) {
  return (
    <>
      {segments(text).map((s, i) =>
        s.marked ? (
          <mark key={i} className="rounded-xs bg-yellow-500/30 text-foreground">
            {s.text}
          </mark>
        ) : (
          <span key={i}>{s.text}</span>
        ),
      )}
    </>
  );
}
