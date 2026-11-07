/** DOM id for a message in the reader, used to scroll to search hits. */
export function messageDomId(uuid: string): string {
  return `msg-${uuid}`;
}
