"use client";

import { useEffect, useState } from "react";
import { getNearConfig, type NearConfig } from "@/lib/near/config";

type RpcTestResult = {
  status: "testing" | "success" | "failed";
  duration?: number;
  chainId?: string;
  blockHeight?: number;
  error?: string;
};

type DiagnosticConfig = NearConfig | { error: string } | null;

export default function DiagnosticPage() {
  const [testResults, setTestResults] = useState<Record<string, RpcTestResult>>({});
  const [config, setConfig] = useState<DiagnosticConfig>(null);

  useEffect(() => {
    // Get config
    try {
      const cfg = getNearConfig();
      setConfig(cfg);
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setConfig({ error: message });
    }
  }, []);

  const testRPC = async (url: string, name: string) => {
    setTestResults((prev) => ({ ...prev, [name]: { status: 'testing' } }));
    
    const startTime = Date.now();
    
    try {
      const response = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: 'test',
          method: 'status',
          params: []
        })
      });

      const duration = Date.now() - startTime;

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data = await response.json();

      setTestResults((prev) => ({
        ...prev,
        [name]: {
          status: 'success',
          duration,
          chainId: data.result?.chain_id,
          blockHeight: data.result?.sync_info?.latest_block_height
        }
      }));
    } catch (error: unknown) {
      const duration = Date.now() - startTime;
      const message = error instanceof Error ? error.message : String(error);
      
      setTestResults((prev) => ({
        ...prev,
        [name]: {
          status: 'failed',
          duration,
          error: message
        }
      }));
    }
  };

  return (
    <div style={{ padding: '20px', maxWidth: '800px', margin: '0 auto', fontFamily: 'system-ui' }}>
      <h1>🔍 NEAR RPC Diagnostic</h1>
      
      <div style={{ background: '#f0f0f0', padding: '20px', borderRadius: '8px', marginBottom: '20px' }}>
        <h2>Environment Configuration</h2>
        {config ? (
          <pre style={{ background: 'white', padding: '10px', borderRadius: '4px', overflow: 'auto' }}>
            {JSON.stringify(config, null, 2)}
          </pre>
        ) : (
          <p>Loading...</p>
        )}
      </div>

      <div style={{ background: '#f0f0f0', padding: '20px', borderRadius: '8px', marginBottom: '20px' }}>
        <h2>RPC Endpoint Tests</h2>
        
        <div style={{ marginBottom: '10px' }}>
          <button 
            onClick={() => testRPC('https://rpc.testnet.pagoda.co', 'pagoda')}
            style={{ padding: '10px 20px', marginRight: '10px', cursor: 'pointer' }}
          >
            Test Pagoda RPC
          </button>
          {testResults.pagoda && (
            <span style={{
              padding: '5px 10px',
              borderRadius: '4px',
              background: testResults.pagoda.status === 'success' ? '#d4edda' : testResults.pagoda.status === 'failed' ? '#f8d7da' : '#fff3cd',
              color: testResults.pagoda.status === 'success' ? '#155724' : testResults.pagoda.status === 'failed' ? '#721c24' : '#856404'
            }}>
              {testResults.pagoda.status === 'success' && `✅ ${testResults.pagoda.duration}ms`}
              {testResults.pagoda.status === 'failed' && `❌ ${testResults.pagoda.error}`}
              {testResults.pagoda.status === 'testing' && `⏳ Testing...`}
            </span>
          )}
        </div>

        <div style={{ marginBottom: '10px' }}>
          <button 
            onClick={() => testRPC('https://rpc.testnet.near.org', 'near')}
            style={{ padding: '10px 20px', marginRight: '10px', cursor: 'pointer' }}
          >
            Test NEAR RPC
          </button>
          {testResults.near && (
            <span style={{
              padding: '5px 10px',
              borderRadius: '4px',
              background: testResults.near.status === 'success' ? '#d4edda' : testResults.near.status === 'failed' ? '#f8d7da' : '#fff3cd',
              color: testResults.near.status === 'success' ? '#155724' : testResults.near.status === 'failed' ? '#721c24' : '#856404'
            }}>
              {testResults.near.status === 'success' && `✅ ${testResults.near.duration}ms`}
              {testResults.near.status === 'failed' && `❌ ${testResults.near.error}`}
              {testResults.near.status === 'testing' && `⏳ Testing...`}
            </span>
          )}
        </div>

        <div style={{ marginBottom: '10px' }}>
          <button 
            onClick={() => testRPC('https://near-testnet.lava.build', 'lava')}
            style={{ padding: '10px 20px', marginRight: '10px', cursor: 'pointer' }}
          >
            Test Lava RPC
          </button>
          {testResults.lava && (
            <span style={{
              padding: '5px 10px',
              borderRadius: '4px',
              background: testResults.lava.status === 'success' ? '#d4edda' : testResults.lava.status === 'failed' ? '#f8d7da' : '#fff3cd',
              color: testResults.lava.status === 'success' ? '#155724' : testResults.lava.status === 'failed' ? '#721c24' : '#856404'
            }}>
              {testResults.lava.status === 'success' && `✅ ${testResults.lava.duration}ms`}
              {testResults.lava.status === 'failed' && `❌ ${testResults.lava.error}`}
              {testResults.lava.status === 'testing' && `⏳ Testing...`}
            </span>
          )}
        </div>
      </div>

      <div style={{ background: '#fff3cd', padding: '20px', borderRadius: '8px', color: '#856404' }}>
        <h3>⚠️ Troubleshooting</h3>
        <ul>
          <li>If ALL tests fail with &quot;Failed to fetch&quot; → <strong>Ad blocker or firewall blocking</strong></li>
          <li>If SOME tests work → Use the working endpoint in .env.local</li>
          <li>If config shows wrong URL → Restart dev server</li>
          <li>Check browser console (F12) for detailed errors</li>
        </ul>
        
        <h4>Quick Fixes:</h4>
        <ol>
          <li>Disable ad blocker</li>
          <li>Try in incognito mode</li>
          <li>Check browser extensions</li>
          <li>Use a working RPC from tests above</li>
        </ol>
      </div>
    </div>
  );
}
