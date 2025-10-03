'use client';

import { useCallback, useEffect, useMemo, useState, type ChangeEvent, type FormEvent } from 'react';
import { Loader2, PlusCircle, Wallet } from 'lucide-react';
import { useNear } from '@/lib/hooks/use-near';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useContractView } from '@/lib/hooks/use-contract-view';
import { useContractCall } from '@/lib/hooks/use-contract-call';
import { useLocalStorage } from '@/lib/hooks/use-local-storage';
import { formatNearAmount, formatTimestamp, parseNearAmount } from '@/lib/utils/format';
import { buildEqualShares, uniq } from '@/lib/utils/shares';
import { getNearConfig } from '@/lib/near/config';
import type {
  BalanceView,
  Circle,
  Expense,
  SettlementSuggestion,
  StorageBalance,
  StorageBalanceBounds
} from '@/lib/types';
import { getCircle } from '@/lib/near/contract';
import { GAS_150_TGAS } from '@/lib/constants';

interface MessageState {
  type: 'success' | 'error';
  text: string;
}

export default function HomePage() {
  const near = useNear();
  const config = getNearConfig();
  const contractId = config.contractId;
  const [notification, setNotification] = useState<MessageState | null>(null);
  const [trackedKey, setTrackedKey] = useState<string>('nearsplitter:guest:circles');
  const [trackedCircleIds, setTrackedCircleIds] = useLocalStorage<string[]>(trackedKey, []);
  const [circleMap, setCircleMap] = useState<Record<string, Circle>>({});
  const [selectedCircleId, setSelectedCircleId] = useState<string | null>(null);

  const [createCircleName, setCreateCircleName] = useState('');
  const [createCirclePassword, setCreateCirclePassword] = useState('');
  const [usePassword, setUsePassword] = useState(false);
  const [joinCircleId, setJoinCircleId] = useState('');
  const [joinCirclePassword, setJoinCirclePassword] = useState('');
  const [expenseAmount, setExpenseAmount] = useState('');
  const [expenseMemo, setExpenseMemo] = useState('');
  const [selectedParticipants, setSelectedParticipants] = useState<Record<string, boolean>>({});
  const [settlementAmount, setSettlementAmount] = useState('');
  const [settlementRecipient, setSettlementRecipient] = useState('');

  const registerMutation = useContractCall();
  const createCircleMutation = useContractCall();
  const joinCircleMutation = useContractCall();
  const addExpenseMutation = useContractCall();
  const payNativeMutation = useContractCall();
  const confirmLedgerMutation = useContractCall();

  // SIMPLE APPROACH: Just use near.accountId to determine if logged in
  // The registration check will happen automatically when near.accountId exists
  
  // Check storage bounds - this is a public query that doesn't require a signed-in account
  const storageBounds = useContractView<StorageBalanceBounds>(
    'storage_balance_bounds',
    {}
  );
  
  // Only check user's storage balance when user is logged in
  const storageBalance = useContractView<StorageBalance | null>(
    near.accountId ? 'storage_balance_of' : null,
    near.accountId ? { account_id: near.accountId } : null,
    { 
      refreshInterval: 15_000
    }
  );
  const mutateStorageBalance = storageBalance.mutate;

  const isRegistered = Boolean(storageBalance.data?.total);
  const isCheckingRegistration = near.accountId && storageBalance.isLoading;
  
  // Debug logging for registration status
  useEffect(() => {
    if (near.accountId) {
      console.log('[Registration] Status:', {
        accountId: near.accountId,
        isRegistered,
        isCheckingRegistration,
        storageData: storageBalance.data,
        storageError: storageBalance.error,
        storageBoundsData: storageBounds.data,
        storageBoundsError: storageBounds.error
      });
    }
  }, [near.accountId, isRegistered, isCheckingRegistration, storageBalance.data, storageBalance.error, storageBounds.data, storageBounds.error]);

  // Detect successful transaction return from wallet
  useEffect(() => {
    if (typeof window === 'undefined') return;
    
    const urlParams = new URLSearchParams(window.location.search);
    const transactionHashes = urlParams.get('transactionHashes');
    const errorCode = urlParams.get('errorCode');
    const errorMessage = urlParams.get('errorMessage');
    
    // Handle transaction error
    if (errorCode || errorMessage) {
      console.log('[Transaction Return] Transaction failed:', { errorCode, errorMessage });
      window.history.replaceState({}, '', window.location.pathname);
      setNotification({ 
        type: 'error', 
        text: errorMessage || 'Transaction failed. Please try again.' 
      });
      return;
    }
    
    // Handle successful transaction
    if (transactionHashes && near.accountId) {
      console.log('[Transaction Return] Detected transaction completion:', transactionHashes);
      
      // Clear URL parameters to avoid re-triggering
      window.history.replaceState({}, '', window.location.pathname);
      
      console.log('[Transaction Return] Starting fast polling for registration status...');
      
      // Immediately check once before starting interval
      (async () => {
        try {
          const balance = await near.viewFunction({
            contractId,
            method: 'storage_balance_of',
            args: { account_id: near.accountId }
          });
          
          if (balance && (balance as StorageBalance).total) {
            console.log('[Transaction Return] ✓ Registration confirmed immediately!', balance);
            mutateStorageBalance(balance as StorageBalance, false);
            
            // Clear any stale circle data from localStorage
            const storageKey = `nearsplitter:${near.accountId}:circles`;
            localStorage.removeItem(storageKey);
            setTrackedCircleIds([]);
            setCircleMap({});
            setSelectedCircleId(null);
            console.log('[Transaction Return] Cleared stale circle data');
            
            setNotification({ 
              type: 'success', 
              text: 'Registration successful! You can now use NearSplitter.' 
            });
            return; // Exit early, no need to poll
          }
        } catch (err) {
          console.log('[Transaction Return] Initial check failed, starting polling...', err);
        }
      })();
      
      // Use aggressive polling for immediate feedback
      let pollCount = 0;
      const maxPolls = 20; // 20 attempts x 500ms = 10 seconds max
      let successNotified = false;
      
      const pollInterval = setInterval(async () => {
        pollCount++;
        console.log(`[Transaction Return] Polling (${pollCount}/${maxPolls})...`);
        
        try {
          const balance = await near.viewFunction({
            contractId,
            method: 'storage_balance_of',
            args: { account_id: near.accountId }
          });
          
          if (balance && (balance as StorageBalance).total) {
            clearInterval(pollInterval);
            
            if (!successNotified) {
              successNotified = true;
              console.log('[Transaction Return] ✓ Registration confirmed!', balance);
              
              // Manually update SWR cache
              mutateStorageBalance(balance as StorageBalance, false);
              
              // Clear any stale circle data from localStorage
              const storageKey = `nearsplitter:${near.accountId}:circles`;
              localStorage.removeItem(storageKey);
              setTrackedCircleIds([]);
              setCircleMap({});
              setSelectedCircleId(null);
              console.log('[Transaction Return] Cleared stale circle data');
              
              setNotification({ 
                type: 'success', 
                text: 'Registration successful! You can now use NearSplitter.' 
              });
            }
          } else if (pollCount >= maxPolls) {
            clearInterval(pollInterval);
            console.warn('[Transaction Return] Polling timed out - trying one final check...');
            
            // One final attempt with a full page reload to clear any cache issues
            setTimeout(() => {
              window.location.reload();
            }, 1000);
          }
        } catch (err) {
          console.error('[Transaction Return] Poll error:', err);
          
          if (pollCount >= maxPolls) {
            clearInterval(pollInterval);
          }
        }
      }, 500); // Poll every 500ms (much faster!)
      
      // Cleanup on unmount
      return () => {
        clearInterval(pollInterval);
      };
    }
  }, [contractId, mutateStorageBalance, near, near.accountId, near.viewFunction, setCircleMap, setNotification, setSelectedCircleId, setTrackedCircleIds]);

  // Fetch all circles where the user is a member (including owned circles)
  // ONLY if the user is registered (has storage deposit)
  const memberCircles = useContractView<Circle[]>(
    (near.accountId && isRegistered) ? 'list_circles_by_member' : null,
    (near.accountId && isRegistered) ? { account_id: near.accountId, from: 0, limit: 100 } : null,
    { refreshInterval: 30_000 }
  );

  const circleExpenses = useContractView<Expense[]>(
    selectedCircleId ? 'list_expenses' : null,
    selectedCircleId ? { circle_id: selectedCircleId, from: 0, limit: 100 } : null,
    { refreshInterval: 20_000 }
  );
  const circleBalances = useContractView<BalanceView[]>(
    selectedCircleId ? 'compute_balances' : null,
    selectedCircleId ? { circle_id: selectedCircleId } : null,
    { refreshInterval: 20_000 }
  );
  const circleSuggestions = useContractView<SettlementSuggestion[]>(
    selectedCircleId ? 'suggest_settlements' : null,
    selectedCircleId ? { circle_id: selectedCircleId } : null,
    { refreshInterval: 25_000 }
  );
  const circleConfirmations = useContractView<string[]>(
    selectedCircleId ? 'get_confirmations' : null,
    selectedCircleId ? { circle_id: selectedCircleId } : null,
    { refreshInterval: 15_000 }
  );
  const isFullyConfirmed = useContractView<boolean>(
    selectedCircleId ? 'is_fully_confirmed' : null,
    selectedCircleId ? { circle_id: selectedCircleId } : null,
    { refreshInterval: 15_000 }
  );
  
  // Autopay queries
  const userAutopayStatus = useContractView<boolean>(
    selectedCircleId && near.accountId ? 'get_autopay' : null,
    selectedCircleId && near.accountId ? { circle_id: selectedCircleId, account_id: near.accountId } : null,
    { refreshInterval: 20_000 }
  );
  const requiredAutopayDeposit = useContractView<string>(
    selectedCircleId && near.accountId ? 'get_required_autopay_deposit' : null,
    selectedCircleId && near.accountId ? { circle_id: selectedCircleId, account_id: near.accountId } : null,
    { refreshInterval: 20_000 }
  );
  const userEscrowDeposit = useContractView<string>(
    selectedCircleId && near.accountId ? 'get_escrow_deposit' : null,
    selectedCircleId && near.accountId ? { circle_id: selectedCircleId, account_id: near.accountId } : null,
    { refreshInterval: 20_000 }
  );
  const allMembersAutopay = useContractView<boolean>(
    selectedCircleId ? 'all_members_autopay' : null,
    selectedCircleId ? { circle_id: selectedCircleId } : null,
    { refreshInterval: 20_000 }
  );

  const selectedCircle = selectedCircleId ? circleMap[selectedCircleId] : null;
  const membersSignature = useMemo(
    () => (selectedCircle ? selectedCircle.members.join('|') : ''),
    [selectedCircle]
  );

  useEffect(() => {
    if (!near.accountId) {
      setTrackedKey('nearsplitter:guest:circles');
      return;
    }
    setTrackedKey(`nearsplitter:${near.accountId}:circles`);
  }, [near.accountId]);

  useEffect(() => {
    if (!memberCircles.data) {
      return;
    }
    const circles = memberCircles.data as Circle[];
    setCircleMap((prev: Record<string, Circle>) => {
      const next = { ...prev };
      for (const circle of circles) {
        next[circle.id] = circle;
      }
      return next;
    });
    setTrackedCircleIds((prev: string[]) => uniq([...prev, ...circles.map((c: Circle) => c.id)]));
  }, [memberCircles.data, setTrackedCircleIds]);

  useEffect(() => {
    if (!selectedCircleId && trackedCircleIds.length > 0) {
      setSelectedCircleId(trackedCircleIds[0]);
    }
  }, [selectedCircleId, trackedCircleIds]);

  useEffect(() => {
    if (!selectedCircle) {
      setSelectedParticipants({});
      return;
    }
    const defaults = Object.fromEntries(
      selectedCircle.members.map((member: string) => [member, true] as const)
    ) as Record<string, boolean>;
    setSelectedParticipants(defaults);
  }, [selectedCircleId, selectedCircle, membersSignature]);

  // Hydrate missing circles from localStorage - ONLY if registered
  useEffect(() => {
    // Don't try to load circles if not registered
    if (!isRegistered || !near.accountId) {
      return;
    }
    
    const missing = trackedCircleIds.filter((id: string) => !circleMap[id]);
    if (missing.length === 0) {
      return;
    }
    (async () => {
      const resolved = await Promise.allSettled(missing.map((id: string) => getCircle(id, near.viewFunction)));
      const next: Record<string, Circle> = {};
      const failedIds: string[] = [];
      
      resolved.forEach((result: PromiseSettledResult<Circle>, idx: number) => {
        if (result.status === 'fulfilled') {
          next[result.value.id] = result.value;
        } else {
          const circleId = missing[idx];
          console.warn('Failed to resolve circle', circleId, result.reason);
          failedIds.push(circleId);
          
          // Don't show notification for "Circle not found" errors
          // This happens normally when localStorage has stale data
          const errorMsg = result.reason?.message || '';
          if (!errorMsg.includes('Circle not found') && !errorMsg.includes('not found')) {
            setNotification({ type: 'error', text: `Unable to load circle ${circleId}` });
          } else {
            console.log(`[Circle] Auto-removed non-existent circle: ${circleId}`);
          }
        }
      });
      
      // Remove failed circles from tracking
      if (failedIds.length > 0) {
        setTrackedCircleIds((prev: string[]) => prev.filter((id: string) => !failedIds.includes(id)));
        console.log('Removed non-existent circles from tracking:', failedIds);
      }
      
      if (Object.keys(next).length > 0) {
        setCircleMap((prev: Record<string, Circle>) => ({ ...prev, ...next }));
      }
    })().catch((error) => console.error('Failed to hydrate circles', error));
  }, [trackedCircleIds, circleMap, setTrackedCircleIds, setCircleMap, setNotification, isRegistered, near.accountId, near.viewFunction]);

  const participantIds = useMemo(
    () => (selectedCircle ? selectedCircle.members.filter((member: string) => selectedParticipants[member]) : []),
    [selectedCircle, selectedParticipants]
  );

  const handleSignIn = useCallback(async () => {
    try {
      await near.signIn();
    } catch (error) {
      setNotification({ type: 'error', text: (error as Error).message });
    }
  }, [near]);

  const handleSignOut = useCallback(async () => {
    await near.signOut();
    setNotification(null);
    setSelectedCircleId(null);
    // Refresh the page to clear all state
    window.location.reload();
  }, [near]);

  const handleRegister = useCallback(async () => {
    if (!storageBounds.data) {
      setNotification({ type: 'error', text: 'Loading storage requirements...' });
      return;
    }
    try {
      console.log('[Registration] Starting storage deposit...');
      
      // storage_deposit takes optional account_id and registration_only params
      // When account_id is null/undefined, it defaults to the caller
      await registerMutation.execute('storage_deposit', {}, {
        deposit: storageBounds.data.min,
        gas: GAS_150_TGAS
      });
      
      // Note: User will be redirected to wallet, then back to the app
      // The useEffect above will handle the return and check registration status
    } catch (error) {
      console.error('[Registration] Error:', error);
      setNotification({ type: 'error', text: (error as Error).message });
    }
  }, [registerMutation, storageBounds.data]);

  const handleCreateCircle = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!createCircleName.trim()) {
        setNotification({ type: 'error', text: 'Circle name cannot be empty.' });
        return;
      }
      if (usePassword && !createCirclePassword.trim()) {
        setNotification({ type: 'error', text: 'Password cannot be empty when protection is enabled.' });
        return;
      }
      try {
        const args: { name: string; invite_code?: string } = { 
          name: createCircleName.trim() 
        };
        if (usePassword && createCirclePassword.trim()) {
          args.invite_code = createCirclePassword.trim();
        }
        await createCircleMutation.execute('create_circle', args);
        setCreateCircleName('');
        setCreateCirclePassword('');
        setUsePassword(false);
        await memberCircles.mutate();
        setNotification({ type: 'success', text: 'Circle created!' });
      } catch (error) {
        setNotification({ type: 'error', text: (error as Error).message });
      }
    },
    [createCircleName, createCirclePassword, usePassword, createCircleMutation, memberCircles]
  );

  const handleJoinCircle = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      const trimmed = joinCircleId.trim();
      if (!trimmed) {
        setNotification({ type: 'error', text: 'Enter a circle ID to join.' });
        return;
      }
      try {
        const args: { circle_id: string; invite_code?: string } = { 
          circle_id: trimmed 
        };
        if (joinCirclePassword.trim()) {
          args.invite_code = joinCirclePassword.trim();
        }
        await joinCircleMutation.execute('join_circle', args);
        setJoinCircleId('');
        setJoinCirclePassword('');
  setTrackedCircleIds((prev: string[]) => uniq([...prev, trimmed]));
        await memberCircles.mutate();
        setNotification({ type: 'success', text: 'Joined circle successfully.' });
      } catch (error) {
        setNotification({ type: 'error', text: (error as Error).message });
      }
    },
    [joinCircleId, joinCirclePassword, joinCircleMutation, memberCircles, setTrackedCircleIds]
  );

  const handleAddExpense = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!selectedCircleId || !selectedCircle) {
        return;
      }
      if (!expenseAmount || participantIds.length === 0) {
        setNotification({ type: 'error', text: 'Amount and at least one participant are required.' });
        return;
      }
      const amountYocto = parseNearAmount(expenseAmount);
      try {
        const shares = buildEqualShares(participantIds);
        await addExpenseMutation.execute('add_expense', {
          circle_id: selectedCircleId,
          amount_yocto: amountYocto,
          shares,
          memo: expenseMemo.trim()
        });
        setExpenseAmount('');
        setExpenseMemo('');
        await Promise.all([circleExpenses.mutate(), circleBalances.mutate(), circleSuggestions.mutate()]);
        setNotification({ type: 'success', text: 'Expense recorded.' });
      } catch (error) {
        setNotification({ type: 'error', text: (error as Error).message });
      }
    },
    [selectedCircleId, selectedCircle, expenseAmount, participantIds, expenseMemo, addExpenseMutation, circleExpenses, circleBalances, circleSuggestions]
  );

  const handlePayNative = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!selectedCircleId || !settlementRecipient || !settlementAmount) {
        setNotification({ type: 'error', text: 'Recipient and amount are required for settlements.' });
        return;
      }
      try {
        const deposit = parseNearAmount(settlementAmount);
        await payNativeMutation.execute(
          'pay_native',
          {
            circle_id: selectedCircleId,
            to: settlementRecipient
          },
          {
            deposit,
            gas: GAS_150_TGAS
          }
        );
        setSettlementAmount('');
        setNotification({ type: 'success', text: 'Native payment submitted.' });
        await Promise.all([circleBalances.mutate(), circleSuggestions.mutate()]);
      } catch (error) {
        setNotification({ type: 'error', text: (error as Error).message });
      }
    },
    [selectedCircleId, settlementRecipient, settlementAmount, payNativeMutation, circleBalances, circleSuggestions]
  );

  const handleConfirmLedger = useCallback(
    async () => {
      if (!selectedCircleId || !near.accountId) {
        setNotification({ type: 'error', text: 'No circle selected.' });
        return;
      }
      
      try {
        // Calculate required deposit (if user has debt)
        const depositAmount = requiredAutopayDeposit.data && BigInt(requiredAutopayDeposit.data) > 0n 
          ? requiredAutopayDeposit.data 
          : '0';
        
        if (BigInt(depositAmount) > 0n) {
          setNotification({ 
            type: 'success', 
            text: `Confirming with ${formatNearAmount(depositAmount)} Ⓝ escrow deposit...` 
          });
        }
        
        // Confirm ledger (which now automatically enables autopay and handles escrow)
        await confirmLedgerMutation.execute('confirm_ledger', 
          { circle_id: selectedCircleId },
          { 
            deposit: depositAmount,
            gas: GAS_150_TGAS 
          }
        );
        
        setNotification({ type: 'success', text: 'Ledger confirmed! ✓' });
        
        // Refresh all relevant data
        await Promise.all([
          circleConfirmations.mutate(),
          isFullyConfirmed.mutate(),
          allMembersAutopay.mutate(),
          userAutopayStatus.mutate(),
          userEscrowDeposit.mutate()
        ]);
      } catch (error) {
        setNotification({ type: 'error', text: (error as Error).message });
      }
    },
    [
      selectedCircleId, 
      near.accountId, 
      requiredAutopayDeposit.data, 
      confirmLedgerMutation, 
      circleConfirmations, 
      isFullyConfirmed, 
      userAutopayStatus, 
      userEscrowDeposit, 
      allMembersAutopay
    ]
  );

  return (
    <main className="mx-auto flex max-w-7xl flex-col gap-8 px-4 py-8 sm:px-6 lg:px-8">
      {/* Header */}
      <header className="flex flex-col gap-6 rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 shadow-2xl sm:flex-row sm:items-center sm:justify-between">
        <div className="space-y-2">
          <h1 className="bg-gradient-to-r from-white via-gray-100 to-brand-300 bg-clip-text text-3xl font-bold tracking-tight text-transparent sm:text-4xl">
            NearSplitter
          </h1>
          <p className="max-w-2xl text-sm text-gray-400">
            Split expenses on NEAR testnet. Create circles, track expenses, and settle balances seamlessly.
          </p>
        </div>
        <div className="flex h-full items-center gap-3">
          {near.status === 'loading' && <Loader2 className="h-5 w-5 animate-spin text-brand-500" />}
          {near.accountId ? (
            <div className="flex items-center gap-3">
              <span className="rounded-full bg-gray-800 px-5 py-2 text-sm font-medium text-gray-200 ring-1 ring-gray-700 min-w-[200px] text-center">
                {near.accountId}
              </span>
              <Button variant="secondary" onClick={handleSignOut}>
                Sign out
              </Button>
            </div>
          ) : (
            <Button onClick={handleSignIn} className="gap-2 bg-brand-500 hover:bg-brand-600 text-black font-semibold">
              <Wallet className="h-4 w-4" /> Connect wallet
            </Button>
          )}
        </div>
      </header>

      {/* Notification */}
      {notification && (
        <div
          className={`rounded-xl border px-4 py-3 text-sm font-medium shadow-lg ${
            notification.type === 'success'
              ? 'border-brand-500/50 bg-brand-500/10 text-brand-300'
              : 'border-rose-500/50 bg-rose-500/10 text-rose-300'
          }`}
        >
          {notification.text}
        </div>
      )}

      {/* Storage Registration Section - Show only if not registered */}
      {near.accountId && !isRegistered && (
        <section className="rounded-2xl border-2 border-brand-500/50 bg-gradient-to-br from-brand-500/10 to-brand-600/5 p-8 shadow-xl">
          {isCheckingRegistration ? (
            <div className="space-y-4">
              <div className="flex items-center justify-center gap-3 py-8">
                <Loader2 className="h-6 w-6 animate-spin text-brand-400" />
                <div className="text-center">
                  <p className="text-gray-300 font-medium">Checking registration status...</p>
                  <p className="text-xs text-gray-500 mt-1">This may take a few moments after completing registration</p>
                </div>
              </div>
              <div className="flex justify-center">
                <Button
                  onClick={() => {
                    console.log('[Manual Refresh] Forcing registration status check...');
                    storageBalance.mutate();
                  }}
                  className="bg-gray-700 hover:bg-gray-600 text-white text-sm"
                >
                  Retry Check
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex items-start gap-4">
              <div className="rounded-full bg-brand-500/20 p-3">
                <svg className="h-6 w-6 text-brand-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div className="flex-1 space-y-4">
              <div>
                <h2 className="text-xl font-bold text-white">Registration Required</h2>
                <p className="mt-2 text-sm text-gray-300">
                  To use NearSplitter, you need to register once. This one-time deposit covers the storage cost
                  for your account data on the NEAR blockchain.
                </p>
              </div>
              <div className="space-y-2 rounded-lg bg-black/30 p-4 text-sm">
                <div className="flex items-center justify-between">
                  <span className="text-gray-400">Registration status:</span>
                  <span className="font-semibold text-rose-400">Not registered</span>
                </div>
                {storageBounds.data ? (
                  <div className="flex items-center justify-between">
                    <span className="text-gray-400">Required deposit:</span>
                    <span className="font-semibold text-brand-400">{formatNearAmount(storageBounds.data.min)} Ⓝ</span>
                  </div>
                ) : (
                  <div className="flex items-center justify-between">
                    <span className="text-gray-400">Required deposit:</span>
                    <span className="font-semibold text-gray-500">Loading...</span>
                  </div>
                )}
                {near.accountId && (
                  <div className="flex items-center justify-between">
                    <span className="text-gray-400">Your account:</span>
                    <span className="font-semibold text-gray-300 text-xs">{near.accountId}</span>
                  </div>
                )}
                {storageBalance.error && (
                  <div className="rounded bg-red-500/10 border border-red-500/30 p-2 mt-2">
                    <p className="text-xs text-red-400">Error checking registration: {String(storageBalance.error)}</p>
                  </div>
                )}
              </div>
              <Button
                onClick={handleRegister}
                loading={registerMutation.loading}
                disabled={!storageBounds.data || !near.accountId || registerMutation.loading}
                className="bg-brand-500 hover:bg-brand-600 text-black font-semibold disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {registerMutation.loading 
                  ? 'Registering...' 
                  : storageBounds.data 
                    ? `Register now (${formatNearAmount(storageBounds.data.min)} Ⓝ)` 
                    : 'Loading...'}
              </Button>
              {!storageBounds.data && (
                <p className="text-xs text-gray-500">Fetching storage requirements from contract...</p>
              )}
            </div>
          </div>
          )}
        </section>
      )}

      {/* Main Content - Only show if registered */}
      {(!near.accountId || isRegistered) && (
        <>
          {/* Circle Management */}
          <section className="grid gap-6 md:grid-cols-2">
            <div className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 shadow-lg">
              <h2 className="text-xl font-bold text-white">Create Circle</h2>
              <p className="mt-2 text-sm text-gray-400">
                Start a new expense group with friends or colleagues.
              </p>
              <form className="mt-4 space-y-3" onSubmit={handleCreateCircle}>
                <div className="flex gap-2">
                  <Input
                    value={createCircleName}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => setCreateCircleName(event.target.value)}
                    placeholder="Trip to Lisbon"
                    className="flex-1 bg-black/50 border-gray-700 focus:border-brand-500 focus:ring-brand-500/20"
                    required
                  />
                  <Button 
                    type="submit" 
                    loading={createCircleMutation.loading} 
                    disabled={!near.accountId || !isRegistered}
                    className="bg-brand-500 hover:bg-brand-600 text-black font-semibold"
                  >
                    <PlusCircle className="h-4 w-4" />
                  </Button>
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="usePassword"
                    checked={usePassword}
                    onChange={(e) => setUsePassword(e.target.checked)}
                    className="h-4 w-4 rounded border-gray-700 bg-black/50 text-brand-500 focus:ring-brand-500/20"
                  />
                  <label htmlFor="usePassword" className="text-sm text-gray-400">
                    Password protect this circle
                  </label>
                </div>
                {usePassword && (
                  <Input
                    type="password"
                    value={createCirclePassword}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => setCreateCirclePassword(event.target.value)}
                    placeholder="Enter password"
                    className="bg-black/50 border-gray-700 focus:border-brand-500 focus:ring-brand-500/20"
                  />
                )}
              </form>
            </div>

            <div className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 shadow-lg">
              <h2 className="text-xl font-bold text-white">Join Existing Circle</h2>
              <p className="mt-2 text-sm text-gray-400">
                Enter a circle ID to join as a member. You&apos;ll be added to the member list.
              </p>
              <form className="mt-4 space-y-3" onSubmit={handleJoinCircle}>
                <div className="flex gap-2">
                  <Input
                    value={joinCircleId}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => setJoinCircleId(event.target.value)}
                    placeholder="circle-0"
                    className="flex-1 bg-black/50 border-gray-700 focus:border-brand-500 focus:ring-brand-500/20"
                  />
                  <Button 
                    type="submit" 
                    loading={joinCircleMutation.loading} 
                    disabled={!near.accountId || !isRegistered}
                    className="bg-brand-500 hover:bg-brand-600 text-black font-semibold"
                  >
                    Join
                  </Button>
                </div>
                <Input
                  type="password"
                  value={joinCirclePassword}
                  onChange={(event: ChangeEvent<HTMLInputElement>) => setJoinCirclePassword(event.target.value)}
                  placeholder="Password (if required)"
                  className="bg-black/50 border-gray-700 focus:border-brand-500 focus:ring-brand-500/20"
                />
                <p className="text-xs text-gray-500">
                  💡 Tip: Ask the circle owner for the circle ID (shown above the circle name)
                </p>
              </form>
            </div>
          </section>

      <section className="grid gap-6 lg:grid-cols-[300px_1fr]">
        <aside className="space-y-4">
          <div className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-5 shadow-lg">
            <h3 className="text-lg font-bold text-white">Your Circles</h3>
            <p className="mt-1 text-xs text-gray-400">
              Click a circle to view details.
            </p>
            <ul className="mt-4 space-y-2">
              {trackedCircleIds.length === 0 && (
                <li className="text-sm text-gray-500">No circles yet. Create or join one!</li>
              )}
              {trackedCircleIds.map((circleId: string) => {
                const circle = circleMap[circleId];
                return (
                  <li key={circleId}>
                    <div
                      onClick={() => setSelectedCircleId(circleId)}
                      className={`w-full rounded-xl border px-4 py-3 text-left text-sm transition-all cursor-pointer ${
                        selectedCircleId === circleId
                          ? 'border-brand-500 bg-brand-500/10 text-brand-100 shadow-lg shadow-brand-500/20'
                          : 'border-gray-800 bg-gray-900/60 text-gray-200 hover:border-gray-700 hover:bg-gray-900'
                      }`}
                    >
                      <div>
                        <span className="font-semibold truncate">{circle ? circle.name : circleId}</span>
                      </div>
                      {circle && (
                        <p className="mt-1 text-xs text-gray-400">
                          {circle.members.length} members • {formatTimestamp(circle.created_ms)}
                        </p>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
          </div>
        </aside>

        <div className="space-y-6">
          {selectedCircle ? (
            <div className="space-y-6">
              <section className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 shadow-lg">
                <div className="flex flex-col gap-4">
                  <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
                    <div className="flex-1">
                      <h2 className="text-2xl font-bold text-white">{selectedCircle.name}</h2>
                      <div className="mt-2 flex flex-col gap-1 text-sm text-gray-400">
                        <p>Owner: <span className="text-gray-300 break-all">{selectedCircle.owner}</span></p>
                        <p>{selectedCircle.members.length} members</p>
                      </div>
                    </div>
                  </div>
                  
                  {/* Circle ID for sharing */}
                  <div className="rounded-lg border border-gray-700 bg-gray-900/50 p-3">
                    <p className="text-xs font-medium text-gray-400 mb-1">Circle ID (share this with others to join)</p>
                    <div className="flex items-center gap-2">
                      <code className="flex-1 text-sm text-brand-400 font-mono break-all">{selectedCircle.id}</code>
                      <button
                        type="button"
                        onClick={() => {
                          navigator.clipboard.writeText(selectedCircle.id);
                          setNotification({ type: 'success', text: 'Circle ID copied!' });
                        }}
                        className="text-xs px-2 py-1 rounded bg-gray-800 hover:bg-gray-700 text-gray-300 transition-colors flex-shrink-0"
                      >
                        Copy
                      </button>
                    </div>
                  </div>
                </div>

                <div className="mt-6 grid gap-4 md:grid-cols-2">
                  <form onSubmit={handleAddExpense} className="space-y-4 rounded-xl border border-gray-800 bg-black/40 p-5">
                    <h3 className="text-lg font-bold text-white">Add Expense</h3>
                    <div className="space-y-1">
                      <label className="text-xs font-medium text-gray-300">Amount (NEAR)</label>
                      <Input
                        value={expenseAmount}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setExpenseAmount(event.target.value)}
                        placeholder="5.0"
                        type="number"
                        min="0"
                        step="0.01"
                        className="bg-gray-900/50 border-gray-700 focus:border-brand-500 focus:ring-brand-500/20"
                        required
                      />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-medium text-gray-300">Description</label>
                      <Input
                        value={expenseMemo}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setExpenseMemo(event.target.value)}
                        placeholder="Dinner at restaurant"
                        className="bg-gray-900/50 border-gray-700 focus:border-brand-500 focus:ring-brand-500/20"
                      />
                    </div>
                    <div className="space-y-2">
                      <p className="text-xs font-medium text-gray-300">Split between</p>
                      <div className="flex flex-wrap gap-2">
                        {selectedCircle.members.map((member) => (
                          <button
                            key={member}
                            type="button"
                            onClick={() =>
                              setSelectedParticipants((prev: Record<string, boolean>) => ({
                                ...prev,
                                [member]: !prev[member]
                              }))
                            }
                            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-all break-all text-left ${
                              selectedParticipants[member]
                                ? 'bg-brand-500 text-black shadow-md shadow-brand-500/30'
                                : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
                            }`}
                          >
                            {member}
                          </button>
                        ))}
                      </div>
                    </div>
                    <Button
                      type="submit"
                      loading={addExpenseMutation.loading}
                      disabled={participantIds.length === 0 || !expenseAmount}
                      className="w-full bg-brand-500 hover:bg-brand-600 text-black font-semibold"
                    >
                      Record Expense
                    </Button>
                  </form>

                  <form onSubmit={handlePayNative} className="space-y-4 rounded-xl border border-gray-800 bg-black/40 p-5">
                    <h3 className="text-lg font-bold text-white">Settle Payment</h3>
                    <div className="space-y-1">
                      <label className="text-xs font-medium text-gray-300">Pay to</label>
                      <select
                        className="w-full rounded-lg border border-gray-700 bg-gray-900/50 px-3 py-2 text-sm text-gray-100 focus:border-brand-500 focus:ring-brand-500/20"
                        value={settlementRecipient}
                        onChange={(event: ChangeEvent<HTMLSelectElement>) =>
                          setSettlementRecipient(event.target.value)
                        }
                      >
                        <option value="">Select member</option>
                        {selectedCircle.members
                          .filter((member: string) => member !== near.accountId)
                          .map((member: string) => (
                            <option key={member} value={member}>
                              {member}
                            </option>
                          ))}
                      </select>
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-medium text-gray-300">Amount (NEAR)</label>
                      <Input
                        value={settlementAmount}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setSettlementAmount(event.target.value)}
                        placeholder="1.5"
                        type="number"
                        min="0"
                        step="0.01"
                        className="bg-gray-900/50 border-gray-700 focus:border-brand-500 focus:ring-brand-500/20"
                      />
                    </div>
                    <Button
                      type="submit"
                      loading={payNativeMutation.loading}
                      disabled={!settlementRecipient || !settlementAmount}
                      className="w-full bg-brand-500 hover:bg-brand-600 text-black font-semibold"
                    >
                      Send Payment
                    </Button>
                  </form>
                </div>
              </section>

              {/* Ledger Confirmation Section */}
              <section className="rounded-2xl border border-brand-700 bg-gradient-to-br from-brand-950 to-gray-950 p-6 shadow-lg">
                <div className="flex items-start gap-4">
                  <div className="rounded-full bg-brand-500/20 p-3 flex-shrink-0">
                    <svg className="h-6 w-6 text-brand-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                  </div>
                  <div className="flex-1">
                    <h3 className="text-lg font-bold text-white">Confirm Expenses</h3>
                    <p className="mt-2 text-sm text-gray-400">
                      All members must confirm the ledger before settlement. Once everyone confirms, payments will be suggested.
                    </p>
                    
                    {isFullyConfirmed.data ? (
                      <div className="mt-4 rounded-lg border border-brand-500 bg-brand-500/10 p-4">
                        <p className="text-sm font-semibold text-brand-300">
                          ✓ All members have confirmed! Ready for settlement.
                        </p>
                      </div>
                    ) : (
                      <div className="mt-4 space-y-3">
                        <div className="flex items-center gap-2">
                          <span className="text-sm text-gray-300">
                            {circleConfirmations.data?.length || 0} / {selectedCircle?.members.length || 0} confirmed
                          </span>
                          <div className="flex-1 h-2 bg-gray-800 rounded-full overflow-hidden">
                            <div 
                              className="h-full bg-brand-500 transition-all duration-300"
                              style={{ 
                                width: `${selectedCircle ? ((circleConfirmations.data?.length || 0) / selectedCircle.members.length) * 100 : 0}%` 
                              }}
                            />
                          </div>
                        </div>

                        {circleConfirmations.data && circleConfirmations.data.length > 0 && (
                          <div className="rounded-lg bg-gray-900/50 p-3">
                            <p className="text-xs font-medium text-gray-400 mb-2">Confirmed by:</p>
                            <div className="flex flex-wrap gap-2">
                              {circleConfirmations.data.map((accountId: string) => (
                                <span 
                                  key={accountId}
                                  className="px-2 py-1 rounded-md bg-brand-500/20 text-brand-300 text-xs font-medium border border-brand-500/30 break-all"
                                >
                                  {accountId}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}

                        {/* Autopay Section - Automatically enabled when confirming */}
                        {near.accountId && selectedCircle?.members.includes(near.accountId) && !circleConfirmations.data?.includes(near.accountId) && (
                          <div className="space-y-4 rounded-lg border border-gray-800 bg-black/30 p-4">
                            <div className="flex items-start gap-3">
                              <div className="flex-1 space-y-2">
                                <h4 className="text-sm font-semibold text-white">Confirm & Settle</h4>
                                <p className="text-xs text-gray-400">
                                  When you confirm, autopay is automatically enabled. If you owe money, you&apos;ll need to deposit it in escrow.
                                </p>
                              </div>
                            </div>

                            {/* Show user's balance and required deposit */}
                            {circleBalances.data && near.accountId && (
                              <>
                                {(() => {
                                  const userBalance = circleBalances.data.find((b: BalanceView) => b.account_id === near.accountId);
                                  const balance = userBalance ? BigInt(userBalance.net) : 0n;
                                  const required = requiredAutopayDeposit.data ? BigInt(requiredAutopayDeposit.data) : 0n;
                                  const escrowed = userEscrowDeposit.data ? BigInt(userEscrowDeposit.data) : 0n;

                                  return (
                                    <div className="space-y-2 rounded-md bg-gray-900/60 p-3 text-xs">
                                      <div className="flex justify-between">
                                        <span className="text-gray-400">Your balance:</span>
                                        <span className={`font-bold ${balance >= 0n ? 'text-brand-400' : 'text-rose-400'}`}>
                                          {balance >= 0n ? '+' : ''}{formatNearAmount(balance.toString())} Ⓝ
                                        </span>
                                      </div>
                                      
                                      {required > 0n && (
                                        <>
                                          <div className="flex justify-between">
                                            <span className="text-gray-400">Required deposit:</span>
                                            <span className="font-bold text-rose-400">
                                              {formatNearAmount(required.toString())} Ⓝ
                                            </span>
                                          </div>
                                          {escrowed > 0n && (
                                            <div className="flex justify-between">
                                              <span className="text-gray-400">Already deposited:</span>
                                              <span className="font-bold text-brand-400">
                                                {formatNearAmount(escrowed.toString())} Ⓝ
                                              </span>
                                            </div>
                                          )}
                                          <div className="mt-2 rounded border border-amber-500/30 bg-amber-500/10 p-2">
                                            <p className="text-xs text-amber-400">
                                              💡 <strong>{formatNearAmount(required.toString())} Ⓝ</strong> will be deposited when you confirm
                                            </p>
                                          </div>
                                        </>
                                      )}
                                      
                                      {required === 0n && balance >= 0n && (
                                        <p className="text-gray-400">
                                          ✓ No deposit required (you are owed money)
                                        </p>
                                      )}
                                    </div>
                                  );
                                })()}
                              </>
                            )}

                            {/* Show all members autopay status */}
                            {allMembersAutopay.data !== undefined && (
                              <div className={`rounded-md p-2 text-xs ${
                                allMembersAutopay.data 
                                  ? 'bg-brand-500/10 text-brand-400 border border-brand-500/30' 
                                  : 'bg-gray-900/60 text-gray-400'
                              }`}>
                                {allMembersAutopay.data ? (
                                  <span>🚀 All members confirmed! Settlement will be automatic.</span>
                                ) : (
                                  <span>⏳ Waiting for all members to confirm for automatic settlement</span>
                                )}
                              </div>
                            )}
                          </div>
                        )}

                        {near.accountId && selectedCircle?.members.includes(near.accountId) && (
                          <Button
                            onClick={handleConfirmLedger}
                            loading={confirmLedgerMutation.loading}
                            disabled={circleConfirmations.data?.includes(near.accountId)}
                            className={`w-full ${
                              circleConfirmations.data?.includes(near.accountId)
                                ? 'bg-gray-700 cursor-not-allowed'
                                : 'bg-brand-500 hover:bg-brand-600 text-black font-semibold'
                            }`}
                          >
                            {confirmLedgerMutation.loading ? (
                              'Processing...'
                            ) : circleConfirmations.data?.includes(near.accountId) ? (
                              '✓ You have confirmed'
                            ) : (
                              requiredAutopayDeposit.data && BigInt(requiredAutopayDeposit.data) > 0n
                                ? `Confirm & Deposit ${formatNearAmount(requiredAutopayDeposit.data)} Ⓝ`
                                : 'Confirm Ledger'
                            )}
                          </Button>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              </section>

              <section className="grid gap-4 md:grid-cols-2">
                <div className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 shadow-lg">
                  <h3 className="text-lg font-bold text-white">Balances</h3>
                  <p className="text-xs text-gray-400">Positive means they are owed, negative means they owe.</p>
                  <ul className="mt-4 space-y-2 text-sm">
                    {circleBalances.data?.map((balance: BalanceView) => (
                      <li
                        key={balance.account_id}
                        className="flex items-center justify-between gap-3 rounded-lg bg-black/40 px-4 py-3 border border-gray-800"
                      >
                        <span className="text-gray-200 font-medium break-all flex-1">{balance.account_id}</span>
                        <span
                          className={`font-bold whitespace-nowrap ${
                            BigInt(balance.net) >= 0n ? 'text-brand-400' : 'text-rose-400'
                          }`}
                        >
                          {BigInt(balance.net) >= 0n ? '+' : ''}{formatNearAmount(BigInt(balance.net).toString())} Ⓝ
                        </span>
                      </li>
                    )) || <p className="text-xs text-gray-500 py-4">No balances yet.</p>}
                  </ul>
                </div>

                <div className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 shadow-lg">
                  <h3 className="text-lg font-bold text-white">Settlement Suggestions</h3>
                  <p className="text-xs text-gray-400">
                    Minimal transfers to settle all debts. Click to prefill.
                  </p>
                  <ul className="mt-4 space-y-3 text-sm">
                    {circleSuggestions.data && circleSuggestions.data.length > 0 ? (
                      circleSuggestions.data.map((suggestion: SettlementSuggestion, idx: number) => (
                        <li
                          key={`${suggestion.from}-${suggestion.to}-${idx}`}
                          className="flex items-center justify-between gap-3 rounded-lg bg-black/40 px-4 py-3 border border-gray-800"
                        >
                          <div className="flex-1 min-w-0">
                            <p className="font-semibold text-gray-100 break-all">
                              {suggestion.from}
                            </p>
                            <p className="text-xs text-gray-400 mt-0.5">→</p>
                            <p className="font-semibold text-brand-400 break-all">
                              {suggestion.to}
                            </p>
                            <p className="text-xs text-gray-400 mt-1">
                              {formatNearAmount(suggestion.amount)} {suggestion.token ?? 'NEAR'}
                            </p>
                          </div>
                          <button
                            type="button"
                            onClick={() => {
                              setSettlementRecipient(suggestion.to);
                              setSettlementAmount(formatNearAmount(suggestion.amount));
                            }}
                            className="text-xs text-brand-400 hover:text-brand-300 font-medium transition-colors"
                          >
                            Prefill →
                          </button>
                        </li>
                      ))
                    ) : (
                      <p className="text-xs text-gray-500 py-4">No suggestions yet. Add expenses first.</p>
                    )}
                  </ul>
                </div>
              </section>

              <section className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 shadow-lg">
                <h3 className="text-lg font-bold text-white">Recent Expenses</h3>
                <p className="text-xs text-gray-400">All recorded expenses in this circle.</p>
                <div className="mt-4 space-y-3 text-sm">
                  {circleExpenses.data && circleExpenses.data.length > 0 ? (
                    circleExpenses.data.map((expense: Expense) => (
                      <article key={expense.id} className="rounded-xl border border-gray-800 bg-black/40 p-4">
                        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                          <h4 className="font-semibold text-white">{expense.memo || 'Untitled expense'}</h4>
                          <div className="flex items-center gap-2 text-sm">
                            <span className="font-bold text-brand-400">
                              {formatNearAmount(expense.amount_yocto)} Ⓝ
                            </span>
                            <span className="text-gray-500">•</span>
                            <span className="text-gray-400">{formatTimestamp(expense.ts_ms)}</span>
                          </div>
                        </div>
                        <p className="text-xs text-gray-400 mt-1 break-all">Paid by <span className="text-gray-300">{expense.payer}</span></p>
                        <div className="mt-3 flex flex-wrap gap-2">
                          {expense.participants.map((participant) => (
                            <div key={participant.account_id} className="flex items-center gap-2 rounded-lg bg-gray-900/60 px-3 py-1.5 text-xs border border-gray-800">
                              <span className="text-gray-300 break-all">{participant.account_id}</span>
                              <span className="text-gray-500">·</span>
                              <span className="text-brand-400 font-medium whitespace-nowrap">{(participant.weight_bps / 100).toFixed(1)}%</span>
                            </div>
                          ))}
                        </div>
                      </article>
                    ))
                  ) : (
                    <p className="text-xs text-gray-500 py-8 text-center">No expenses yet. Add one to get started!</p>
                  )}
                </div>
              </section>
            </div>
          ) : (
            <div className="rounded-2xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-12 text-center shadow-lg">
              <div className="mx-auto max-w-md space-y-3">
                <div className="mx-auto w-16 h-16 rounded-full bg-brand-500/10 flex items-center justify-center">
                  <svg className="h-8 w-8 text-brand-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                  </svg>
                </div>
                <h3 className="text-lg font-semibold text-white">No Circle Selected</h3>
                <p className="text-sm text-gray-400">
                  Select a circle from the sidebar or create a new one to start tracking expenses.
                </p>
              </div>
            </div>
          )}
        </div>
      </section>
        </>
      )}
    </main>
  );
}
