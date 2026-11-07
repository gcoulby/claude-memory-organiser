import { useImportStore } from "./importStore";
import { useTaggingStore } from "./taggingStore";

/** True while any background job is writing: import or tagging. */
export function useJobRunning(): boolean {
  const importing = useImportStore((s) => s.state.phase === "running");
  const tagging = useTaggingStore((s) => s.state.phase === "running");
  return importing || tagging;
}
