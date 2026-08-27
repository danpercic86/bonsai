// Spec-003: the sidebar-wide membership lookup for graph solo/hide markers.
// Separate from the component file (ToastContext idiom) so
// react-refresh/only-export-components stays quiet.
import { createContext } from 'react';

export type RefFilterMark = 'solo' | 'hidden' | null;

/** Membership lookup for a FULL ref name. Default: nothing marked. */
export const RefFilterMarkerContext = createContext<(fullRef: string) => RefFilterMark>(
  () => null,
);
