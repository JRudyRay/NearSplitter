/**
 * Secure password storage using Web Crypto API (AES-GCM).
 * 
 * The owner signs a deterministic message to derive an encryption key,
 * then uses that key to encrypt/decrypt the circle password.
 * Only the owner can decrypt because only they can sign with their private key.
 * 
 * Uses NIST-approved AES-256-GCM for authenticated encryption.
 */

const ALGORITHM = 'AES-GCM';
const KEY_LENGTH = 256; // bits
const IV_LENGTH = 12; // 96 bits for GCM

/**
 * Derive a cryptographic key from a signature using PBKDF2
 */
async function deriveKeyFromSignature(signatureHex: string): Promise<CryptoKey> {
  try {
    // Convert hex signature to bytes
    const signatureBuffer = new Uint8Array(
      signatureHex.match(/.{1,2}/g)!.map(byte => parseInt(byte, 16))
    );
    
    const baseKey = await crypto.subtle.importKey(
      'raw',
      signatureBuffer,
      { name: 'PBKDF2' },
      false,
      ['deriveBits']
    );

    const derivedBits = await crypto.subtle.deriveBits(
      {
        name: 'PBKDF2',
        salt: new TextEncoder().encode('nearsplitter-v1'),
        iterations: 100_000,
        hash: 'SHA-256',
      },
      baseKey,
      KEY_LENGTH
    );

    return crypto.subtle.importKey(
      'raw',
      derivedBits,
      { name: ALGORITHM },
      false,
      ['encrypt', 'decrypt']
    );
  } catch (error) {
    throw new Error(`Failed to derive key: ${(error as Error).message}`);
  }
}

/**
 * Encrypt a password for local storage
 * @param password The circle password to encrypt
 * @param signatureHex The wallet signature (hex string) used to derive encryption key
 */
async function encryptPassword(
  password: string,
  signatureHex: string
): Promise<string> {
  try {
    const key = await deriveKeyFromSignature(signatureHex);
    const iv = crypto.getRandomValues(new Uint8Array(IV_LENGTH));
    const plaintext = new TextEncoder().encode(password);

    const ciphertext = await crypto.subtle.encrypt(
      { name: ALGORITHM, iv },
      key,
      plaintext
    );

    // Combine IV + ciphertext for storage
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);

    // Encode to base64 for storage
    return btoa(String.fromCharCode(...combined));
  } catch (error) {
    throw new Error(`Failed to encrypt password: ${(error as Error).message}`);
  }
}

/**
 * Decrypt a password from local storage
 * @param encrypted The encrypted password (base64)
 * @param signatureHex The wallet signature (hex string) used to derive encryption key
 */
async function decryptPassword(
  encrypted: string,
  signatureHex: string
): Promise<string> {
  try {
    const key = await deriveKeyFromSignature(signatureHex);
    
    // Decode from base64
    const combined = new Uint8Array(
      atob(encrypted).split('').map(c => c.charCodeAt(0))
    );

    // Extract IV and ciphertext
    const iv = combined.slice(0, IV_LENGTH);
    const ciphertext = combined.slice(IV_LENGTH);

    const plaintext = await crypto.subtle.decrypt(
      { name: ALGORITHM, iv },
      key,
      ciphertext
    );

    return new TextDecoder().decode(plaintext);
  } catch (error) {
    throw new Error(`Failed to decrypt password: ${(error as Error).message}`);
  }
}

/**
 * Store an encrypted password for a circle
 */
export async function storeEncryptedPassword(
  circleId: string,
  password: string,
  signatureHex: string
): Promise<void> {
  const encrypted = await encryptPassword(password, signatureHex);
  const storageKey = `nearsplitter:encrypted-password:${circleId}`;
  localStorage.setItem(storageKey, encrypted);
}

/**
 * Retrieve and decrypt a password for a circle
 */
export async function getEncryptedPassword(
  circleId: string,
  signatureHex: string
): Promise<string | null> {
  const storageKey = `nearsplitter:encrypted-password:${circleId}`;
  const encrypted = localStorage.getItem(storageKey);
  
  if (!encrypted) return null;
  
  try {
    const decrypted = await decryptPassword(encrypted, signatureHex);
    return decrypted;
  } catch (error) {
    console.error(`Failed to decrypt password for ${circleId}:`, error);
    return null;
  }
}

/**
 * Check if an encrypted password exists for a circle
 */
export function hasEncryptedPassword(circleId: string): boolean {
  const storageKey = `nearsplitter:encrypted-password:${circleId}`;
  return Boolean(localStorage.getItem(storageKey));
}

/**
 * Remove encrypted password for a circle
 */
export function removeEncryptedPassword(circleId: string): void {
  const storageKey = `nearsplitter:encrypted-password:${circleId}`;
  localStorage.removeItem(storageKey);
}

/**
 * Get the message to sign for deriving the encryption key
 * This message is signed by the user's wallet to generate a signature
 * that is then used to derive the encryption key.
 */
export function getSignMessage(circleId: string): string {
  return `nearsplitter:circle:${circleId}:password-key:v2`;
}

/**
 * Verify browser support for Web Crypto API
 */
export function isWebCryptoSupported(): boolean {
  return (
    typeof window !== 'undefined' &&
    window.crypto &&
    typeof window.crypto.subtle === 'object'
  );
}
