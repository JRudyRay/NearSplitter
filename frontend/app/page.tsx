'use client';

import { useCallback, useEffect, useMemo, useState, type ChangeEvent, type FormEvent } from 'react';
import { Wallet, HelpCircle, Receipt, Users, DollarSign, TrendingUp, Copy, Check, ArrowRight, Eye, EyeOff } from 'lucide-react';
import Link from 'next/link';
import { useNear } from '@/lib/hooks/use-near';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Logo } from '@/components/ui/logo';
import { AppHeader } from '@/components/ui/app-header';
import { EmptyState } from '@/components/ui/empty-state';
import { ListSkeleton, CircleCardSkeleton } from '@/components/ui/skeleton';
import { TransactionConfirmation } from '@/components/ui/confirmation-modal';
import { useToast } from '@/components/providers/toast-provider';
import { StorageRegistrationSection } from '@/components/home/storage-registration-section';
import { CirclesTab } from '@/components/home/circles-tab';
import { useContractView } from '@/lib/hooks/use-contract-view';
import { useContractCall } from '@/lib/hooks/use-contract-call';
import { useLocalStorage } from '@/lib/hooks/use-local-storage';
import { formatNearAmount, formatTimestamp, parseNearAmount } from '@/lib/utils/format';
import { buildEqualShares, uniq } from '@/lib/utils/shares';
import { getNearConfig } from '@/lib/near/config';
import { 
  validateAmount, 
  validateCircleName, 
  validatePassword, 
  validateMemo, 
  validateCircleId,
  validateRequired,
  sanitizeInput 
} from '@/lib/utils/validation';
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
import type { FinalExecutionOutcome } from 'near-api-js/lib/providers';

export default function HomePage() {
  const near = useNear();
  const config = getNearConfig();
  const contractId = config.contractId;
  const toast = useToast();
  const [trackedKey, setTrackedKey] = useState<string>('nearsplitter:guest:circles');
  const [trackedCircleIds, setTrackedCircleIds] = useLocalStorage<string[]>(trackedKey, []);
  const [circleMap, setCircleMap] = useState<Record<string, Circle>>({});
  const [selectedCircleId, setSelectedCircleId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'circles' | 'expenses' | 'settlements'>('circles');
  const [copiedCircleId, setCopiedCircleId] = useState(false);
  
  // Track when user returns from wallet after a transaction (for seamless registration flow)
  const [isCheckingAfterReturn, setIsCheckingAfterReturn] = useState(() => {
    if (typeof window === 'undefined') return false;
    const urlParams = new URLSearchParams(window.location.search);
    return Boolean(urlParams.get('transactionHashes'));
  });

  const [createCircleName, setCreateCircleName] = useState('');
  const [createCirclePassword, setCreateCirclePassword] = useState('');
  const [showCreatePassword, setShowCreatePassword] = useState(false);
  const [joinCircleId, setJoinCircleId] = useState('');
  const [joinCirclePassword, setJoinCirclePassword] = useState('');
  const [showJoinPassword, setShowJoinPassword] = useState(false);
  const [expenseAmount, setExpenseAmount] = useState('');
  const [expenseMemo, setExpenseMemo] = useState('');
  const [selectedParticipants, setSelectedParticipants] = useState<Record<string, boolean>>({});
  const [settlementAmount, setSettlementAmount] = useState('');
  const [settlementRecipient, setSettlementRecipient] = useState('');
  
  // Password display state
  const [showPassword, setShowPassword] = useState(false);
  const [passwordCopied, setPasswordCopied] = useState(false);

  // Validation errors
  const [validationErrors, setValidationErrors] = useState<Record<string, string>>({});

  // Confirmation modal state
  const [confirmationModal, setConfirmationModal] = useState<{
    isOpen: boolean;
    type: string;
    onConfirm: () => void | Promise<void>;
    details?: Array<{ label: string; value: string }>;
  }>({
    isOpen: false,
    type: '',
    onConfirm: () => {}
  });

  const registerMutation = useContractCall();
  const createCircleMutation = useContractCall();
  const joinCircleMutation = useContractCall();
  const addExpenseMutation = useContractCall();
  const payNativeMutation = useContractCall();
  const confirmLedgerMutation = useContractCall();

  // Copy circle ID helper
  const copyCircleId = useCallback((id: string) => {
    navigator.clipboard.writeText(id);
    setCopiedCircleId(true);
    setTimeout(() => setCopiedCircleId(false), 2000);
    toast.success('Circle ID copied!', { title: 'Copied' });
  }, [toast]);
  
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
  const isCheckingRegistration = near.accountId && (storageBalance.isLoading || isCheckingAfterReturn);
  
  // Debug logging for registration status
  useEffect(() => {
    if (near.accountId) {
      console.log('[Registration] Status:', {
        accountId: near.accountId,
        isRegistered,
        isCheckingRegistration,
        isCheckingAfterReturn,
        storageData: storageBalance.data,
        storageError: storageBalance.error,
        storageBoundsData: storageBounds.data,
        storageBoundsError: storageBounds.error
      });
    }
  }, [near.accountId, isRegistered, isCheckingRegistration, isCheckingAfterReturn, storageBalance.data, storageBalance.error, storageBounds.data, storageBounds.error]);

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
      setIsCheckingAfterReturn(false);
      toast.error(errorMessage || 'Transaction failed. Please try again.', { title: 'Transaction failed' });
      return;
    }
    
    // Handle successful transaction
    if (transactionHashes && near.accountId) {
      console.log('[Transaction Return] Detected transaction completion:', transactionHashes);
      setIsCheckingAfterReturn(true);
      
      // Clear URL parameters to avoid re-triggering
      window.history.replaceState({}, '', window.location.pathname);

      // Provide explorer link (best-effort)
      const firstHash = transactionHashes.split(',')[0];
      const explorerBase = config.explorerUrl || '';
      const explorerTxUrl = explorerBase ? `${explorerBase}/transactions/${firstHash}` : null;
      toast.info('Confirming your registration on-chain...', {
        title: 'Almost there!',
        durationMs: 6_000,
      });
      
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
            setIsCheckingAfterReturn(false);
            
            // Clear any stale circle data from localStorage
            const storageKey = `nearsplitter:${near.accountId}:circles`;
            localStorage.removeItem(storageKey);
            setTrackedCircleIds([]);
            setCircleMap({});
            setSelectedCircleId(null);
            console.log('[Transaction Return] Cleared stale circle data');

            toast.success('You\'re all set! Welcome to NearSplitter.', { 
              title: '✓ Registration Complete',
              actionLabel: explorerTxUrl ? 'View transaction' : undefined,
              actionHref: explorerTxUrl ?? undefined,
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
              setIsCheckingAfterReturn(false);
              
              // Clear any stale circle data from localStorage
              const storageKey = `nearsplitter:${near.accountId}:circles`;
              localStorage.removeItem(storageKey);
              setTrackedCircleIds([]);
              setCircleMap({});
              setSelectedCircleId(null);
              console.log('[Transaction Return] Cleared stale circle data');

              toast.success('You\'re all set! Welcome to NearSplitter.', { 
                title: '✓ Registration Complete',
                actionLabel: explorerTxUrl ? 'View transaction' : undefined,
                actionHref: explorerTxUrl ?? undefined,
              });
            }
          } else if (pollCount >= maxPolls) {
            clearInterval(pollInterval);
            setIsCheckingAfterReturn(false);
            console.warn('[Transaction Return] Polling timed out - triggering revalidation...');
            
            // Force SWR revalidation instead of full page reload
            mutateStorageBalance();
            toast.info('Still confirming... please wait a moment.', { title: 'Finalizing' });
          }
        } catch (err) {
          console.error('[Transaction Return] Poll error:', err);
          
          if (pollCount >= maxPolls) {
            clearInterval(pollInterval);
            setIsCheckingAfterReturn(false);
          }
        }
      }, 500); // Poll every 500ms (much faster!)
      
      // Cleanup on unmount
      return () => {
        clearInterval(pollInterval);
      };
    }
  }, [contractId, mutateStorageBalance, near, near.accountId, near.viewFunction, setCircleMap, setSelectedCircleId, setTrackedCircleIds, toast, config.explorerUrl]);

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
            toast.error(`Unable to load circle ${circleId}`, { title: 'Circle load failed' });
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
  }, [trackedCircleIds, circleMap, setTrackedCircleIds, setCircleMap, isRegistered, near.accountId, near.viewFunction, toast]);

  const participantIds = useMemo(
    () => (selectedCircle ? selectedCircle.members.filter((member: string) => selectedParticipants[member]) : []),
    [selectedCircle, selectedParticipants]
  );

  const handleSignIn = useCallback(async () => {
    try {
      await near.signIn();
    } catch (error) {
      toast.error((error as Error).message, { title: 'Wallet connection failed' });
    }
  }, [near, toast]);

  const handleSignOut = useCallback(async () => {
    await near.signOut();
    setSelectedCircleId(null);
    // Refresh the page to clear all state
    window.location.reload();
  }, [near]);

  const handleRegister = useCallback(async () => {
    if (!storageBounds.data) {
      toast.info('Loading storage requirements…', { title: 'Please wait' });
      return;
    }
    try {
      console.log('[Registration] Starting storage deposit...');

      toast.info('Your wallet will open to approve a one-time storage deposit.', {
        title: 'Registration',
        durationMs: 7_000,
      });
      
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
      toast.error((error as Error).message, { title: 'Registration failed' });
    }
  }, [registerMutation, storageBounds.data, toast]);

  const handleCreateCircle = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      
      // Clear previous errors
      setValidationErrors({});
      
      // Validate inputs
      const nameValidation = validateCircleName(createCircleName);
      const passwordValidation = validatePassword(createCirclePassword);
      
      const errors: Record<string, string> = {};
      if (!nameValidation.isValid) errors.circleName = nameValidation.error!;
      if (!passwordValidation.isValid) errors.circlePassword = passwordValidation.error!;
      
      if (Object.keys(errors).length > 0) {
        setValidationErrors(errors);
        toast.error(Object.values(errors)[0], { title: 'Check the form' });
        return;
      }

      // Sanitize inputs
      const sanitizedName = sanitizeInput(createCircleName.trim());
      const sanitizedPassword = sanitizeInput(createCirclePassword.trim());

      // Show confirmation modal
      setConfirmationModal({
        isOpen: true,
        type: 'create circle',
        details: [
          { label: 'Circle Name', value: sanitizedName },
          { label: 'Password Protected', value: 'Yes' }
        ],
        onConfirm: async () => {
          try {
            toast.info('Check your wallet to approve.', { title: 'Create circle', durationMs: 6_000 });
            const args: { name: string; invite_code?: string } = { 
              name: sanitizedName 
            };
            if (sanitizedPassword) {
              args.invite_code = sanitizedPassword;
            }
            const outcome = await createCircleMutation.execute('create_circle', args) as FinalExecutionOutcome | undefined;
            
            // Extract the returned circle ID from the transaction result
            let newCircleId: string | null = null;
            if (outcome && typeof outcome.status === 'object' && outcome.status !== null) {
              const status = outcome.status as { SuccessValue?: string };
              if (status.SuccessValue) {
                try {
                  const decoded = atob(status.SuccessValue);
                  // The contract returns the circle ID as a JSON string
                  newCircleId = JSON.parse(decoded);
                } catch {
                  // If JSON parse fails, try using raw decoded value
                  newCircleId = atob(status.SuccessValue);
                }
              }
            }
            
            setCreateCircleName('');
            setCreateCirclePassword('');
            
            // Immediately track and select the new circle
            if (newCircleId) {
              // Store password in localStorage for the owner to recall it later
              if (sanitizedPassword) {
                try {
                  const stored = localStorage.getItem('circle_passwords') ? JSON.parse(localStorage.getItem('circle_passwords')!) : {};
                  stored[newCircleId] = sanitizedPassword;
                  localStorage.setItem('circle_passwords', JSON.stringify(stored));
                } catch (e) {
                  console.warn('Could not store circle password:', e);
                }
              }
              
              setTrackedCircleIds((prev: string[]) => uniq([...prev, newCircleId!]));
              setSelectedCircleId(newCircleId);
              
              // Fetch the circle data immediately and add to circleMap
              if (near.viewFunction) {
                try {
                  const newCircle = await getCircle(newCircleId, near.viewFunction);
                  setCircleMap((prev: Record<string, Circle>) => ({ ...prev, [newCircleId!]: newCircle }));
                } catch (fetchError) {
                  console.warn('[CreateCircle] Could not fetch new circle data:', fetchError);
                }
              }
            }
            
            await memberCircles.mutate();
            toast.success(`Circle created successfully!${newCircleId ? ` ID: ${newCircleId}` : ''}`, { title: 'Circle created' });
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            toast.error((error as Error).message, { title: 'Create circle failed' });
            throw error;
          }
        }
      });
    },
    [createCircleName, createCirclePassword, createCircleMutation, memberCircles, setTrackedCircleIds, setSelectedCircleId, near.viewFunction, setCircleMap, toast]
  );

  const handleJoinCircle = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      
      // Clear previous errors
      setValidationErrors({});
      
      // Validate inputs
      const idValidation = validateCircleId(joinCircleId);
      
      if (!idValidation.isValid) {
        setValidationErrors({ circleId: idValidation.error! });
        toast.error(idValidation.error!, { title: 'Check the form' });
        return;
      }

      const trimmed = sanitizeInput(joinCircleId.trim());
      const sanitizedPassword = sanitizeInput(joinCirclePassword.trim());

      // Show confirmation modal
      setConfirmationModal({
        isOpen: true,
        type: 'join circle',
        details: [
          { label: 'Circle ID', value: trimmed }
        ],
        onConfirm: async () => {
          try {
            toast.info('Check your wallet to approve.', { title: 'Join circle', durationMs: 6_000 });
            const args: { circle_id: string; invite_code?: string } = { 
              circle_id: trimmed 
            };
            if (sanitizedPassword) {
              args.invite_code = sanitizedPassword;
            }
            await joinCircleMutation.execute('join_circle', args);
            setJoinCircleId('');
            setJoinCirclePassword('');
            setTrackedCircleIds((prev: string[]) => uniq([...prev, trimmed]));
            setSelectedCircleId(trimmed);
            
            // Fetch the circle data immediately and add to circleMap
            if (near.viewFunction) {
              try {
                const joinedCircle = await getCircle(trimmed, near.viewFunction);
                setCircleMap((prev: Record<string, Circle>) => ({ ...prev, [trimmed]: joinedCircle }));
              } catch (fetchError) {
                console.warn('[JoinCircle] Could not fetch circle data:', fetchError);
              }
            }
            
            await memberCircles.mutate();
            toast.success('Joined circle successfully!', { title: 'Circle joined' });
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            toast.error((error as Error).message, { title: 'Join circle failed' });
            throw error;
          }
        }
      });
    },
    [joinCircleId, joinCirclePassword, joinCircleMutation, memberCircles, setTrackedCircleIds, near.viewFunction, setCircleMap, setSelectedCircleId, toast]
  );

  const handleAddExpense = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      
      if (!selectedCircleId || !selectedCircle) {
        return;
      }

      // Clear previous errors
      setValidationErrors({});

      // Validate inputs
      const amountValidation = validateAmount(expenseAmount);
      const memoValidation = validateMemo(expenseMemo);

      const errors: Record<string, string> = {};
      if (!amountValidation.isValid) errors.expenseAmount = amountValidation.error!;
      if (!memoValidation.isValid) errors.expenseMemo = memoValidation.error!;
      if (participantIds.length === 0) errors.participants = 'Select at least one participant';

      if (Object.keys(errors).length > 0) {
        setValidationErrors(errors);
        toast.error(Object.values(errors)[0], { title: 'Check the form' });
        return;
      }

      const sanitizedMemo = sanitizeInput(expenseMemo.trim());
      const amountYocto = parseNearAmount(expenseAmount);

      // Show confirmation modal
      setConfirmationModal({
        isOpen: true,
        type: 'add expense',
        details: [
          { label: 'Amount', value: `${expenseAmount} Ⓝ` },
          { label: 'Description', value: sanitizedMemo },
          { label: 'Split between', value: `${participantIds.length} member(s)` },
          { label: 'Each pays', value: `${(parseFloat(expenseAmount) / participantIds.length).toFixed(4)} Ⓝ` }
        ],
        onConfirm: async () => {
          try {
            toast.info('Check your wallet to approve.', { title: 'Add expense', durationMs: 6_000 });
            const shares = buildEqualShares(participantIds);
            await addExpenseMutation.execute('add_expense', {
              circle_id: selectedCircleId,
              amount_yocto: amountYocto,
              shares,
              memo: sanitizedMemo
            });
            setExpenseAmount('');
            setExpenseMemo('');
            await Promise.all([circleExpenses.mutate(), circleBalances.mutate(), circleSuggestions.mutate()]);
            toast.success('Expense recorded successfully!', { title: 'Expense added' });
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            toast.error((error as Error).message, { title: 'Add expense failed' });
            throw error;
          }
        }
      });
    },
    [selectedCircleId, selectedCircle, expenseAmount, participantIds, expenseMemo, addExpenseMutation, circleExpenses, circleBalances, circleSuggestions, toast]
  );

  const handlePayNative = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      
      if (!selectedCircleId) {
        return;
      }

      // Clear previous errors
      setValidationErrors({});

      // Validate inputs
      const amountValidation = validateAmount(settlementAmount);
      const recipientValidation = validateRequired(settlementRecipient, 'Recipient');

      const errors: Record<string, string> = {};
      if (!amountValidation.isValid) errors.settlementAmount = amountValidation.error!;
      if (!recipientValidation.isValid) errors.settlementRecipient = recipientValidation.error!;

      if (Object.keys(errors).length > 0) {
        setValidationErrors(errors);
        toast.error(Object.values(errors)[0], { title: 'Check the form' });
        return;
      }

      const deposit = parseNearAmount(settlementAmount);

      // Show confirmation modal
      setConfirmationModal({
        isOpen: true,
        type: 'settle payment',
        details: [
          { label: 'Amount', value: `${settlementAmount} Ⓝ` },
          { label: 'Recipient', value: settlementRecipient }
        ],
        onConfirm: async () => {
          try {
            toast.info('Check your wallet to approve.', { title: 'Payment', durationMs: 6_000 });
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
            toast.success('Payment submitted successfully!', { title: 'Payment sent' });
            await Promise.all([circleBalances.mutate(), circleSuggestions.mutate()]);
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            toast.error((error as Error).message, { title: 'Payment failed' });
            throw error;
          }
        }
      });
    },
    [selectedCircleId, settlementRecipient, settlementAmount, payNativeMutation, circleBalances, circleSuggestions, toast]
  );

  const handleConfirmLedger = useCallback(
    async () => {
      if (!selectedCircleId || !near.accountId) {
        toast.error('No circle selected.', { title: 'Cannot confirm' });
        return;
      }
      
      try {
        // Calculate required deposit (if user has debt)
        const depositAmount = requiredAutopayDeposit.data && BigInt(requiredAutopayDeposit.data) > 0n 
          ? requiredAutopayDeposit.data 
          : '0';
        
        if (BigInt(depositAmount) > 0n) {
          toast.info(`Confirming with ${formatNearAmount(depositAmount)} Ⓝ escrow deposit…`, { title: 'Confirm ledger' });
        }
        
        // Confirm ledger (which now automatically enables autopay and handles escrow)
        await confirmLedgerMutation.execute('confirm_ledger', 
          { circle_id: selectedCircleId },
          { 
            deposit: depositAmount,
            gas: GAS_150_TGAS 
          }
        );

        toast.success('Ledger confirmed! ✓', { title: 'Confirmed' });
        
        // Refresh all relevant data
        await Promise.all([
          circleConfirmations.mutate(),
          isFullyConfirmed.mutate(),
          allMembersAutopay.mutate(),
          userAutopayStatus.mutate(),
          userEscrowDeposit.mutate()
        ]);
      } catch (error) {
        toast.error((error as Error).message, { title: 'Confirm failed' });
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
      allMembersAutopay,
      toast
    ]
  );

  // Landing state (explicit connect)
  if (!near.accountId) {
    return (
      <main className="min-h-screen bg-bg">
        <div className="mx-auto max-w-6xl px-4 py-4 sm:px-6 sm:py-6 flex flex-col gap-6">
          <AppHeader onConnect={handleSignIn} onSignOut={handleSignOut} />

          <section className="near-card p-6 sm:p-8">
            <div className="flex flex-col gap-6">
              <div className="flex items-center justify-center">
                <Logo size="lg" />
              </div>

              <div className="text-center space-y-3">
                <h1 className="text-3xl sm:text-4xl font-extrabold text-fg tracking-tight">
                  Split group expenses — settle automatically on NEAR.
                </h1>
                <p className="text-muted-fg max-w-2xl mx-auto">
                  Create a circle, add expenses, and let the contract compute minimal settlements. Wallet connection is only needed to
                  sign transactions you approve.
                </p>
              </div>

              <div className="flex flex-col sm:flex-row gap-3 items-stretch sm:items-center justify-center">
                <Button
                  onClick={handleSignIn}
                  size="lg"
                  leftIcon={<Wallet className="h-5 w-5" />}
                  rightIcon={<ArrowRight className="h-5 w-5" />}
                  aria-label="Connect NEAR wallet"
                >
                  Connect wallet
                </Button>
                <Link
                  href="/help/"
                  className="inline-flex items-center justify-center gap-2 font-medium px-6 py-3 text-lg rounded-xl bg-muted hover:bg-muted/80 text-fg border border-border hover:border-brand-500/50 transition-all duration-200"
                >
                  <HelpCircle className="h-5 w-5 flex-shrink-0" />
                  How it works
                </Link>
              </div>

              <div className="grid gap-3 sm:grid-cols-3">
                <div className="rounded-2xl border border-border bg-card/60 p-4">
                  <div className="text-sm font-semibold text-fg">What happens on-chain</div>
                  <div className="mt-1 text-sm text-muted-fg">
                    Circles and expenses are recorded by a smart contract. You’ll see a preview before every transaction.
                  </div>
                </div>
                <div className="rounded-2xl border border-border bg-card/60 p-4">
                  <div className="text-sm font-semibold text-fg">Why connect a wallet</div>
                  <div className="mt-1 text-sm text-muted-fg">
                    To create circles, add expenses, and confirm settlements. Browsing doesn’t trigger popups.
                  </div>
                </div>
                <div className="rounded-2xl border border-border bg-card/60 p-4">
                  <div className="text-sm font-semibold text-fg">Testnet note</div>
                  <div className="mt-1 text-sm text-muted-fg">
                    This environment uses testnet. Funds are not real NEAR.
                  </div>
                </div>
              </div>
            </div>
          </section>
        </div>
      </main>
    );
  }

  return (
    <main className="mx-auto flex max-w-6xl flex-col gap-4 px-4 py-4 sm:gap-5 sm:py-6 sm:px-6 min-h-screen">
      <AppHeader onConnect={handleSignIn} onSignOut={handleSignOut} />

      {/* Tabs Navigation - Only show when registered */}
      {near.accountId && isRegistered && (
        <nav
          className="near-card p-2 flex gap-1.5"
          role="tablist"
          aria-label="Main navigation"
        >
          <button
            onClick={() => setActiveTab('circles')}
            role="tab"
            aria-selected={activeTab === 'circles'}
            aria-controls="circles-panel"
            className={`flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-xl font-semibold transition-all duration-200 ${
              activeTab === 'circles'
                ? 'bg-brand-500 text-black shadow-near-glow'
                : 'text-muted-fg hover:text-fg hover:bg-muted/60'
            }`}
          >
            <Users className="w-4 h-4" />
            Circles
          </button>
          <button
            onClick={() => {
              setActiveTab('expenses');
              if (!selectedCircleId && trackedCircleIds.length > 0) {
                setSelectedCircleId(trackedCircleIds[0]);
              }
            }}
            role="tab"
            aria-selected={activeTab === 'expenses'}
            aria-controls="expenses-panel"
            className={`flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-xl font-semibold transition-all duration-200 ${
              activeTab === 'expenses'
                ? 'bg-brand-500 text-black shadow-near-glow'
                : 'text-muted-fg hover:text-fg hover:bg-muted/60'
            }`}
          >
            <Receipt className="w-4 h-4" />
            Expenses
          </button>
          <button
            onClick={() => {
              setActiveTab('settlements');
              if (!selectedCircleId && trackedCircleIds.length > 0) {
                setSelectedCircleId(trackedCircleIds[0]);
              }
            }}
            role="tab"
            aria-selected={activeTab === 'settlements'}
            aria-controls="settlements-panel"
            className={`flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-xl font-semibold transition-all duration-200 ${
              activeTab === 'settlements'
                ? 'bg-brand-500 text-black shadow-near-glow'
                : 'text-muted-fg hover:text-fg hover:bg-muted/60'
            }`}
          >
            <DollarSign className="w-4 h-4" />
            Settle
          </button>
        </nav>
      )}

      {/* Storage Registration Section - Show only if not registered */}
      {near.accountId && !isRegistered && (
        <StorageRegistrationSection
          isCheckingRegistration={Boolean(isCheckingRegistration)}
          isCheckingAfterReturn={isCheckingAfterReturn}
          accountId={near.accountId}
          requiredDepositLabel="Required deposit"
          requiredDepositValue={storageBounds.data ? `${formatNearAmount(storageBounds.data.min)} Ⓝ` : 'Loading…'}
          storageError={storageBalance.error ? String(storageBalance.error) : null}
          onRetryCheck={() => {
            console.log('[Manual Refresh] Forcing registration status check...');
            storageBalance.mutate();
          }}
          onRegister={handleRegister}
          registerLoading={registerMutation.loading}
          disableRegister={!storageBounds.data || !near.accountId || registerMutation.loading}
        />
      )}

      {/* Main Content - Only show if registered */}
      {(!near.accountId || isRegistered) && (
        <>
          {/* Circle Management - Show on Circles tab - Enhanced cards with better visual hierarchy */}
          <CirclesTab
            active={activeTab === 'circles'}
            canSubmit={Boolean(near.accountId && isRegistered)}
            createCircleName={createCircleName}
            setCreateCircleName={setCreateCircleName}
            createCirclePassword={createCirclePassword}
            setCreateCirclePassword={setCreateCirclePassword}
            showCreatePassword={showCreatePassword}
            setShowCreatePassword={setShowCreatePassword}
            joinCircleId={joinCircleId}
            setJoinCircleId={setJoinCircleId}
            joinCirclePassword={joinCirclePassword}
            setJoinCirclePassword={setJoinCirclePassword}
            showJoinPassword={showJoinPassword}
            setShowJoinPassword={setShowJoinPassword}
            validationErrors={validationErrors}
            onCreateCircle={handleCreateCircle}
            onJoinCircle={handleJoinCircle}
            creating={createCircleMutation.loading}
            joining={joinCircleMutation.loading}
          />

      {/* Circles List and Details - Circle list always visible, details shown when circle selected */}
      <section className="grid gap-2.5 lg:grid-cols-[300px_1fr]" aria-label="Circle management">
        <aside className="space-y-2" role="complementary" aria-label="Circle list">
          <nav className={`rounded-xl border border-border/50 bg-gradient-to-br from-card to-muted p-2.5 shadow-xl hover:shadow-xl transition-all duration-300 shadow-near-glow-sm backdrop-blur-sm`}>
            <header className="mb-2">
              <h3 className="text-base sm:text-lg font-bold text-fg flex items-center gap-2">
                <div className={`w-7 h-7 rounded-lg bg-brand-500/10 flex items-center justify-center`}>
                  <Users className={`w-3.5 h-3.5 text-brand-500`} aria-hidden="true" />
                </div>
                Your Circles
              </h3>
              <p className="mt-1 text-sm text-muted-fg">
                {trackedCircleIds.length === 0 
                  ? "No circles yet"
                  : `${trackedCircleIds.length} circle${trackedCircleIds.length === 1 ? '' : 's'}`
                }
              </p>
            </header>
            <ul className="space-y-2" role="list">
              {memberCircles.isLoading ? (
                <li className="mt-2">
                  <CircleCardSkeleton />
                  <div className="mt-2">
                    <CircleCardSkeleton />
                  </div>
                </li>
              ) : trackedCircleIds.length === 0 ? (
                <li className="mt-2">
                  <EmptyState type="circles" />
                </li>
              ) : (
                trackedCircleIds.map((circleId: string) => {
                const circle = circleMap[circleId];
                return (
                  <li key={circleId}>
                    <button
                      onClick={() => setSelectedCircleId(circleId)}
                      className={`w-full rounded-lg border px-3 py-2.5 text-left text-sm transition-all duration-200 cursor-pointer transform ${
                        selectedCircleId === circleId
                          ? `border-brand-500 bg-brand-500/10 text-brand-300 shadow-lg shadow-near-glow scale-[1.02] ring-2 ring-brand-500/20`
                          : `border-border bg-card/60 text-fg hover:border-brand-500/50 hover:bg-card hover:shadow-md hover:scale-[1.01]`
                      }`}
                      aria-pressed={selectedCircleId === circleId}
                      aria-label={`Select ${circle ? circle.name : circleId}`}
                    >
                      <div className="flex items-center gap-2">
                        <div className={`w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 ${
                          selectedCircleId === circleId 
                            ? 'bg-brand-500' 
                            : 'bg-muted'
                        }`}>
                          <Users className={`w-4 h-4 ${
                            selectedCircleId === circleId 
                              ? 'text-black' 
                              : 'text-muted-fg'
                          }`} aria-hidden="true" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <span className="font-semibold block truncate text-sm">{circle ? circle.name : circleId}</span>
                          {circle && (
                            <p className="mt-0.5 text-xs text-muted-fg flex items-center gap-1">
                              <span className="flex items-center gap-0.5">
                                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                  <path d="M9 6a3 3 0 11-6 0 3 3 0 016 0zM17 6a3 3 0 11-6 0 3 3 0 016 0zM12.93 17c.046-.327.07-.66.07-1a6.97 6.97 0 00-1.5-4.33A5 5 0 0119 16v1h-6.07zM6 11a5 5 0 015 5v1H1v-1a5 5 0 015-5z" />
                                </svg>
                                {circle.members.length}
                              </span>
                              <span className="text-border">•</span>
                              <span className="truncate">{formatTimestamp(circle.created_ms)}</span>
                            </p>
                          )}
                        </div>
                      </div>
                    </button>
                  </li>
                );
              })
              )}
            </ul>
          </nav>
        </aside>

        <div className="space-y-3">
          {selectedCircle ? (
            <div className="space-y-3">
              <article className="rounded-xl border border-border/50 bg-gradient-to-br from-card to-muted p-3 shadow-xl hover:shadow-xl transition-all duration-300 backdrop-blur-sm">
                <header className="flex flex-col gap-3">
                  <div className="grid gap-3 lg:grid-cols-[1fr_320px] lg:items-start">
                    <div className="space-y-3">
                      <div>
                        <div className="flex items-center gap-2 mb-1.5">
                          <div className={`w-9 h-9 rounded-lg bg-brand-500 flex items-center justify-center shadow-near-glow`}>
                            <Users className="w-4 h-4 text-black" aria-hidden="true" />
                          </div>
                          <h2 className="text-base sm:text-lg font-bold text-fg">{selectedCircle.name}</h2>
                        </div>
                        <dl className="flex flex-col gap-1 text-sm text-muted-fg">
                          <div className="flex items-center gap-2">
                            <dt className="sr-only">Owner</dt>
                            <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                              <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                            </svg>
                            <dd className="text-fg break-all font-medium">
                              {selectedCircle.owner === near.accountId ? 'You (Owner)' : selectedCircle.owner}
                            </dd>
                          </div>
                          <div className="flex items-center gap-2">
                            <dt className="sr-only">Members</dt>
                            <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                              <path d="M9 6a3 3 0 11-6 0 3 3 0 016 0zM17 6a3 3 0 11-6 0 3 3 0 016 0zM12.93 17c.046-.327.07-.66.07-1a6.97 6.97 0 00-1.5-4.33A5 5 0 0119 16v1h-6.07zM6 11a5 5 0 015 5v1H1v-1a5 5 0 015-5z" />
                            </svg>
                            <dd>{selectedCircle.members.length} member{selectedCircle.members.length === 1 ? '' : 's'}</dd>
                          </div>
                          <div className="flex items-center gap-2">
                            <dt className="sr-only">Created</dt>
                            <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                              <path fillRule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clipRule="evenodd" />
                            </svg>
                            <dd>{formatTimestamp(selectedCircle.created_ms)}</dd>
                          </div>
                          {/* Circle Status Badges */}
                          <div className="flex items-center gap-2 flex-wrap mt-1">
                            {selectedCircle.locked && (
                              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-500/20 text-yellow-400 border border-yellow-500/30">
                                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                                  <path fillRule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clipRule="evenodd" />
                                </svg>
                                Settlement in Progress
                              </span>
                            )}
                            {!selectedCircle.membership_open && !selectedCircle.locked && (
                              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-muted-fg/20 text-muted-fg border border-muted-fg/30">
                                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                                  <path fillRule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clipRule="evenodd" />
                                </svg>
                                Closed to New Members
                              </span>
                            )}
                            {selectedCircle.membership_open && !selectedCircle.locked && (
                              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-500/20 text-green-400 border border-green-500/30">
                                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                                  <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                                </svg>
                                Open for Members
                              </span>
                            )}
                          </div>
                        </dl>
                      </div>

                      {/* Owner Controls - Membership Toggle */}
                      {selectedCircle.owner === near.accountId && !selectedCircle.locked && (
                        <div className={`rounded-lg border border-border/50 bg-card/50 p-2.5 shadow-near-glow-sm backdrop-blur-sm`}>
                          <div className="flex items-center justify-between">
                            <div>
                              <p className="text-sm font-semibold text-fg">Membership</p>
                              <p className="text-xs text-muted-fg">
                                {selectedCircle.membership_open ? 'New members can join' : 'Circle is closed to new members'}
                              </p>
                            </div>
                            <button
                              type="button"
                              onClick={async () => {
                                if (!near.accountId || !near.callFunction) return;
                                try {
                                  toast.info('Check your wallet to approve.', { title: 'Update membership', durationMs: 6_000 });
                                  await near.callFunction({
                                    contractId,
                                    method: 'set_membership_open',
                                    args: { circle_id: selectedCircle.id, open: !selectedCircle.membership_open },
                                    gas: GAS_150_TGAS,
                                    deposit: '0',
                                  });
                                  toast.success(selectedCircle.membership_open ? 'Circle closed to new members' : 'Circle opened for new members', { title: 'Updated' });
                                  // Refresh circle data
                                  if (near.viewFunction) {
                                    const updated = await getCircle(selectedCircle.id, near.viewFunction);
                                    setCircleMap(prev => ({ ...prev, [updated.id]: updated }));
                                  }
                                } catch (err) {
                                  toast.error(`Failed to update membership: ${String(err)}`, { title: 'Update failed' });
                                }
                              }}
                              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-brand-500/20 ${
                                selectedCircle.membership_open ? 'bg-brand-500' : 'bg-muted-fg'
                              }`}
                              aria-pressed={selectedCircle.membership_open}
                              aria-label={selectedCircle.membership_open ? 'Close circle to new members' : 'Open circle to new members'}
                            >
                              <span
                                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                                  selectedCircle.membership_open ? 'translate-x-6' : 'translate-x-1'
                                }`}
                              />
                            </button>
                          </div>
                        </div>
                      )}
                    </div>

                    <div className="space-y-3">
                      {/* Circles + Expenses: Circle ID for sharing + Circle Password (for owner only) */}
                      {activeTab !== 'settlements' && (
                        <div className="space-y-2.5">
                          <div className={`rounded-lg border border-border/50 bg-card/50 p-2.5 shadow-near-glow-sm backdrop-blur-sm`}>
                            <label htmlFor="circle-id-display" className="text-xs font-semibold text-muted-fg mb-1.5 block flex items-center gap-1.5">
                              <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                <path fillRule="evenodd" d="M12.586 4.586a2 2 0 112.828 2.828l-3 3a2 2 0 01-2.828 0 1 1 0 00-1.414 1.414 4 4 0 005.656 0l3-3a4 4 0 00-5.656-5.656l-1.5 1.5a1 1 0 101.414 1.414l1.5-1.5zm-5 5a2 2 0 012.828 0 1 1 0 101.414-1.414 4 4 0 00-5.656 0l-3 3a4 4 0 105.656 5.656l1.5-1.5a1 1 0 10-1.414-1.414l-1.5 1.5a2 2 0 11-2.828-2.828l3-3z" clipRule="evenodd" />
                              </svg>
                              Share Circle ID
                            </label>
                            <div className="flex items-center gap-2">
                              <code id="circle-id-display" className={`flex-1 text-xs text-brand-500 font-mono break-all bg-muted/30 px-2 py-1.5 rounded-lg border border-border`}>
                                {selectedCircle.id}
                              </code>
                              <button
                                type="button"
                                onClick={() => {
                                  copyCircleId(selectedCircle.id);
                                }}
                                className={`text-xs px-2.5 py-1.5 rounded-lg bg-brand-500 hover:opacity-90 text-black font-semibold transition-all duration-200 flex-shrink-0 flex items-center gap-1.5 shadow-near-glow hover:scale-105`}
                                aria-label="Copy circle ID to clipboard"
                              >
                                {copiedCircleId ? (
                                  <>
                                    <Check className="w-3 h-3" aria-hidden="true" />
                                    Copied
                                  </>
                                ) : (
                                  <>
                                    <Copy className="w-3 h-3" aria-hidden="true" />
                                    Copy
                                  </>
                                )}
                              </button>
                            </div>
                          </div>
                          
                          {/* Circle Password - only visible to owner */}
                          {selectedCircle.owner === near.accountId && (() => {
                            const stored = typeof window !== 'undefined' ? localStorage.getItem('circle_passwords') : null;
                            const passwords = stored ? JSON.parse(stored) : {};
                            const circlePassword = passwords[selectedCircle.id];
                            
                            return circlePassword ? (
                              <div className={`rounded-lg border border-amber-500/50 bg-amber-500/10 p-2.5 shadow-near-glow-sm backdrop-blur-sm`}>
                                <label htmlFor="circle-password-display" className="text-xs font-semibold text-amber-600 dark:text-amber-400 mb-1.5 block flex items-center gap-1.5">
                                  <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clipRule="evenodd" />
                                  </svg>
                                  Circle Password (Owner Only)
                                </label>
                                <div className="flex items-center gap-2">
                                  <code id="circle-password-display" className={`flex-1 text-xs font-mono break-all bg-muted/30 px-2 py-1.5 rounded-lg border border-border transition-all ${showPassword ? 'text-amber-600 dark:text-amber-400' : 'text-muted-fg'}`}>
                                    {showPassword ? circlePassword : '•'.repeat(Math.min(circlePassword.length, 12))}
                                  </code>
                                  <button
                                    type="button"
                                    onClick={() => setShowPassword(!showPassword)}
                                    className="text-xs px-2.5 py-1.5 rounded-lg bg-muted hover:bg-border text-fg font-semibold transition-all flex-shrink-0 flex items-center gap-1.5"
                                    aria-label={showPassword ? 'Hide password' : 'Show password'}
                                  >
                                    {showPassword ? (
                                      <EyeOff className="w-3 h-3" aria-hidden="true" />
                                    ) : (
                                      <Eye className="w-3 h-3" aria-hidden="true" />
                                    )}
                                  </button>
                                  <button
                                    type="button"
                                    onClick={() => {
                                      navigator.clipboard.writeText(circlePassword);
                                      setPasswordCopied(true);
                                      setTimeout(() => setPasswordCopied(false), 2000);
                                    }}
                                    className={`text-xs px-2.5 py-1.5 rounded-lg text-black font-semibold transition-all flex-shrink-0 flex items-center gap-1.5 shadow-near-glow hover:scale-105 ${
                                      passwordCopied ? 'bg-brand-500' : 'bg-amber-500 hover:bg-amber-600'
                                    }`}
                                    aria-label="Copy password to clipboard"
                                  >
                                    {passwordCopied ? (
                                      <>
                                        <Check className="w-3 h-3" aria-hidden="true" />
                                        Copied
                                      </>
                                    ) : (
                                      <>
                                        <Copy className="w-3 h-3" aria-hidden="true" />
                                        Copy
                                      </>
                                    )}
                                  </button>
                                </div>
                              </div>
                            ) : null;
                          })()}
                        </div>
                      )}

                      {/* Settle: show balances on the right (no Share Circle ID) */}
                      {activeTab === 'settlements' && (
                        <article className={`rounded-lg border border-border/50 bg-card/50 p-2.5 shadow-near-glow-sm backdrop-blur-sm`}>
                          <header className="flex items-center gap-2 mb-2">
                            <div className={`rounded-lg bg-brand-500/20 p-2 shadow-near-glow-sm`}>
                              <TrendingUp className={`h-4 w-4 text-brand-500`} aria-hidden="true" />
                            </div>
                            <div>
                              <h3 className="text-sm font-bold text-fg">Balances</h3>
                              <p className="text-xs text-muted-fg">
                                {circleBalances.data?.length || 0} member{(circleBalances.data?.length || 0) === 1 ? '' : 's'}
                              </p>
                            </div>
                          </header>
                          <p className="text-xs text-muted-fg mb-2 flex items-center gap-1.5">
                            <svg className="w-3 h-3 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                              <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
                            </svg>
                            Positive = owed, Negative = owes
                          </p>
                          <ul className="space-y-1.5 max-h-64 overflow-auto pr-1" role="list">
                            {circleBalances.data ? (
                              [...circleBalances.data]
                                .sort((a, b) => {
                                  // Put current user first
                                  if (a.account_id === near.accountId) return -1;
                                  if (b.account_id === near.accountId) return 1;
                                  // Then sort by magnitude (who is owed/owes the most)
                                  return Math.abs(Number(b.net)) - Math.abs(Number(a.net));
                                })
                                .map((balance: BalanceView) => (
                              <li
                                key={balance.account_id}
                                className={`flex items-center justify-between gap-3 rounded-lg px-3 py-2 border transition-all duration-200 backdrop-blur-sm ${
                                  balance.account_id === near.accountId
                                    ? 'bg-brand-500/10 border-brand-500/50 shadow-sm'
                                    : 'bg-gradient-to-r from-muted/50 to-card/50 border-border/50 hover:border-border hover:shadow-md'
                                }`}
                              >
                                <div className="flex items-center gap-2 flex-1 min-w-0">
                                  <div className={`w-7 h-7 rounded-lg flex items-center justify-center flex-shrink-0 ${
                                    BigInt(balance.net) >= 0n ? 'bg-brand-500/10' : 'bg-rose-500/10'
                                  }`}>
                                    {BigInt(balance.net) === 0n ? (
                                      <Check className="w-3.5 h-3.5 text-muted-fg" />
                                    ) : (
                                      <svg className={`w-3.5 h-3.5 ${BigInt(balance.net) >= 0n ? 'text-brand-500' : 'text-rose-400'}`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                        <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                                      </svg>
                                    )}
                                  </div>
                                  <span className={`text-sm text-fg font-medium truncate ${balance.account_id === near.accountId ? 'font-bold' : ''}`}>
                                    {balance.account_id === near.accountId ? 'You' : balance.account_id}
                                  </span>
                                </div>
                                <span
                                  className={`font-bold text-sm whitespace-nowrap ${
                                    BigInt(balance.net) > 0n ? 'text-brand-500' : 
                                    BigInt(balance.net) < 0n ? 'text-rose-400' : 'text-muted-fg'
                                  }`}
                                  aria-label={`Balance: ${BigInt(balance.net) >= 0n ? 'owed' : 'owes'} ${formatNearAmount(BigInt(balance.net).toString())} NEAR`}
                                >
                                  {BigInt(balance.net) > 0n ? '+' : ''}{formatNearAmount(BigInt(balance.net).toString())} Ⓝ
                                </span>
                              </li>
                            ))) : <p className="text-xs text-muted-fg py-3 text-center">No balances yet</p>}
                          </ul>
                        </article>
                      )}
                    </div>
                  </div>
                </header>

                {/* EXPENSES TAB: Add Expense Form */}
                {activeTab === 'expenses' && (
                  <div className="mt-2">
                    <form onSubmit={handleAddExpense} className={`space-y-2.5 rounded-xl border border-border/50 bg-gradient-to-br from-muted/60 to-card/60 p-2.5 shadow-xl hover:shadow-xl transition-all duration-300 shadow-near-glow-sm backdrop-blur-sm`}>
                    <header className="flex items-center gap-2">
                      <div className={`rounded-lg bg-brand-500/20 p-2 shadow-near-glow-sm`}>
                        <Receipt className={`h-4 w-4 text-brand-500`} aria-hidden="true" />
                      </div>
                      <h3 className="text-sm sm:text-base font-bold text-fg">Add Expense</h3>
                    </header>
                    <div className="space-y-1.5">
                      <label htmlFor="expense-amount" className="text-sm font-semibold text-fg block">
                        Amount (NEAR)
                      </label>
                      <Input
                        id="expense-amount"
                        value={expenseAmount}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setExpenseAmount(event.target.value)}
                        placeholder="5.0"
                        type="number"
                        min="0"
                        step="0.01"
                        className={`bg-card/50 border-border focus:border-brand-500 focus:ring-brand-500/20 text-sm h-9 transition-all duration-200 hover:border-muted-fg`}
                        required
                        aria-required="true"
                      />
                    </div>
                    <div className="space-y-1.5">
                      <label htmlFor="expense-description" className="text-sm font-semibold text-fg block">
                        Description
                      </label>
                      <Input
                        id="expense-description"
                        value={expenseMemo}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setExpenseMemo(event.target.value)}
                        placeholder="Dinner at restaurant"
                        className={`bg-card/50 border-border focus:border-brand-500 focus:ring-brand-500/20 text-sm h-9 transition-all duration-200 hover:border-muted-fg`}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <p className="text-sm font-semibold text-fg flex items-center gap-1.5">
                        <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                          <path d="M9 6a3 3 0 11-6 0 3 3 0 016 0zM17 6a3 3 0 11-6 0 3 3 0 016 0zM12.93 17c.046-.327.07-.66.07-1a6.97 6.97 0 00-1.5-4.33A5 5 0 0119 16v1h-6.07zM6 11a5 5 0 015 5v1H1v-1a5 5 0 015-5z" />
                        </svg>
                        Split ({Object.values(selectedParticipants).filter(Boolean).length} selected)
                      </p>
                      <div className="flex flex-wrap gap-1.5" role="group" aria-label="Select expense participants">
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
                            className={`rounded-lg px-2.5 py-1.5 text-xs font-medium transition-all duration-200 break-all text-left flex items-center gap-1.5 ${
                              selectedParticipants[member]
                                ? `bg-brand-500 text-black shadow-lg shadow-near-glow hover:scale-105`
                                : 'bg-muted text-fg hover:bg-border hover:scale-105'
                            }`}
                            aria-pressed={selectedParticipants[member]}
                            aria-label={`${selectedParticipants[member] ? 'Remove' : 'Add'} ${member}`}
                          >
                            {selectedParticipants[member] && (
                              <svg className="w-3 h-3 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                              </svg>
                            )}
                            <span className="truncate">{member}</span>
                          </button>
                        ))}
                      </div>
                    </div>
                    <Button
                      type="submit"
                      loading={addExpenseMutation.loading}
                      disabled={participantIds.length === 0 || !expenseAmount}
                      className={`w-full bg-brand-500 hover:bg-brand-600 text-black font-bold text-sm h-9 shadow-near-glow hover:scale-[1.02] transition-all duration-200 shadow-lg`}
                      aria-label="Record expense"
                    >
                      Record Expense
                    </Button>
                  </form>
                  </div>
                )}

              </article>

              {/* SETTLEMENTS TAB ONLY: Ledger Confirmation Section */}
              {activeTab === 'settlements' && (
              <article className={`rounded-xl border border-border/50 bg-gradient-to-br from-card to-muted p-2.5 shadow-xl hover:shadow-xl transition-all duration-300 shadow-near-glow-sm backdrop-blur-sm`}>
                <header className="flex items-start gap-2.5 mb-3">
                  <div className={`rounded-lg bg-brand-500/20 p-2 flex-shrink-0 shadow-near-glow`}>
                    <svg className={`h-4 w-4 text-brand-500`} fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                  </div>
                  <div className="flex-1">
                    <h3 className="text-sm sm:text-base font-bold text-fg">Confirm Expenses</h3>
                    <p className="mt-1 text-xs text-muted-fg">
                      All members must confirm before settlement
                    </p>
                  </div>
                </header>
                
                <div>
                  {isFullyConfirmed.data ? (
                    <div className={`rounded-lg border-2 border-brand-500 bg-brand-500/10 p-3 flex items-start gap-2 shadow-near-glow`}>
                      <svg className={`w-4 h-4 text-brand-500 flex-shrink-0 mt-0.5`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                        <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                      </svg>
                      <p className={`text-lg sm:text-lg font-semibold text-brand-400`}>
                        All members have confirmed! Ready for settlement.
                      </p>
                    </div>
                  ) : (
                    <div className="space-y-2">
                      <div className="space-y-2">
                        <div className="flex items-center gap-3">
                          <span className="text-lg sm:text-lg font-semibold text-fg">
                            {circleConfirmations.data?.length || 0} / {selectedCircle?.members.length || 0} confirmed
                          </span>
                          <div className="flex-1 h-3 bg-muted rounded-full overflow-hidden ring-1 ring-border">
                            <div 
                              className={`h-full bg-brand-500 transition-all duration-500 ease-out shadow-near-glow`}
                              style={{ 
                                width: `${selectedCircle ? ((circleConfirmations.data?.length || 0) / selectedCircle.members.length) * 100 : 0}%` 
                              }}
                              role="progressbar"
                              aria-valuenow={circleConfirmations.data?.length || 0}
                              aria-valuemin={0}
                              aria-valuemax={selectedCircle?.members.length || 0}
                              aria-label="Confirmation progress"
                            />
                          </div>
                        </div>

                        {circleConfirmations.data && circleConfirmations.data.length > 0 && (
                          <div className="rounded-xl bg-card/50 border border-border/50 p-4 backdrop-blur-sm">
                            <p className="text-lg font-semibold text-muted-fg mb-2 flex items-center gap-2">
                              <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                              </svg>
                              Confirmed by:
                            </p>
                            <div className="flex flex-wrap gap-2">
                              {circleConfirmations.data.map((accountId: string) => (
                                <span 
                                  key={accountId}
                                  className={`px-3 py-1.5 rounded-lg bg-brand-500/20 text-brand-400 text-lg sm:text-lg font-medium border border-brand-500/50 break-all flex items-center gap-2`}
                                >
                                  <svg className="w-3 h-3 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                                  </svg>
                                  {accountId}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>

                      {/* Autopay Section - Automatically enabled when confirming */}
                      {near.accountId && selectedCircle?.members.includes(near.accountId) && !circleConfirmations.data?.includes(near.accountId) && (
                          <div className="space-y-3 rounded-xl border border-border/50 bg-muted/30 p-5 backdrop-blur-sm">
                            <div className="flex items-start gap-3">
                              <div className="flex-1 space-y-2">
                                <h4 className="text-lg font-semibold text-fg flex items-center gap-2">
                                  <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M6.267 3.455a3.066 3.066 0 001.745-.723 3.066 3.066 0 013.976 0 3.066 3.066 0 001.745.723 3.066 3.066 0 012.812 2.812c.051.643.304 1.254.723 1.745a3.066 3.066 0 010 3.976 3.066 3.066 0 00-.723 1.745 3.066 3.066 0 01-2.812 2.812 3.066 3.066 0 00-1.745.723 3.066 3.066 0 01-3.976 0 3.066 3.066 0 00-1.745-.723 3.066 3.066 0 01-2.812-2.812 3.066 3.066 0 00-.723-1.745 3.066 3.066 0 010-3.976 3.066 3.066 0 00.723-1.745 3.066 3.066 0 012.812-2.812zm7.44 5.252a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                                  </svg>
                                  Confirm & Settle
                                </h4>
                                <p className="text-base text-muted-fg leading-relaxed">
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
                                    <div className="space-y-3 rounded-lg bg-card/60 border border-border p-4 text-lg">
                                      <div className="flex justify-between items-center">
                                        <span className="text-muted-fg">Your balance:</span>
                                        <span className={`font-bold ${balance >= 0n ? 'text-brand-500' : 'text-rose-400'}`}>
                                          {balance >= 0n ? '+' : ''}{formatNearAmount(balance.toString())} Ⓝ
                                        </span>
                                      </div>
                                      
                                      {required > 0n && (
                                        <>
                                          <div className="flex justify-between items-center">
                                            <span className="text-muted-fg">Required deposit:</span>
                                            <span className="font-bold text-rose-400">
                                              {formatNearAmount(required.toString())} Ⓝ
                                            </span>
                                          </div>
                                          {escrowed > 0n && (
                                            <div className="flex justify-between items-center">
                                              <span className="text-muted-fg">Already deposited:</span>
                                              <span className={`font-bold text-brand-500`}>
                                                {formatNearAmount(escrowed.toString())} Ⓝ
                                              </span>
                                            </div>
                                          )}
                                          <div className={`rounded-lg border-2 border-brand-500 bg-brand-500/10 p-3 shadow-near-glow`}>
                                            <p className={`text-lg text-brand-400 flex items-center gap-2`}>
                                              <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                                <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
                                              </svg>
                                              <strong>{formatNearAmount(required.toString())} Ⓝ</strong> will be deposited when you confirm
                                            </p>
                                          </div>
                                        </>
                                      )}
                                      
                                      {required === 0n && balance >= 0n && (
                                        <p className={`text-fg flex items-center gap-2`}>
                                          <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                            <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                                          </svg>
                                          No deposit required (you are owed money)
                                        </p>
                                      )}
                                    </div>
                                  );
                                })()}
                              </>
                            )}

                            {/* Show all members autopay status */}
                            {allMembersAutopay.data !== undefined && (
                              <div className={`rounded-lg p-3 text-lg flex items-center gap-3 ${
                                allMembersAutopay.data 
                                  ? `bg-brand-500/10 text-brand-500 border-2 border-brand-500 shadow-near-glow` 
                                  : 'bg-card/60 text-muted-fg border border-border'
                              }`}>
                                {allMembersAutopay.data ? (
                                  <>
                                    <svg className="w-5 h-5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                      <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-8.707l-3-3a1 1 0 00-1.414 1.414L10.586 9H7a1 1 0 100 2h3.586l-1.293 1.293a1 1 0 101.414 1.414l3-3a1 1 0 000-1.414z" clipRule="evenodd" />
                                    </svg>
                                    <span className="font-semibold">All members confirmed! Settlement will be automatic.</span>
                                  </>
                                ) : (
                                  <>
                                    <svg className="w-5 h-5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                      <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clipRule="evenodd" />
                                    </svg>
                                    <span>Waiting for all members to confirm for automatic settlement</span>
                                  </>
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
                            className={`w-full h-10 sm:h-11 text-lg sm:text-lg font-semibold transition-all duration-200 ${
                              circleConfirmations.data?.includes(near.accountId)
                                ? 'bg-muted cursor-not-allowed text-muted-fg'
                                : `bg-brand-500 hover:bg-brand-600 text-black shadow-near-glow hover:scale-[1.02] shadow-lg`
                            }`}
                            aria-label={circleConfirmations.data?.includes(near.accountId) ? 'Already confirmed' : 'Confirm ledger'}
                          >
                            {confirmLedgerMutation.loading ? (
                              <span className="flex items-center gap-2 justify-center">
                                <svg className="animate-spin h-5 w-5" fill="none" viewBox="0 0 24 24" aria-hidden="true">
                                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                </svg>
                                Processing...
                              </span>
                            ) : circleConfirmations.data?.includes(near.accountId) ? (
                              <span className="flex items-center gap-2 justify-center">
                                <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                  <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                                </svg>
                                You have confirmed
                              </span>
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
              </article>
              )}

              {/* SETTLEMENTS TAB ONLY: Settle Payment (shown under Confirm Expenses) */}
              {activeTab === 'settlements' && (
                <div className="mt-2">
                  <header className="mb-2 px-1">
                    <h3 className="text-sm font-bold text-fg flex items-center gap-2">
                      <div className="w-1.5 h-1.5 rounded-full bg-brand-500 animate-pulse"></div>
                      Manual Settlement Payment
                    </h3>
                  </header>
                  <form onSubmit={handlePayNative} className={`space-y-3 rounded-xl border border-border/50 bg-gradient-to-br from-muted/60 to-card/60 p-3 shadow-xl hover:shadow-xl transition-all duration-300 shadow-near-glow-sm backdrop-blur-sm`}>
                    <div className="grid grid-cols-[1fr_120px] gap-2 items-start">
                      <div className="space-y-1.5">
                        <label htmlFor="settlement-recipient" className="text-xs font-semibold text-fg block">
                          Pay To
                        </label>
                        <select
                          id="settlement-recipient"
                          className={`w-full rounded-lg border border-border bg-card/60 px-2.5 py-2 text-sm text-fg focus:border-brand-500 focus:ring-1 focus:ring-brand-500/20 h-9 transition-all duration-200 hover:border-muted-fg`}
                          value={settlementRecipient}
                          onChange={(event: ChangeEvent<HTMLSelectElement>) =>
                            setSettlementRecipient(event.target.value)
                          }
                          aria-required="true"
                        >
                          <option value="">Select recipient...</option>
                          {selectedCircle.members
                            .filter((member: string) => member !== near.accountId)
                            .map((member: string) => (
                              <option key={member} value={member}>
                                {member}
                              </option>
                            ))}
                        </select>
                      </div>
                      <div className="space-y-1.5">
                        <label htmlFor="settlement-amount" className="text-xs font-semibold text-fg block">
                          Amount (Ⓝ)
                        </label>
                        <Input
                          id="settlement-amount"
                          value={settlementAmount}
                          onChange={(event: ChangeEvent<HTMLInputElement>) => setSettlementAmount(event.target.value)}
                          placeholder="0.0"
                          type="number"
                          min="0"
                          step="0.01"
                          className={`bg-card/60 border-border focus:border-brand-500 focus:ring-brand-500/20 text-sm h-9 transition-all duration-200 hover:border-muted-fg`}
                          required
                          aria-required="true"
                        />
                      </div>
                    </div>
                    <Button
                      type="submit"
                      loading={payNativeMutation.loading}
                      disabled={!settlementRecipient || !settlementAmount}
                      className={`w-full bg-brand-500 hover:bg-brand-600 text-black font-bold text-sm h-9 shadow-near-glow hover:scale-[1.02] transition-all duration-200 shadow-lg flex items-center justify-center gap-2`}
                      aria-label="Send payment"
                    >
                      <DollarSign className="w-3.5 h-3.5" />
                      Send Payment
                    </Button>
                  </form>
                </div>
              )}

              {/* SETTLEMENTS TAB ONLY: Settlement Suggestions */}
              {activeTab === 'settlements' && (
              <section className={`grid gap-2.5 lg:grid-cols-1`} aria-label="Settlement suggestions">
                <article className={`rounded-xl border border-border/50 bg-gradient-to-br from-card to-muted p-2.5 shadow-xl hover:shadow-xl transition-all duration-300 shadow-near-glow-sm backdrop-blur-sm`}>
                  <header className="flex items-center gap-2 mb-2">
                    <div className={`rounded-lg bg-brand-500/20 p-2 shadow-near-glow-sm`}>
                      <Users className={`h-4 w-4 text-brand-500`} aria-hidden="true" />
                    </div>
                    <div>
                      <h3 className="text-sm sm:text-base font-bold text-fg">Settlement Suggestions</h3>
                      <p className="text-xs text-muted-fg">
                        {circleSuggestions.data?.length || 0} suggested
                      </p>
                    </div>
                  </header>
                  <p className="text-xs text-muted-fg mb-2 flex items-center gap-1.5">
                    <svg className="w-3 h-3 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                      <path fillRule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clipRule="evenodd" />
                    </svg>
                    Minimal transfers to settle debts
                  </p>
                  <ul className="space-y-1.5" role="list">
                    {circleSuggestions.data && circleSuggestions.data.length > 0 ? (
                      circleSuggestions.data.map((suggestion: SettlementSuggestion, idx: number) => (
                        <li
                          key={`${suggestion.from}-${suggestion.to}-${idx}`}
                          className="rounded-xl bg-gradient-to-r from-muted/50 to-card/50 border border-border/50 hover:border-border transition-all duration-200 hover:shadow-lg backdrop-blur-sm overflow-hidden"
                        >
                          <div className="p-5 flex items-center gap-4">
                            <div className="flex-1 min-w-0">
                              <div className="flex items-center gap-2 mb-2">
                                <div className="w-8 h-8 rounded-lg bg-muted flex items-center justify-center flex-shrink-0">
                                  <svg className="w-4 h-4 text-muted-fg" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                                  </svg>
                                </div>
                                <p className="font-semibold text-fg truncate text-lg">
                                  {suggestion.from === near.accountId ? 'You' : suggestion.from}
                                </p>
                              </div>
                              <div className="flex items-center gap-2 my-2">
                                <div className={`flex-1 h-0.5 bg-brand-500 opacity-50`} />
                                <svg className={`w-5 h-5 text-brand-500`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                  <path fillRule="evenodd" d="M10.293 5.293a1 1 0 011.414 0l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414-1.414L12.586 11H5a1 1 0 110-2h7.586l-2.293-2.293a1 1 0 010-1.414z" clipRule="evenodd" />
                                </svg>
                                <div className={`flex-1 h-0.5 bg-brand-500 opacity-50`} />
                              </div>
                              <div className="flex items-center gap-2">
                                <div className={`w-8 h-8 rounded-lg bg-brand-500/10 flex items-center justify-center flex-shrink-0`}>
                                  <svg className={`w-4 h-4 text-brand-500`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                                  </svg>
                                </div>
                                <p className={`font-semibold text-brand-500 truncate text-lg`}>
                                  {suggestion.to === near.accountId ? 'You' : suggestion.to}
                                </p>
                              </div>
                              <div className="mt-2 flex items-center justify-between">
                                <span className="text-lg font-bold text-fg flex items-center gap-1.5">
                                  <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path d="M8.433 7.418c.155-.103.346-.196.567-.267v1.698a2.305 2.305 0 01-.567-.267C8.07 8.34 8 8.114 8 8c0-.114.07-.34.433-.582zM11 12.849v-1.698c.22.071.412.164.567.267.364.243.433.468.433.582 0 .114-.07.34-.433.582a2.305 2.305 0 01-.567.267z" />
                                    <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-13a1 1 0 10-2 0v.092a4.535 4.535 0 00-1.676.662C6.602 6.234 6 7.009 6 8c0 .99.602 1.765 1.324 2.246.48.32 1.054.545 1.676.662v1.941c-.391-.127-.68-.317-.843-.504a1 1 0 10-1.51 1.31c.562.649 1.413 1.076 2.353 1.253V15a1 1 0 102 0v-.092a4.535 4.535 0 001.676-.662C13.398 13.766 14 12.991 14 12c0-.99-.602-1.765-1.324-2.246A4.535 4.535 0 0011 9.092V7.151c.391.127.68.317.843.504a1 1 0 101.511-1.31c-.563-.649-1.413-1.076-2.354-1.253V5z" clipRule="evenodd" />
                                  </svg>
                                  {formatNearAmount(suggestion.amount)} {suggestion.token ?? 'Ⓝ'}
                                </span>
                                {suggestion.from === near.accountId && (
                                  <button
                                    type="button"
                                    onClick={() => {
                                      setSettlementRecipient(suggestion.to);
                                      setSettlementAmount(formatNearAmount(suggestion.amount));
                                    }}
                                    className={`text-lg bg-brand-500 hover:opacity-90 text-black px-4 py-2 rounded-lg font-semibold transition-all duration-200 min-h-[44px] flex items-center gap-2 shadow-near-glow hover:scale-105`}
                                    aria-label="Prefill payment form with this suggestion"
                                  >
                                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7l5 5m0 0l-5 5m5-5H6" />
                                    </svg>
                                    Prefill
                                  </button>
                                )}
                              </div>
                            </div>
                          </div>
                        </li>
                      ))
                    ) : (
                      <li className="py-4 text-center">
                        <EmptyState type="settlements" />
                      </li>
                    )}
                  </ul>
                </article>
              </section>
              )}

              {/* EXPENSES TAB ONLY: Recent Expenses */}
              {activeTab === 'expenses' && (
              <section className={`rounded-xl border border-border bg-gradient-to-br from-card to-muted p-2.5 shadow-lg shadow-near-glow-sm`}>
                <div className="flex items-center gap-2 mb-2">
                  <div className={`rounded-lg bg-brand-500/20 p-1.5 shadow-near-glow-sm`}>
                    <Receipt className={`h-4 w-4 text-brand-500`} />
                  </div>
                  <h3 className="text-sm sm:text-base font-bold text-fg">Recent Expenses</h3>
                </div>
                <p className="text-xs text-muted-fg mb-2">All recorded expenses in this circle</p>
                <div className="space-y-1.5 text-sm">
                  {circleExpenses.isLoading ? (
                    <ListSkeleton count={3} />
                  ) : circleExpenses.data && circleExpenses.data.length > 0 ? (
                    circleExpenses.data.map((expense: Expense) => (
                      <article key={expense.id} className="rounded-lg border border-border bg-muted/40 p-2.5">
                        <div className="flex flex-col gap-1.5 sm:flex-row sm:items-center sm:justify-between">
                          <h4 className="font-semibold text-fg text-sm">{expense.memo || 'Untitled expense'}</h4>
                          <div className="flex items-center gap-1.5 text-xs">
                            <span className={`font-bold text-brand-500`}>
                              {formatNearAmount(expense.amount_yocto)} Ⓝ
                            </span>
                            <span className="text-muted-fg">•</span>
                            <span className="text-muted-fg">{formatTimestamp(expense.ts_ms)}</span>
                          </div>
                        </div>
                        <p className="text-xs text-muted-fg mt-1 truncate">Paid by <span className="text-fg">{expense.payer}</span></p>
                        <div className="mt-1.5 flex flex-wrap gap-1">
                          {expense.participants.map((participant) => (
                            <div key={participant.account_id} className="flex items-center gap-1.5 rounded-md bg-card/60 px-2 py-1 text-xs border border-border">
                              <span className="text-fg truncate max-w-[120px]">{participant.account_id}</span>
                              <span className="text-muted-fg">·</span>
                              <span className={`text-brand-500 font-medium whitespace-nowrap`}>{(participant.weight_bps / 100).toFixed(0)}%</span>
                            </div>
                          ))}
                        </div>
                      </article>
                    ))
                  ) : (
                    <div className="py-3 text-center">
                      <EmptyState type="expenses" />
                    </div>
                  )}
                </div>
              </section>
              )}
            </div>
          ) : (
            <div className="rounded-xl border border-border bg-gradient-to-br from-card to-muted p-4 sm:p-5 text-center shadow-lg">
              <div className="mx-auto max-w-md space-y-2">
                <div className={`mx-auto w-12 h-12 rounded-full bg-brand-500/10 flex items-center justify-center`}>
                  <svg className={`h-6 w-6 text-brand-500`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                  </svg>
                </div>
                <h3 className="text-sm font-semibold text-fg">No Circle Selected</h3>
                <p className="text-xs text-muted-fg">
                  Select a circle from the sidebar or create a new one
                </p>
              </div>
            </div>
          )}
        </div>
      </section>
        </>
      )}

      {/* Transaction Confirmation Modal */}
      <TransactionConfirmation
        isOpen={confirmationModal.isOpen}
        onClose={() => setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} })}
        onConfirm={confirmationModal.onConfirm}
        transactionType={confirmationModal.type}
        additionalDetails={confirmationModal.details}
      />
    </main>
  );
}
