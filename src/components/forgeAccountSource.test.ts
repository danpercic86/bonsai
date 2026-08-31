/** P80 §0.1 — the AccountSource display vocabulary. Pure map; the single source
 *  of truth for the switcher caption + tooltip so the microcopy can't drift. */
import { describe, it, expect } from 'vitest';

import { accountSourceCaption, accountSourceTooltip } from './forgeAccountSource';

describe('forgeAccountSource', () => {
  it('maps each source to its caption', () => {
    expect(accountSourceCaption('override')).toBe('Pinned to this repo');
    expect(accountSourceCaption('ownerMatch')).toBe('Matched by owner');
    expect(accountSourceCaption('hostDefault')).toBe('Host default');
    expect(accountSourceCaption('single')).toBeNull();
    expect(accountSourceCaption('none')).toBeNull();
  });

  it('maps each source to its tooltip (null where there is no caption)', () => {
    expect(accountSourceTooltip('override')).toMatch(/Pinned to this repository/);
    expect(accountSourceTooltip('ownerMatch')).toMatch(/username matches/);
    expect(accountSourceTooltip('hostDefault')).toBe('The default account for this host.');
    expect(accountSourceTooltip('single')).toBeNull();
    expect(accountSourceTooltip('none')).toBeNull();
  });
});
