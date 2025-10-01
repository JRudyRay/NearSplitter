import { utils } from "near-api-js";

export function formatNearAmount(amount: string | number, fractionDigits = 2): string {
  try {
    return utils.format.formatNearAmount(String(amount), fractionDigits);
  } catch (error) {
    console.warn("Failed to format NEAR amount", error);
    return String(amount);
  }
}

export function parseNearAmount(amount: string): string {
  const trimmed = amount.trim();
  if (!trimmed || Number.isNaN(Number(trimmed))) {
    throw new Error(`Invalid NEAR amount: ${amount}`);
  }
  const parsed = utils.format.parseNearAmount(trimmed);
  if (!parsed) {
    throw new Error(`Invalid NEAR amount: ${amount}`);
  }
  return parsed;
}

export function formatTimestamp(tsMs: number): string {
  return new Date(tsMs).toLocaleString();
}

export function shortenAccountId(accountId: string, chars = 6): string {
  if (accountId.length <= chars * 2) {
    return accountId;
  }
  return `${accountId.slice(0, chars)}…${accountId.slice(-chars)}`;
}
