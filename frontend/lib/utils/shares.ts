import type { MemberShare } from '@/lib/types';

export function buildEqualShares(memberIds: string[]): MemberShare[] {
  if (memberIds.length === 0) {
    throw new Error('At least one participant required');
  }

  const baseShare = Math.floor(10_000 / memberIds.length);
  const remainder = 10_000 - baseShare * memberIds.length;

  return memberIds.map((accountId, index) => ({
    account_id: accountId,
    weight_bps: baseShare + (index === memberIds.length - 1 ? remainder : 0)
  }));
}

export function uniq(items: string[]): string[] {
  return Array.from(new Set(items));
}
