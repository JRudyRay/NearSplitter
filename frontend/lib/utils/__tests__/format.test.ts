import { describe, expect, it } from 'vitest';

import { formatNearAmount, formatBalanceWithSign, parseNearAmount, shortenAccountId } from '@/lib/utils/format';

describe('format utilities', () => {
  it('formats yoctoNEAR amounts into human readable NEAR', () => {
    const oneNearYocto = '1000000000000000000000000';
    expect(formatNearAmount(oneNearYocto, 2)).toBe('1.00');
    expect(formatNearAmount(oneNearYocto, 3)).toBe('1.000');
  });

  // UPDATED: Test negative I128 amounts (from balance calculations)
  it('formats negative yoctoNEAR amounts (I128 debtor balances)', () => {
    const negativeOneNear = '-1000000000000000000000000';
    expect(formatNearAmount(negativeOneNear, 2)).toBe('-1.00');
    
    const negativeHalfNear = '-500000000000000000000000';
    expect(formatNearAmount(negativeHalfNear, 2)).toBe('-0.50');
  });

  it('handles zero amounts gracefully', () => {
    expect(formatNearAmount('0', 2)).toBe('0.00');
    expect(formatNearAmount(0, 2)).toBe('0.00');
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

  // UPDATED: Tests for formatBalanceWithSign helper
  describe('formatBalanceWithSign', () => {
    it('identifies positive (creditor) balances', () => {
      const result = formatBalanceWithSign('1000000000000000000000000', 2);
      expect(result.value).toBe('1.00');
      expect(result.isPositive).toBe(true);
      expect(result.isNegative).toBe(false);
      expect(result.isZero).toBe(false);
    });

    it('identifies negative (debtor) balances', () => {
      const result = formatBalanceWithSign('-1000000000000000000000000', 2);
      expect(result.value).toBe('1.00'); // Absolute value
      expect(result.isPositive).toBe(false);
      expect(result.isNegative).toBe(true);
      expect(result.isZero).toBe(false);
    });

    it('identifies zero balances', () => {
      const result = formatBalanceWithSign('0', 2);
      expect(result.value).toBe('0.00');
      expect(result.isPositive).toBe(false);
      expect(result.isNegative).toBe(false);
      expect(result.isZero).toBe(true);
    });
  });
});
