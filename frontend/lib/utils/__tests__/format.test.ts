import { describe, expect, it } from 'vitest';

import { formatNearAmount, parseNearAmount, shortenAccountId } from '@/lib/utils/format';

describe('format utilities', () => {
  it('formats yoctoNEAR amounts into human readable NEAR', () => {
    const oneNearYocto = '1000000000000000000000000';
    expect(formatNearAmount(oneNearYocto, 2)).toBe('1.00');
    expect(formatNearAmount(oneNearYocto, 3)).toBe('1.000');
  });

  it('parses user input into yoctoNEAR', () => {
    expect(parseNearAmount('1')).toBe('1000000000000000000000000');
    expect(parseNearAmount('0.5')).toBe('500000000000000000000000');
  });

  it('throws for invalid NEAR amounts', () => {
    expect(() => parseNearAmount('not-a-number')).toThrowError(/Invalid NEAR amount/);
  });

  it('shortens account ids while preserving start and end', () => {
  expect(shortenAccountId('averylongaccount.testnet', 4)).toBe('aver…tnet');
  expect(shortenAccountId('short.near', 4)).toBe('shor…near');
  });
});
