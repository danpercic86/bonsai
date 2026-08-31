// P90: maps a CommitStatus's contexts to sorted CheckRows (§4.9 — problems first).
import type { CommitStatus } from '../../ipc';
import { sortContexts } from './checkVisuals';
import { CheckRow } from './CheckRow';

export interface ChecksListProps {
  status: CommitStatus;
  onOpen(url: string): void;
}

export function ChecksList({ status, onOpen }: ChecksListProps) {
  const rows = sortContexts(status.contexts);
  return (
    <ul className="checks-list" role="list">
      {rows.map((c) => (
        <CheckRow key={c.name} context={c} onOpen={onOpen} />
      ))}
    </ul>
  );
}
