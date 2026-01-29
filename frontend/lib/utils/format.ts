import { utils } from "near-api-js";

/**
 * Format a yoctoNEAR amount to human-readable NEAR
 * UPDATED: Handle negative amounts for I128 balance display
 */
export function formatNearAmount(amount: string | number, fractionDigits = 2): string {
  try {
    const amountStr = String(amount);
    // Handle negative amounts (I128 from balance calculations)
    const isNegative = amountStr.startsWith('-');
    const absoluteAmount = isNegative ? amountStr.slice(1) : amountStr;
    
    const formatted = utils.format.formatNearAmount(absoluteAmount, fractionDigits);
    // Ensure proper formatting with leading zero and exactly 2 decimal places
    const num = parseFloat(formatted);
    const result = num.toFixed(fractionDigits);
    
    return isNegative ? `-${result}` : result;
  } catch (error) {
    console.warn("Failed to format NEAR amount", error);
    return "0.00";
  }
}

/**
 * Format balance with sign indicator for UI display
 * Positive = creditor (green/+), Negative = debtor (red/-)
 */
export function formatBalanceWithSign(amount: string, fractionDigits = 2): { value: string; isPositive: boolean; isNegative: boolean; isZero: boolean } {
  const isNegative = amount.startsWith('-');
  const absoluteAmount = isNegative ? amount.slice(1) : amount;
  const formatted = formatNearAmount(absoluteAmount, fractionDigits);
  const numValue = parseFloat(formatted);
  
  return {
    value: formatted,
    isPositive: numValue > 0 && !isNegative,
    isNegative: isNegative && numValue > 0,
    isZero: numValue === 0
  };
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
