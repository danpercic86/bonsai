// P84 reveal-in-graph a11y announcer (UI contract §6). An always-mounted,
// visually-hidden `role="status"` live region that the container updates on every
// sidebar reveal. It must stay permanently mounted (not conditionally rendered)
// so screen readers reliably pick up the text change.
//
// Copy (UI contract §6):
//  - in-layout: `Revealed <ref> at commit <short-oid>`
//  - miss:      `<ref> is not in the loaded history`
//
// The visually-hidden recipe comes from the shared `.sr-only` utility
// (`styles/tokens-and-base.css`) — do NOT inline the clip recipe.

export interface RevealAnnouncerProps {
  /** The message to announce; empty string renders nothing (initial state). */
  message: string;
}

/** Formats the in-layout announcement: `Revealed <ref> at commit <short-oid>`. */
export function revealedMessage(ref: string, oid: string): string {
  return `Revealed ${ref} at commit ${oid.slice(0, 7)}`;
}

/** Formats the miss announcement: `<ref> is not in the loaded history`. */
export function revealMissMessage(ref: string): string {
  return `${ref} is not in the loaded history`;
}

export function RevealAnnouncer({ message }: RevealAnnouncerProps) {
  return (
    <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
      {message}
    </span>
  );
}
