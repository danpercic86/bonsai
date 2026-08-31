// P69g — UI §2.2 / ui-reference §12.1: a titled group of settings rows.
//
// `<section aria-labelledby>` (UI §7.1) so the group name is announced when focus
// enters it. The uppercase treatment is CSS only — the catalog stores group names
// in sentence case, and search result headers reuse the same string.

import type { ReactNode, Ref } from 'react';

import { useSettingsGroupVisible } from './SettingsSearchContext';

export function SettingsGroup({
  id,
  title,
  innerRef,
  children,
}: {
  /** Stable slug; the title element gets `{id}-title`. */
  id: string;
  /** MUST equal the catalog `group` value of every row inside. */
  title: string;
  /** P69h: the Git-config deep link scrolls the Identity group into view, so its
   *  section element has to be reachable. A wrapper `<div>` would break the
   *  `.settings-group + .settings-group` separator rule. */
  innerRef?: Ref<HTMLElement>;
  children: ReactNode;
}) {
  /* P69k: in a search result block a group whose rows all filtered out would
     render as a bare heading over nothing. */
  const visible = useSettingsGroupVisible(title);
  if (!visible) return null;

  return (
    <section className="settings-group" aria-labelledby={`${id}-title`} ref={innerRef}>
      <h4 className="settings-group-title" id={`${id}-title`}>
        {title}
      </h4>
      {children}
    </section>
  );
}
