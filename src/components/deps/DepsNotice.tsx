// P125-ui §6 — the single state-notice slot (stale / failure-with-report) and
// the link note beneath it. Warning-bar recipe, `role="status"`, never
// `--danger`, never a modal; the link note has no × and no timer.
import { CHECK_NOW, failureDetails } from './depsCopy';
import type { DepsNoticeState } from './depsView';
import { noticeText } from './depsView';

export interface DepsNoticeProps {
  notice: DepsNoticeState;
  linkNote: string | null;
  nowMs: number;
  running: boolean;
  onCheckNow(): void;
}

export function DepsNotice({ notice, linkNote, nowMs, running, onCheckNow }: DepsNoticeProps) {
  if (notice === null && linkNote === null) return null;
  return (
    <div className="deps-notices">
      {notice !== null && (
        <div
          className="deps-notice"
          role="status"
          title={notice.kind === 'failure' ? failureDetails(notice.failure.message) : undefined}
        >
          <span className="deps-notice-text">{noticeText(notice, nowMs)}</span>
          <button type="button" className="deps-link deps-notice-action" disabled={running} onClick={onCheckNow}>
            {CHECK_NOW}
          </button>
        </div>
      )}
      {linkNote !== null && (
        <div className="deps-notice" role="status">
          <span className="deps-notice-text">{linkNote}</span>
        </div>
      )}
    </div>
  );
}
