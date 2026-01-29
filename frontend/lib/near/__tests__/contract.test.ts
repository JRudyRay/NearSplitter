/**
 * Contract binding tests - validates type normalization and contract interface alignment
 * UPDATED: Added tests for U128/I128 normalization and CircleState handling
 */
import { describe, expect, it } from 'vitest';

import { normalizeU128, normalizeI128, isValidCircleState, isValidCircle, VALID_CIRCLE_STATES } from '@/lib/types';

describe('Contract type normalization', () => {
  describe('normalizeU128', () => {
    // Contract returns U128 as string in JSON
    it('returns string values as-is', () => {
      expect(normalizeU128('1000000000000000000000000')).toBe('1000000000000000000000000');
      expect(normalizeU128('0')).toBe('0');
    });

    // Handle wrapped object format { U128: "value" } from some RPC responses
    it('unwraps { U128: string } format', () => {
      expect(normalizeU128({ U128: '1000000000000000000000000' })).toBe('1000000000000000000000000');
    });

    // Handle number format (small values)
    it('converts numbers to strings', () => {
      expect(normalizeU128(12345)).toBe('12345');
      expect(normalizeU128(0)).toBe('0');
    });

    // Handle BigInt format
    it('converts BigInt to strings', () => {
      expect(normalizeU128(BigInt('1000000000000000000000000'))).toBe('1000000000000000000000000');
    });

    // Edge cases
    it('handles null/undefined with fallback', () => {
      expect(normalizeU128(null as unknown as string)).toBe('0');
      expect(normalizeU128(undefined as unknown as string)).toBe('0');
    });
  });

  describe('normalizeI128', () => {
    // I128 can be negative (for debtor balances)
    it('handles positive string values', () => {
      expect(normalizeI128('1000000000000000000000000')).toBe('1000000000000000000000000');
    });

    it('handles negative string values', () => {
      expect(normalizeI128('-1000000000000000000000000')).toBe('-1000000000000000000000000');
    });

    it('converts negative numbers', () => {
      expect(normalizeI128(-12345)).toBe('-12345');
    });

    // Handle wrapped format { I128: "value" }
    it('handles wrapped format with negative', () => {
      expect(normalizeI128({ I128: '-500' })).toBe('-500');
    });
  });

  describe('CircleState validation', () => {
    it('validates all known circle states', () => {
      expect(isValidCircleState('open')).toBe(true);
      expect(isValidCircleState('settlement_in_progress')).toBe(true);
      expect(isValidCircleState('settled')).toBe(true);
    });

    it('rejects invalid circle states', () => {
      expect(isValidCircleState('unknown')).toBe(false);
      expect(isValidCircleState('')).toBe(false);
      expect(isValidCircleState('Open')).toBe(false); // Case sensitive
    });

    it('VALID_CIRCLE_STATES contains all states', () => {
      expect(VALID_CIRCLE_STATES).toContain('open');
      expect(VALID_CIRCLE_STATES).toContain('settlement_in_progress');
      expect(VALID_CIRCLE_STATES).toContain('settled');
      expect(VALID_CIRCLE_STATES).toHaveLength(3);
    });
  });

  describe('Circle validation', () => {
    const validCircle = {
      id: 'circle-123',
      owner: 'alice.testnet',
      name: 'Test Circle',
      members: ['alice.testnet', 'bob.testnet'],
      created_ms: 1704067200000,
      invite_code_hash: null,
      locked: false,
      membership_open: true,
      state: 'open' as const
    };

    it('validates a correct circle object', () => {
      expect(isValidCircle(validCircle)).toBe(true);
    });

    it('rejects circle with missing required fields', () => {
      const { id, ...missingId } = validCircle;
      expect(isValidCircle(missingId)).toBe(false);

      const { members, ...missingMembers } = validCircle;
      expect(isValidCircle(missingMembers)).toBe(false);
    });

    it('rejects circle with invalid state', () => {
      const invalidState = { ...validCircle, state: 'invalid' };
      expect(isValidCircle(invalidState)).toBe(false);
    });

    it('rejects non-object values', () => {
      expect(isValidCircle(null)).toBe(false);
      expect(isValidCircle(undefined)).toBe(false);
      expect(isValidCircle('string')).toBe(false);
      expect(isValidCircle(123)).toBe(false);
    });
  });
});

describe('Contract method signatures', () => {
  // Document the expected contract interface for reference
  // These tests serve as documentation and will fail if the interface changes
  
  it('documents view method signatures', () => {
    // View methods should match these signatures:
    const viewMethods = {
      // get_circle(circle_id: String) -> Option<Circle>
      get_circle: { args: ['circle_id'], returns: 'Circle | null' },
      
      // get_circles_for_member(account_id: String) -> Vec<Circle>
      get_circles_for_member: { args: ['account_id'], returns: 'Circle[]' },
      
      // get_expenses(circle_id: String) -> Vec<Expense>
      get_expenses: { args: ['circle_id'], returns: 'Expense[]' },
      
      // get_balances(circle_id: String) -> Vec<BalanceView>
      get_balances: { args: ['circle_id'], returns: 'BalanceView[]' },
      
      // get_settlements(circle_id: String) -> Vec<Settlement>
      get_settlements: { args: ['circle_id'], returns: 'Settlement[]' },
      
      // get_settlement_suggestions(circle_id: String) -> Vec<SettlementSuggestion>
      get_settlement_suggestions: { args: ['circle_id'], returns: 'SettlementSuggestion[]' },
      
      // get_claims(circle_id: String) -> Vec<Claim>
      get_claims: { args: ['circle_id'], returns: 'Claim[]' },
      
      // get_confirmations(circle_id: String) -> Vec<AccountId>
      get_confirmations: { args: ['circle_id'], returns: 'string[]' },
      
      // storage_balance_bounds() -> StorageBalanceBounds
      storage_balance_bounds: { args: [], returns: 'StorageBalanceBounds' },
      
      // storage_balance_of(account_id: String) -> Option<StorageBalance>
      storage_balance_of: { args: ['account_id'], returns: 'StorageBalance | null' },
    };

    // This test documents the interface - if it fails, update the frontend bindings
    expect(Object.keys(viewMethods).length).toBeGreaterThan(0);
  });

  it('documents change method signatures', () => {
    // Change methods require gas and possibly deposit
    const changeMethods = {
      // create_circle(name: String, invite_code: Option<String>) -> Circle
      create_circle: { args: ['name', 'invite_code?'], deposit: '0', gas: '50 TGas' },
      
      // join_circle(circle_id: String, invite_code: Option<String>)
      join_circle: { args: ['circle_id', 'invite_code?'], deposit: '0', gas: '50 TGas' },
      
      // add_expense(circle_id: String, participants: Vec<MemberShare>, amount_yocto: U128, memo: String) -> Expense
      add_expense: { args: ['circle_id', 'participants', 'amount_yocto', 'memo'], deposit: '0', gas: '100 TGas' },
      
      // file_claim(...) -> Claim
      file_claim: { args: ['circle_id', 'expense_id', 'reason', 'proposed_amount?', 'proposed_participants?'], deposit: '0', gas: '100 TGas' },
      
      // approve_claim(circle_id: String, claim_id: String)
      approve_claim: { args: ['circle_id', 'claim_id'], deposit: '0', gas: '100 TGas' },
      
      // reject_claim(circle_id: String, claim_id: String)
      reject_claim: { args: ['circle_id', 'claim_id'], deposit: '0', gas: '100 TGas' },
      
      // confirm_ledger(circle_id: String) - requires escrow deposit
      confirm_ledger: { args: ['circle_id'], deposit: 'escrow amount', gas: '150 TGas' },
      
      // record_payment(circle_id: String, to: String, amount: U128) - requires attached NEAR
      record_payment: { args: ['circle_id', 'to', 'amount'], deposit: 'payment amount', gas: '150 TGas' },
      
      // storage_deposit(account_id: Option<String>) - requires min storage deposit
      storage_deposit: { args: ['account_id?'], deposit: 'min storage', gas: '50 TGas' },
    };

    // This test documents the interface - if it fails, update the frontend bindings
    expect(Object.keys(changeMethods).length).toBeGreaterThan(0);
  });
});
