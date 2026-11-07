/** True when a key press belongs to a text field rather than to a shortcut. */
export function isTyping(e: KeyboardEvent): boolean {
  const t = e.target;
  return (
    t instanceof HTMLInputElement ||
    t instanceof HTMLTextAreaElement ||
    (t instanceof HTMLElement && t.isContentEditable)
  );
}

export function hasModifier(e: KeyboardEvent): boolean {
  return e.metaKey || e.ctrlKey || e.altKey;
}
