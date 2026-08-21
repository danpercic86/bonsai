// P79 §3.2 — the "Accounts" category page. A thin wrapper (mirrors
// IdentitiesCategory): the section is repo-independent, so it reads nothing from
// SettingsContext.
import { SettingsAccountsSection } from '../SettingsAccountsSection';

export function AccountsCategory() {
  return <SettingsAccountsSection />;
}
