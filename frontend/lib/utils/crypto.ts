/**
 * Cryptographic utilities for secure password handling
 * 
 * SECURITY: All password hashing is done client-side before sending to the blockchain.
 * This ensures that plaintext passwords NEVER appear on-chain or in transaction history.
 */

/**
 * Generate a cryptographically secure random salt
 * Uses Web Crypto API for secure random generation
 * @returns 32-character hex string salt
 */
export function generateSalt(): string {
  const array = new Uint8Array(16); // 16 bytes = 32 hex chars
  crypto.getRandomValues(array);
  return Array.from(array)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Hash a password with a salt using SHA-256
 * Uses the same format as the smart contract expects: "salt:password:nearsplitter-v1"
 * 
 * @param password - The plaintext password to hash
 * @param salt - The salt to use (from generateSalt() for create, or from circle for join)
 * @returns 64-character hex string (SHA-256 hash)
 */
export async function hashPassword(password: string, salt: string): Promise<string> {
  const message = `${salt}:${password}:nearsplitter-v1`;
  const encoder = new TextEncoder();
  const data = encoder.encode(message);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Prepare invite code data for creating a circle
 * Generates a salt and hashes the password
 * 
 * @param password - The plaintext password (will NOT be sent to blockchain)
 * @returns Object with hash and salt to send to contract, plus original password for local storage
 */
export async function prepareInviteCodeForCreate(password: string): Promise<{
  invite_code_hash: string;
  invite_code_salt: string;
  originalPassword: string;
}> {
  const salt = generateSalt();
  const hash = await hashPassword(password, salt);
  return {
    invite_code_hash: hash,
    invite_code_salt: salt,
    originalPassword: password, // Keep for local storage only - never sent to chain!
  };
}

/**
 * Prepare invite code hash for joining a circle
 * Uses the circle's salt to hash the password
 * 
 * @param password - The plaintext password (will NOT be sent to blockchain)
 * @param salt - The salt from the circle's invite_code_salt field
 * @returns 64-character hex string hash to send to contract
 */
export async function prepareInviteCodeForJoin(
  password: string,
  salt: string
): Promise<string> {
  return hashPassword(password, salt);
}
