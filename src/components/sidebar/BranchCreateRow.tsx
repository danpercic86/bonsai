// P118: the inline "create branch" input row, split out of BranchesSection for
// the same reason as SidebarActionButton — its `disabled` is the only other
// thing in that section that depends on the in-flight flag, so it reads
// `useSidebarBusy()` here and the section stops re-rendering on every flip.
// Markup, class names and key handling are byte-preserved from the inline
// version.
import type { Dispatch, SetStateAction } from 'react';
import { useSidebarBusy } from './sidebarBusyContext';

export interface BranchCreateRowProps {
  value: string;
  setValue: Dispatch<SetStateAction<string>>;
  error: string | null;
  setError: Dispatch<SetStateAction<string | null>>;
  close(): void;
  submit(): void | Promise<void>;
}

export function BranchCreateRow({
  value,
  setValue,
  error,
  setError,
  close,
  submit,
}: BranchCreateRowProps) {
  const busy = useSidebarBusy();
  return (
    <div className="branch-create-row">
      <input
        className="branch-create-input"
        type="text"
        placeholder="new-branch-name"
        autoFocus
        value={value}
        disabled={busy}
        onChange={(e) => {
          setValue(e.target.value);
          setError(null);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void submit();
          } else if (e.key === 'Escape') {
            close();
          }
        }}
        onBlur={() => {
          if (value.trim() === '') close();
        }}
      />
      {error !== null && (
        <div className="branch-create-error" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
