import { openUrl } from "@tauri-apps/plugin-opener";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

const SAFE_SCHEMES = ["http:", "https:", "mailto:"];

function openExternally(href: string | undefined) {
  if (!href) return;
  try {
    if (SAFE_SCHEMES.includes(new URL(href).protocol)) void openUrl(href);
  } catch {
    // Not a valid absolute URL: nothing sensible to open.
  }
}

const components: Components = {
  // Links open in the browser; navigating the app window would leave the app.
  a: ({ href, children }) => (
    <a
      href={href}
      title={href}
      onClick={(e) => {
        e.preventDefault();
        openExternally(href);
      }}
    >
      {children}
    </a>
  ),
  // Remote images are never fetched: that would leak activity to third parties.
  img: ({ alt, src }) => (
    <span className="text-muted-foreground italic" title={typeof src === "string" ? src : ""}>
      [image{alt ? `: ${alt}` : ""}]
    </span>
  ),
};

export function Markdown({ text }: { text: string }) {
  return (
    <div className="prose prose-sm max-w-none dark:prose-invert prose-pre:bg-muted prose-pre:text-foreground">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {text}
      </ReactMarkdown>
    </div>
  );
}
