/**
 * RPC error decoding tests - validates user-friendly error message generation
 * UPDATED: Added tests for decodeNearError and isNotFoundError utilities
 */
import { describe, expect, it } from 'vitest';

import { decodeNearError, isNotFoundError } from '@/lib/near/rpc';

describe('RPC error utilities', () => {
  describe('decodeNearError', () => {
    it('extracts panic messages from Smart contract panicked format', () => {
      const error = new Error('Smart contract panicked: Circle not found');
      const result = decodeNearError(error);
      // Should match the ERROR_PATTERNS entry for "Circle not found"
      expect(result).toContain('Circle not found');
    });

    it('maps known error patterns to user-friendly messages', () => {
      // Already a member
      expect(decodeNearError('Already a member')).toBe('You are already a member of this circle.');
      
      // Invalid invite code
      expect(decodeNearError('Invalid invite code')).toBe('The invite code is incorrect.');
      
      // Owner only action
      expect(decodeNearError('Only owner can do this')).toContain('Only the circle owner');
    });

    it('handles storage deposit errors', () => {
      const error = new Error('Account must call storage_deposit first');
      expect(decodeNearError(error)).toContain('storage deposit');
    });

    it('handles settlement state errors', () => {
      const error = new Error('Circle is locked for settlement');
      expect(decodeNearError(error)).toContain('settlement');
    });

    it('handles deposit amount errors', () => {
      const error = new Error('Must deposit at least 10000000000000000000000');
      expect(decodeNearError(error)).toContain('Insufficient deposit');
    });

    it('returns raw message for unknown error patterns', () => {
      const error = new Error('Some completely unknown error format');
      expect(decodeNearError(error)).toBe('Some completely unknown error format');
    });

    it('handles null/undefined gracefully', () => {
      expect(decodeNearError(null)).toBe('Unknown error');
      expect(decodeNearError(undefined)).toBe('Unknown error');
    });

    it('handles non-Error objects', () => {
      expect(typeof decodeNearError('string error')).toBe('string');
      expect(typeof decodeNearError({ message: 'object error' })).toBe('string');
    });
  });

  describe('isNotFoundError', () => {
    it('detects "not found" in error messages', () => {
      expect(isNotFoundError(new Error('Circle not found'))).toBe(true);
      expect(isNotFoundError(new Error('Account not found'))).toBe(true);
      expect(isNotFoundError(new Error('Resource NOT FOUND'))).toBe(true);
    });

    it('detects "does not exist" patterns', () => {
      expect(isNotFoundError(new Error('Account does not exist'))).toBe(true);
      expect(isNotFoundError(new Error('Circle id-123 does not exist'))).toBe(true);
    });

    it('returns false for unrelated errors', () => {
      expect(isNotFoundError(new Error('Permission denied'))).toBe(false);
      expect(isNotFoundError(new Error('Invalid amount'))).toBe(false);
    });

    it('handles string errors', () => {
      expect(isNotFoundError('Circle not found')).toBe(true);
      expect(isNotFoundError('Permission denied')).toBe(false);
    });

    it('handles null/undefined', () => {
      // String(null) = 'null', String(undefined) = 'undefined'
      // Neither matches "not found" or "does not exist"
      expect(isNotFoundError(null)).toBe(false);
      expect(isNotFoundError(undefined)).toBe(false);
    });
  });
});
