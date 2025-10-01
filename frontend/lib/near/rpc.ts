import { utils } from "near-api-js";
import { getNearConfig } from "@/lib/near/config";

const decoder = new TextDecoder("utf-8");
const encoder = new TextEncoder();

async function rpcRequest<T>(method: string, params: Record<string, unknown>): Promise<T> {
  const { nodeUrl } = getNearConfig();

  const response = await fetch(nodeUrl, {
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
    throw new Error(`RPC error: ${response.status} ${text}`);
  }

  const payload = (await response.json()) as { error?: { message: string }; result?: T };

  if (payload.error) {
    throw new Error(payload.error.message);
  }

  if (!payload.result) {
    throw new Error("Malformed RPC response: missing result");
  }

  return payload.result;
}

export async function viewFunction<T>(methodName: string, args: Record<string, unknown> = {}): Promise<T> {
  const { contractId } = getNearConfig();
  const result = await rpcRequest<{ result: number[] }>("query", {
    request_type: "call_function",
    account_id: contractId,
    method_name: methodName,
    args_base64: utils.serialize.base_encode(encoder.encode(JSON.stringify(args))),
    finality: "optimistic"
  });

  return JSON.parse(decoder.decode(new Uint8Array(result.result))) as T;
}
