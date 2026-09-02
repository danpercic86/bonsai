// §8.3 / §8.4: the Claude Code CLI health probe behind the Settings AI section
// and the one-time consent dialog that gates enabling AI. Extracted verbatim
// from App so the container only wires the dialog and forwards `aiAvailability`.
import { useCallback, useEffect, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { ipc } from '../ipc';
import type { AiAvailability, UiSettingsPatch } from '../ipc';

export interface UseAiAvailability {
  aiAvailability: AiAvailability | null;
  consentOpen: boolean;
  setConsentOpen: Dispatch<SetStateAction<boolean>>;
  handleConfirmConsent: () => void;
}

export function useAiAvailability(
  settingsOpen: boolean,
  activeRepo: string | null,
  handleSettingsChange: (patch: UiSettingsPatch) => void,
): UseAiAvailability {
  // CLI health probe result; null while probing. Re-fetched on Settings open and
  // on repo open (§8.3). A req-id guards against out-of-order probe resolutions.
  const [aiAvailability, setAiAvailability] = useState<AiAvailability | null>(null);
  const aiProbeIdRef = useRef(0);
  // Consent ConfirmDialog (opened by SettingsPanel's enable toggle when consent
  // has not yet been recorded).
  const [consentOpen, setConsentOpen] = useState(false);

  // P13 §8.3: probe the Claude Code CLI. Re-runnable; a req-id guards against a
  // stale probe overwriting a newer result. Never throws (the IPC never rejects
  // for CLI state) — a rejection just leaves the last-known availability.
  const probeAiAvailability = useCallback(() => {
    const id = ++aiProbeIdRef.current;
    void ipc
      .checkAiAvailability()
      .then((a) => {
        if (id === aiProbeIdRef.current) setAiAvailability(a);
      })
      .catch(() => {
        // Non-fatal — keep the last-known availability.
      });
  }, []);

  // Probe on Settings open (fresh status for the AI section) and whenever a repo
  // becomes active (§8.3). Cheap enough to re-run; the req-id dedupes races.
  useEffect(() => {
    if (settingsOpen) probeAiAvailability();
  }, [settingsOpen, probeAiAvailability]);
  useEffect(() => {
    if (activeRepo !== null) probeAiAvailability();
  }, [activeRepo, probeAiAvailability]);

  // Consent flow (§8.4): the Settings enable toggle defers here when consent has
  // not been recorded; confirming records BOTH enable + consent in one patch.
  const handleConfirmConsent = useCallback(() => {
    setConsentOpen(false);
    handleSettingsChange({ aiEnabled: true, aiConsented: true });
  }, [handleSettingsChange]);

  return { aiAvailability, consentOpen, setConsentOpen, handleConfirmConsent };
}
