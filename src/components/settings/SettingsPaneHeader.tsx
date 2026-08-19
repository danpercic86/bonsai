// P69g — UI §2.2: the pane's title + subtitle block.
//
// The subtitle is where each category states its scope ONCE ("Applies to every
// repository"), which is what lets the rail carry a `repo` pill on exactly one
// item instead of decorating six.

import type { ReactNode } from 'react';

export function SettingsPaneHeader({
  title,
  subtitle,
  trailing,
}: {
  title: string;
  subtitle: string;
  /** Optional trailing slot — P69h puts the Git-config scope switch here. */
  trailing?: ReactNode;
}) {
  return (
    <header className="settings-pane-header">
      <div className="settings-pane-heading">
        <h3 className="settings-pane-title">{title}</h3>
        <p className="settings-pane-subtitle">{subtitle}</p>
      </div>
      {trailing}
    </header>
  );
}
