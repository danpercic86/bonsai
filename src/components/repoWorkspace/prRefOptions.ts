// P78 / P100: Combobox option builders for the PR create form's Base + Compare
// ref fields. Extracted from RepoWorkspace so the container keeps shrinking.
//
// P100: every enabled row carries a short-oid hint — the same disambiguator the
// command palette puts on its branch actions (`paletteActions` → `shortOid(b.tip)`).
// It is also the only enabled-option combobox hint in the app, so it is what makes
// the `.combobox-option-hint` idle + active states reachable in the harness.

import type { BranchInfo, BranchesSnapshot, RemoteBranchInfo } from '../../ipc';
import type { ComboboxOption } from '../Combobox';
import { shortOid } from '../workspaceUtils';

function toOption(b: BranchInfo | RemoteBranchInfo): ComboboxOption {
  return { value: b.name, label: b.name, hint: shortOid(b.tip) };
}

/** Compare = local branches only. */
export function prCompareRefOptions(branches: BranchesSnapshot | null): ComboboxOption[] {
  return (branches?.local ?? []).map(toOption);
}

/** Base = local branches, then remote-tracking branches. */
export function prBaseRefOptions(branches: BranchesSnapshot | null): ComboboxOption[] {
  return [...(branches?.local ?? []).map(toOption), ...(branches?.remote ?? []).map(toOption)];
}
