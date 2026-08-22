/**
 * P87b §2.1 / §5-3 — the in-flight phase readout. A small inline span reusing the
 * `.toolbar-job-status` treatment (11px `--text-2`) so the toolbar never reflows
 * mid-op. Clickable while an op runs → expands the git activity dock and reveals
 * the active run (reassurance-to-detail in one click).
 */
export interface ToolbarPhaseReadoutProps {
  /** The readout text (`phaseLabel` or `objectsReadout`). Empty ⇒ nothing renders. */
  phase: string | null;
  /** The fuller phase string for `title`. */
  title?: string;
  /** Expand the git dock + reveal the active run. When absent the readout is inert. */
  onShow?: () => void;
}

export function ToolbarPhaseReadout({ phase, title, onShow }: ToolbarPhaseReadoutProps) {
  if (phase === null || phase === '') return null;
  return (
    <button
      type="button"
      className="toolbar-job-status toolbar-phase"
      title={title ?? phase}
      aria-label="Show git activity"
      onClick={() => onShow?.()}
    >
      {phase}
    </button>
  );
}
