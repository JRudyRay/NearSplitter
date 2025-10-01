import { describe, expect, it } from 'vitest';

import { buildEqualShares, uniq } from '@/lib/utils/shares';

describe('shares utilities', () => {
  it('distributes equal shares and assigns remainder to last member', () => {
    const members = ['alice.testnet', 'bob.testnet', 'carol.testnet'];
    const shares = buildEqualShares(members);

    expect(shares).toHaveLength(3);
    expect(shares[0]).toEqual({ account_id: 'alice.testnet', weight_bps: 3333 });
    expect(shares[1]).toEqual({ account_id: 'bob.testnet', weight_bps: 3333 });
    expect(shares[2]).toEqual({ account_id: 'carol.testnet', weight_bps: 3334 });
    expect(shares.reduce((sum, share) => sum + share.weight_bps, 0)).toBe(10_000);
  });

  it('throws when no members provided', () => {
    expect(() => buildEqualShares([])).toThrow('At least one participant required');
  });

  it('deduplicates arrays of ids', () => {
    expect(uniq(['a', 'b', 'a', 'c', 'b'])).toEqual(['a', 'b', 'c']);
    expect(uniq([])).toEqual([]);
  });
});
