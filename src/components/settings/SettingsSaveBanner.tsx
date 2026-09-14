// P113 §17.3 — the Settings card's save-failure banner.
//
// The one outcome in the P113 sweep that is NOT a row note, and §4.4's
// disambiguation rule is why:
//
//   1. **The failure is global, not per-row.** The write is of the whole
//      snapshot; attaching it to the row the user last touched would name a
//      smaller target than the failure covers.
//   2. **It is a standing state, not a transient outcome.** After a failed write
//      the UI shows values that are not on disk, and that stays true until a
//      write succeeds. Standing state → bordered banner, not a barred note.
//   3. **It outlives the category.** The user can move to another category, or
//      close and reopen Settings, while the condition persists — so it spans the
//      content column and shows on every page.
//   4. **It is `--danger`, not `--warning`.** Unlike every other error in this
//      contract, work IS at risk: the change is in memory only and dies with the
//      process. That is exactly the line §4.1 draws, and it is why this composes
//      the shared `.error-banner` recipe rather than the note recipes.
//
// ALWAYS MOUNTED, empty when idle: `role="alert"` fires on a text change inside
// an existing live region, and `.settings-save-banner:empty` collapses the
// chrome to a rendered height of 0 so the card's idle geometry is unchanged.
//
// P113 A3: "always mounted empty" is a property of THIS component, not of the
// sequence. Rendering `failed` straight from the prop held it only while
// Settings was open at failure time; the zero-announcement sequence is: the
// write fails while Settings is OPEN (banner path, no toast) → close → reopen →
// `SettingsShell` mounts and the text arrives in the SAME commit, which §8.2 /
// §12.3.4 says is not reliably announced. Mirroring the prop through an effect
// makes mount and text two commits in every entry path. The side effect —
// reopening Settings while the condition stands RE-announces it — is desirable:
// the values are still not on disk.

import { useEffect, useState } from 'react';

import { SETTINGS_SAVE_FAILURE_TEXT } from '../../hooks/useSettingsSaveFailure';

export function SettingsSaveBanner({
  failed,
  onRetry,
}: {
  /** Mirrors `useUiSettings`'s failure streak: true from the first failed write
   *  until one succeeds. NOT reset by a fresh user change — the values are still
   *  not on disk at that moment. */
  failed: boolean;
  /** `armSettingsSave(0)`. Has a real job: the bounded backoff is 300/600/1200 ms
   *  and then STOPS, after which the pending patch sits unsent until the user
   *  happens to change something else. Without this the banner is a dead end for
   *  anyone who has stopped changing settings — exactly the state a persistent
   *  failure produces. */
  onRetry(): void;
}) {
  /** The prop, one commit later (see the A3 note above). Initial `false`
   *  unconditionally: that is what makes the FIRST paint of a banner mounting
   *  into a standing failure empty. */
  const [show, setShow] = useState(false);
  useEffect(() => {
    setShow(failed);
  }, [failed]);

  return (
    <div className="error-banner settings-save-banner" role="alert" data-settings-save-banner="">
      {show && (
        <>
          <span className="error-banner-text">{SETTINGS_SAVE_FAILURE_TEXT}</span>
          <button type="button" className="section-action" onClick={onRetry}>
            Retry
          </button>
        </>
      )}
    </div>
  );
}
