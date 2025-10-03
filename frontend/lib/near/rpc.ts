import { getNearConfig } from "@/lib/near/config";

const decoder = new TextDecoder("utf-8");

function encodeJsonToBase64(argsJson: string): string {
  if (typeof window !== 'undefined' && typeof window.btoa === 'function') {
    // Encode to UTF-8 first to support non-ASCII characters before base64 conversion
    const utf8Bytes = new TextEncoder().encode(argsJson);
    let binary = '';
    utf8Bytes.forEach((byte) => {
      binary += String.fromCharCode(byte);
    });
    return window.btoa(binary);
  }

  const NodeBuffer = (globalThis as { Buffer?: typeof import('buffer').Buffer }).Buffer;
  if (NodeBuffer) {
    return NodeBuffer.from(argsJson, 'utf-8').toString('base64');
  }

  throw new Error('Base64 encoding unavailable in current environment');
}

type RpcError = Error & {
  status?: number;
  endpoint?: string;
  recoverable?: boolean;
  responseText?: string;
};

type AggregatedRpcError = RpcError & { errors?: RpcError[] };

function shouldRetryStatus(status: number): boolean {
  return status === 429 || status === 403 || status === 408 || status === 425 || status >= 500;
}

async function rpcRequest<T>(method: string, params: Record<string, unknown>): Promise<T> {
  // Ensure we're in the browser - return a promise that never resolves during SSR
  if (typeof window === 'undefined') {
    console.warn('[RPC] Attempted to call RPC during SSR/build phase - this should not happen');
    // Return a promise that never resolves to prevent build errors
    return new Promise(() => {});
  }

  const config = getNearConfig();
  const endpoints = Array.from(new Set([config.nodeUrl, ...(config.fallbackNodeUrls ?? [])])).filter(Boolean);
  const attemptErrors: RpcError[] = [];

  for (const endpoint of endpoints) {
    try {
      console.log('[RPC] Making request to:', endpoint, 'method:', method);

      const response = await fetch(endpoint, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: "dontcare",
          method,
          params
        })
      });

      if (!response.ok) {
        const text = await response.text();
        const httpError = new Error(`RPC error: ${response.status} ${text || response.statusText}`) as RpcError;
        httpError.status = response.status;
        httpError.endpoint = endpoint;
        httpError.responseText = text;

        if (shouldRetryStatus(response.status)) {
          httpError.recoverable = true;
        }
        throw httpError;
      }

      const payload = (await response.json()) as { error?: { message: string }; result?: T };

      if (payload.error) {
        const rpcError = new Error(payload.error.message) as RpcError;
        rpcError.endpoint = endpoint;
        rpcError.recoverable = false;
        console.error('[RPC] RPC error payload:', payload.error);
        throw rpcError;
      }

      if (!payload.result) {
        const malformed = new Error('Malformed RPC response: missing result') as RpcError;
        malformed.endpoint = endpoint;
        malformed.recoverable = true;
        throw malformed;
      }

      return payload.result;
    } catch (error) {
      const err: RpcError = error instanceof Error ? (error as RpcError) : new Error(String(error));
      err.endpoint = err.endpoint ?? endpoint;

      if (err instanceof TypeError && err.message === 'Failed to fetch') {
        err.recoverable = true;
        console.error('[RPC] Fetch failed due to network/CORS issue for endpoint:', endpoint);
        console.error('[RPC] Possible causes:');
        console.error('  1. Endpoint missing browser CORS headers (common on deprecated rpc.near.org / pagoda.co)');
        console.error('  2. Browser extension (ad blocker, privacy filter) blocking the request');
        console.error('  3. Local firewall/antivirus filtering the domain');
        console.error('  4. Offline or unstable network connection');
        console.error('  Tip: Switching to a community RPC like https://rpc.testnet.fastnear.com usually resolves this.');
      } else {
        console.error('[RPC] Error from endpoint', endpoint, err);
      }

      attemptErrors.push(err);

      if (err.recoverable) {
        console.warn('[RPC] Recoverable error detected. Trying next RPC endpoint if available...');
        continue;
      }

      console.error('[RPC] Non-recoverable error encountered. Aborting further retries.');
      break;
    }
  }

  if (attemptErrors.length) {
    const endpointsTried = attemptErrors.map((item) => item.endpoint ?? 'unknown').join(', ');
    const lastError = attemptErrors[attemptErrors.length - 1];
    const message = `RPC request failed after trying ${attemptErrors.length} endpoint(s). Tried: ${endpointsTried}. Last error: ${lastError?.message ?? 'Unknown error'}`;
  const aggregate = new Error(message, { cause: lastError }) as AggregatedRpcError;
    aggregate.endpoint = lastError?.endpoint;
    aggregate.recoverable = lastError?.recoverable;
  aggregate.errors = attemptErrors;

    console.error('[RPC] All endpoints failed for method:', method, 'params:', params);
    throw aggregate;
  }

  throw new Error(`RPC request failed for method "${method}" with unknown error`);
}

export async function viewFunction<T>(methodName: string, args: Record<string, unknown> = {}): Promise<T> {
  console.log(`[RPC] view call: ${methodName}`, args);
  
  const { contractId } = getNearConfig();
  
  try {
    // Encode arguments to base64 - NEAR RPC expects base64-encoded JSON
  const argsJson = JSON.stringify(args);
  const argsBase64 = encodeJsonToBase64(argsJson);
    
    console.log(`[RPC] Encoded args for ${methodName}:`, { argsJson, argsBase64 });
    
    const result = await rpcRequest<{ result: number[] }>("query", {
      request_type: "call_function",
      account_id: contractId,
      method_name: methodName,
      args_base64: argsBase64,
      finality: "optimistic"
    });

    // Handle empty result (null return from contract)
    if (!result.result || result.result.length === 0) {
      console.log(`[RPC] Empty result (null) for ${methodName}`);
      return null as T;
    }

    const decoded = decoder.decode(new Uint8Array(result.result));
    console.log(`[RPC] Decoded result for ${methodName}:`, decoded);
    
    if (decoded === 'null' || decoded === '') {
      return null as T;
    }

    return JSON.parse(decoded) as T;
  } catch (error) {
    // Handle contract panics (like "Circle not found")
    if (error instanceof Error && error.message.includes('GuestPanic')) {
      console.warn(`[RPC] Contract panic for ${methodName}:`, error.message);
      throw error; // Re-throw so caller can handle
    }
    console.error(`[RPC] Failed view call ${methodName}:`, error);
    throw error;
  }
}
