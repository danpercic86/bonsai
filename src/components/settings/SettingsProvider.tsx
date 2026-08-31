// P69f §2.2 — the Settings provider component.
//
// Split from `SettingsContext.ts` so that module can export the two hooks
// without tripping `react-refresh/only-export-components` (the `ToastContext.ts`
// idiom). Nesting the two providers here is the whole implementation: the memo
// boundary itself lives in `useSettingsPanelAdapter`.

import type { ReactNode } from 'react';

import {
  SettingsActionsContext,
  SettingsValuesContext,
  type SettingsActions,
  type SettingsValues,
} from './SettingsContext';

export function SettingsProvider({
  values,
  actions,
  children,
}: {
  values: SettingsValues;
  actions: SettingsActions;
  children: ReactNode;
}) {
  return (
    <SettingsValuesContext.Provider value={values}>
      <SettingsActionsContext.Provider value={actions}>{children}</SettingsActionsContext.Provider>
    </SettingsValuesContext.Provider>
  );
}
