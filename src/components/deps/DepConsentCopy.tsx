// P125-ui §8.1 — the consent paragraphs. The SINGLE source for both consent
// routes (the dialog's off body and the Settings `DepConsentDialog`), so the
// disclosure can never differ between them.
import { CONSENT_FACTS, CONSENT_P1, consentSchedule } from './depsCopy';

export function DepConsentCopy({ intervalHours }: { intervalHours: number }) {
  return (
    <div className="deps-consent">
      <p className="dialog-body">{CONSENT_P1}</p>
      {CONSENT_FACTS.map((f) => (
        <p key={f.lead} className="dialog-body-detail">
          <strong>{f.lead}</strong> {f.rest}
        </p>
      ))}
      <p className="dialog-body-detail">{consentSchedule(intervalHours)}</p>
    </div>
  );
}
