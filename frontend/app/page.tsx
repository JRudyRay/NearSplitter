'use client';

import { useCallback, useEffect, useMemo, useState, type ChangeEvent, type FormEvent } from 'react';
import { Loader2, PlusCircle, Wallet, HelpCircle, Receipt, Users, DollarSign, TrendingUp, Eye, EyeOff } from 'lucide-react';
import { useNear } from '@/lib/hooks/use-near';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Logo } from '@/components/ui/logo';
import { EmptyState } from '@/components/ui/empty-state';
import { CardSkeleton, ListSkeleton, FormSkeleton, CircleCardSkeleton } from '@/components/ui/skeleton';
import { TransactionConfirmation } from '@/components/ui/confirmation-modal';
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
  const [activeTab, setActiveTab] = useState<'circles' | 'expenses' | 'settlements'>('circles');
  const [theme, setTheme] = useLocalStorage<'green' | 'blue' | 'purple' | 'pink'>('nearsplitter:theme', 'green');

  // Theme color mappings
  const themeColors = {
    green: {
      primary: 'emerald',
      gradient: 'from-emerald-500 to-green-400',
      glow: 'shadow-[0_0_20px_rgba(16,185,129,0.5)]',
      glowSm: 'shadow-[0_0_10px_rgba(16,185,129,0.3)]',
      ring: 'ring-emerald-700/50',
      border: 'border-emerald-500',
      borderSoft: 'border-emerald-500/50',
      bg: 'bg-emerald-500',
      bgSoft: 'bg-emerald-500/10',
      bgSofter: 'bg-emerald-500/20',
      text: 'text-emerald-400',
      text300: 'text-emerald-300',
      text100: 'text-emerald-100',
      hover: 'hover:bg-emerald-600',
      hoverBorder: 'hover:border-emerald-500/50',
      focusBorder: 'focus:border-emerald-500',
      focusRing: 'focus:ring-emerald-500/20',
      hoverText: 'hover:text-emerald-300',
      border700: 'border-emerald-700',
      from950: 'from-emerald-950',
      hex: '#10b981',
      hex2: '#06d6a0'
    },
    blue: {
      primary: 'blue',
      gradient: 'from-blue-500 to-cyan-400',
      glow: 'shadow-[0_0_20px_rgba(59,130,246,0.5)]',
      glowSm: 'shadow-[0_0_10px_rgba(59,130,246,0.3)]',
      ring: 'ring-blue-700/50',
      border: 'border-blue-500',
      borderSoft: 'border-blue-500/50',
      bg: 'bg-blue-500',
      bgSoft: 'bg-blue-500/10',
      bgSofter: 'bg-blue-500/20',
      text: 'text-blue-400',
      text300: 'text-blue-300',
      text100: 'text-blue-100',
      hover: 'hover:bg-blue-600',
      hoverBorder: 'hover:border-blue-500/50',
      focusBorder: 'focus:border-blue-500',
      focusRing: 'focus:ring-blue-500/20',
      hoverText: 'hover:text-blue-300',
      border700: 'border-blue-700',
      from950: 'from-blue-950',
      hex: '#3b82f6',
      hex2: '#06b6d4'
    },
    purple: {
      primary: 'purple',
      gradient: 'from-purple-500 to-pink-400',
      glow: 'shadow-[0_0_20px_rgba(168,85,247,0.5)]',
      glowSm: 'shadow-[0_0_10px_rgba(168,85,247,0.3)]',
      ring: 'ring-purple-700/50',
      border: 'border-purple-500',
      borderSoft: 'border-purple-500/50',
      bg: 'bg-purple-500',
      bgSoft: 'bg-purple-500/10',
      bgSofter: 'bg-purple-500/20',
      text: 'text-purple-400',
      text300: 'text-purple-300',
      text100: 'text-purple-100',
      hover: 'hover:bg-purple-600',
      hoverBorder: 'hover:border-purple-500/50',
      focusBorder: 'focus:border-purple-500',
      focusRing: 'focus:ring-purple-500/20',
      hoverText: 'hover:text-purple-300',
      border700: 'border-purple-700',
      from950: 'from-purple-950',
      hex: '#a855f7',
      hex2: '#ec4899'
    },
    pink: {
      primary: 'pink',
      gradient: 'from-pink-500 to-rose-400',
      glow: 'shadow-[0_0_20px_rgba(236,72,153,0.5)]',
      glowSm: 'shadow-[0_0_10px_rgba(236,72,153,0.3)]',
      ring: 'ring-pink-700/50',
      border: 'border-pink-500',
      borderSoft: 'border-pink-500/50',
      bg: 'bg-pink-500',
      bgSoft: 'bg-pink-500/10',
      bgSofter: 'bg-pink-500/20',
      text: 'text-pink-400',
      text300: 'text-pink-300',
      text100: 'text-pink-100',
      hover: 'hover:bg-pink-600',
      hoverBorder: 'hover:border-pink-500/50',
      focusBorder: 'focus:border-pink-500',
      focusRing: 'focus:ring-pink-500/20',
      hoverText: 'hover:text-pink-300',
      border700: 'border-pink-700',
      from950: 'from-pink-950',
      hex: '#ec4899',
      hex2: '#fb7185'
    }
  };

  const currentTheme = themeColors[theme];

  const [createCircleName, setCreateCircleName] = useState('');
  const [createCirclePassword, setCreateCirclePassword] = useState('');
  const [showCreatePassword, setShowCreatePassword] = useState(false);
  const [usePassword, setUsePassword] = useState(true); // Always true now
  const [joinCircleId, setJoinCircleId] = useState('');
  const [joinCirclePassword, setJoinCirclePassword] = useState('');
  const [showJoinPassword, setShowJoinPassword] = useState(false);
  const [expenseAmount, setExpenseAmount] = useState('');
  const [expenseMemo, setExpenseMemo] = useState('');
  const [selectedParticipants, setSelectedParticipants] = useState<Record<string, boolean>>({});
  const [settlementAmount, setSettlementAmount] = useState('');
  const [settlementRecipient, setSettlementRecipient] = useState('');

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
      
      // Clear previous errors
      setValidationErrors({});
      
      // Validate inputs
      const nameValidation = validateCircleName(createCircleName);
      const passwordValidation = usePassword ? validatePassword(createCirclePassword) : { isValid: true };
      
      const errors: Record<string, string> = {};
      if (!nameValidation.isValid) errors.circleName = nameValidation.error!;
      if (!passwordValidation.isValid) errors.circlePassword = passwordValidation.error!;
      
      if (Object.keys(errors).length > 0) {
        setValidationErrors(errors);
        setNotification({ type: 'error', text: Object.values(errors)[0] });
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
          { label: 'Protected', value: usePassword ? 'Yes (password required)' : 'No (public)' }
        ],
        onConfirm: async () => {
          try {
            const args: { name: string; invite_code?: string } = { 
              name: sanitizedName 
            };
            if (usePassword && sanitizedPassword) {
              args.invite_code = sanitizedPassword;
            }
            await createCircleMutation.execute('create_circle', args);
            setCreateCircleName('');
            setCreateCirclePassword('');
            setUsePassword(false);
            await memberCircles.mutate();
            setNotification({ type: 'success', text: 'Circle created successfully!' });
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            setNotification({ type: 'error', text: (error as Error).message });
            throw error;
          }
        }
      });
    },
    [createCircleName, createCirclePassword, usePassword, createCircleMutation, memberCircles]
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
        setNotification({ type: 'error', text: idValidation.error! });
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
            await memberCircles.mutate();
            setNotification({ type: 'success', text: 'Joined circle successfully!' });
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            setNotification({ type: 'error', text: (error as Error).message });
            throw error;
          }
        }
      });
    },
    [joinCircleId, joinCirclePassword, joinCircleMutation, memberCircles, setTrackedCircleIds]
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
        setNotification({ type: 'error', text: Object.values(errors)[0] });
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
            setNotification({ type: 'success', text: 'Expense recorded successfully!' });
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            setNotification({ type: 'error', text: (error as Error).message });
            throw error;
          }
        }
      });
    },
    [selectedCircleId, selectedCircle, expenseAmount, participantIds, expenseMemo, addExpenseMutation, circleExpenses, circleBalances, circleSuggestions]
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
        setNotification({ type: 'error', text: Object.values(errors)[0] });
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
            setNotification({ type: 'success', text: 'Payment submitted successfully!' });
            await Promise.all([circleBalances.mutate(), circleSuggestions.mutate()]);
            setConfirmationModal({ isOpen: false, type: '', onConfirm: () => {} });
          } catch (error) {
            setNotification({ type: 'error', text: (error as Error).message });
            throw error;
          }
        }
      });
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
    <main className="mx-auto flex max-w-6xl flex-col gap-3 px-3 py-3 sm:gap-3 sm:py-4 sm:px-4 lg:px-5 min-h-screen">
      {/* Header - Enhanced with better structure and accessibility */}
      <header 
        className="rounded-xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 shadow-lg overflow-hidden backdrop-blur-sm"
        role="banner"
      >
        {/* Top bar with logo and actions - Improved spacing and alignment */}
        <div className="flex flex-col gap-2 p-3 sm:flex-row sm:items-center sm:justify-between border-b border-gray-800/50">
          <Logo size="md" theme={theme} />
          <div className="flex items-center gap-1.5 sm:gap-2 flex-wrap">
            {/* Theme Selector - Enhanced with better labels and touch targets */}
            <div className="flex items-center gap-1.5 px-2 py-1.5 rounded-lg bg-gray-800/50 border border-gray-700/50 backdrop-blur-sm">
              <span className="text-xs font-medium text-gray-400 hidden sm:inline">Theme:</span>
              <div className="flex gap-1" role="group" aria-label="Theme selector">
                <button
                  onClick={() => setTheme('green')}
                  className={`w-6 h-6 rounded-full bg-gradient-to-br from-emerald-500 to-green-400 transition-all duration-200 hover:scale-105 ${theme === 'green' ? 'ring-2 ring-white scale-110 shadow-lg' : 'opacity-60 hover:opacity-100'}`}
                  title="Neon Green Theme"
                  aria-label="Neon Green Theme"
                  aria-pressed={theme === 'green'}
                />
                <button
                  onClick={() => setTheme('blue')}
                  className={`w-6 h-6 rounded-full bg-gradient-to-br from-blue-500 to-cyan-400 transition-all duration-200 hover:scale-105 ${theme === 'blue' ? 'ring-2 ring-white scale-110 shadow-lg' : 'opacity-60 hover:opacity-100'}`}
                  title="Electric Blue Theme"
                  aria-label="Electric Blue Theme"
                  aria-pressed={theme === 'blue'}
                />
                <button
                  onClick={() => setTheme('purple')}
                  className={`w-6 h-6 rounded-full bg-gradient-to-br from-purple-500 to-pink-400 transition-all duration-200 hover:scale-105 ${theme === 'purple' ? 'ring-2 ring-white scale-110 shadow-lg' : 'opacity-60 hover:opacity-100'}`}
                  title="Cyber Purple Theme"
                  aria-label="Cyber Purple Theme"
                  aria-pressed={theme === 'purple'}
                />
                <button
                  onClick={() => setTheme('pink')}
                  className={`w-6 h-6 rounded-full bg-gradient-to-br from-pink-500 to-rose-400 transition-all duration-200 hover:scale-105 ${theme === 'pink' ? 'ring-2 ring-white scale-110 shadow-lg' : 'opacity-60 hover:opacity-100'}`}
                  title="Hot Pink Theme"
                  aria-label="Hot Pink Theme"
                  aria-pressed={theme === 'pink'}
                />
              </div>
            </div>
            
            <a 
              href="/help"
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-gray-800/50 hover:bg-gray-700 text-gray-300 hover:text-white transition-all duration-200 text-lg font-medium border border-gray-700/50 hover:border-gray-600 hover:shadow-lg"
              aria-label="Help and documentation"
            >
              <HelpCircle className="w-4 h-4" aria-hidden="true" />
              <span className="hidden sm:inline">How to Use</span>
              <span className="sm:hidden">Help</span>
            </a>
            {near.status === 'loading' && (
              <div className="flex items-center gap-2 px-3 py-2" role="status" aria-live="polite">
                <Loader2 className={`h-5 w-5 animate-spin ${currentTheme.text}`} aria-hidden="true" />
                <span className="sr-only">Loading...</span>
              </div>
            )}
            {near.accountId ? (
              <div className="flex items-center gap-2">
                <span 
                  className={`rounded-full bg-gradient-to-r ${theme === 'green' ? 'from-emerald-500/20 to-green-500/20' : theme === 'blue' ? 'from-blue-500/20 to-cyan-500/20' : theme === 'purple' ? 'from-purple-500/20 to-pink-500/20' : 'from-pink-500/20 to-rose-500/20'} px-3 py-1.5 text-lg font-medium text-gray-200 ring-1 ${currentTheme.ring} min-w-[120px] sm:min-w-[160px] text-center ${currentTheme.glowSm} backdrop-blur-sm truncate`}
                  title={near.accountId}
                  aria-label={`Connected as ${near.accountId}`}
                >
                  {near.accountId}
                </span>
                <Button 
                  variant="secondary" 
                  onClick={handleSignOut} 
                  className="whitespace-nowrap text-lg hover:scale-105 transition-transform duration-200"
                  aria-label="Sign out from wallet"
                >
                  Sign out
                </Button>
              </div>
            ) : (
              <Button 
                onClick={handleSignIn} 
                className={`gap-2 bg-gradient-to-r ${currentTheme.gradient} hover:from-${currentTheme.primary}-600 text-black font-bold whitespace-nowrap text-lg ${currentTheme.glow} hover:scale-105 transition-all duration-200`}
                aria-label="Connect NEAR wallet"
              >
                <Wallet className="h-4 w-4" aria-hidden="true" /> Connect wallet
              </Button>
            )}
          </div>
        </div>

        {/* Tabs Navigation - Enhanced with better accessibility and visual feedback */}
        {near.accountId && isRegistered && (
          <nav 
            className="flex gap-1 px-3 sm:px-4 py-2 bg-black/20 backdrop-blur-sm" 
            role="tablist"
            aria-label="Main navigation"
          >
            <button
              onClick={() => setActiveTab('circles')}
              role="tab"
              aria-selected={activeTab === 'circles'}
              aria-controls="circles-panel"
              className={`flex-1 px-3 sm:px-4 py-2 rounded-lg font-semibold text-lg transition-all duration-200 ${
                activeTab === 'circles'
                  ? `bg-gradient-to-r ${currentTheme.gradient} text-black ${currentTheme.glow} shadow-lg scale-[1.02]`
                  : 'text-gray-400 hover:text-white hover:bg-gray-800/50 hover:scale-[1.01]'
              }`}
            >
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
              className={`flex-1 px-3 sm:px-4 py-2 rounded-lg font-semibold text-lg transition-all duration-200 ${
                activeTab === 'expenses'
                  ? `bg-gradient-to-r ${currentTheme.gradient} text-black ${currentTheme.glow} shadow-lg scale-[1.02]`
                  : 'text-gray-400 hover:text-white hover:bg-gray-800/50 hover:scale-[1.01]'
              }`}
            >
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
              className={`flex-1 px-3 sm:px-4 py-2 rounded-lg font-semibold text-lg transition-all duration-200 ${
                activeTab === 'settlements'
                  ? `bg-gradient-to-r ${currentTheme.gradient} text-black ${currentTheme.glow} shadow-lg scale-[1.02]`
                  : 'text-gray-400 hover:text-white hover:bg-gray-800/50 hover:scale-[1.01]'
              }`}
            >
              Settlements
            </button>
          </nav>
        )}
      </header>

      {/* Notification - Enhanced with better animations and accessibility */}
      {notification && (
        <div
          role="alert"
          aria-live="polite"
          className={`rounded-lg border px-4 py-3 text-lg font-medium shadow-lg backdrop-blur-sm animate-in slide-in-from-top-4 duration-300 ${
            notification.type === 'success'
              ? `${currentTheme.borderSoft} ${currentTheme.bgSoft} ${currentTheme.text300} ring-1 ${currentTheme.ring}`
              : 'border-rose-500/50 bg-rose-500/10 text-rose-300 ring-1 ring-rose-700/50'
          }`}
        >
          <div className="flex items-center gap-3">
            {notification.type === 'success' ? (
              <svg className="w-5 h-5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            ) : (
              <svg className="w-5 h-5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            )}
            <span className="flex-1">{notification.text}</span>
            <button
              onClick={() => setNotification(null)}
              className="flex-shrink-0 text-gray-400 hover:text-white transition-colors p-1 rounded hover:bg-white/10"
              aria-label="Dismiss notification"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>
      )}

      {/* Storage Registration Section - Show only if not registered */}
      {near.accountId && !isRegistered && (
        <section className={`rounded-xl border-2 ${currentTheme.borderSoft} bg-gradient-to-br ${currentTheme.bgSoft} to-gray-900/5 p-3 sm:p-4 shadow-xl ${currentTheme.glow}`}>
          {isCheckingRegistration ? (
            <div className="space-y-2">
              <div className="flex items-center justify-center gap-3 py-4">
                <Loader2 className={`h-6 w-6 animate-spin ${currentTheme.text}`} />
                <div className="text-center">
                  <p className="text-gray-300 font-medium text-lg">Checking registration status...</p>
                  <p className="text-lg text-gray-500 mt-1">This may take a few moments after completing registration</p>
                </div>
              </div>
              <div className="flex justify-center">
                <Button
                  onClick={() => {
                    console.log('[Manual Refresh] Forcing registration status check...');
                    storageBalance.mutate();
                  }}
                  className="bg-gray-700 hover:bg-gray-600 text-white text-lg"
                >
                  Retry Check
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex items-start gap-4">
              <div className={`rounded-full ${currentTheme.bgSofter} p-3 ${currentTheme.glowSm}`}>
                <svg className={`h-6 w-6 ${currentTheme.text}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div className="flex-1 space-y-2">
              <div>
                <h2 className="text-2xl font-bold text-white">Registration Required</h2>
                <p className="mt-2 text-lg text-gray-300">
                  To use NearSplitter, you need to register once. This one-time deposit covers the storage cost
                  for your account data on the NEAR blockchain.
                </p>
              </div>
              <div className="space-y-2 rounded-lg bg-black/30 p-4 text-lg">
                <div className="flex items-center justify-between">
                  <span className="text-gray-400">Registration status:</span>
                  <span className="font-semibold text-rose-400">Not registered</span>
                </div>
                {storageBounds.data ? (
                  <div className="flex items-center justify-between">
                    <span className="text-gray-400">Required deposit:</span>
                    <span className={`font-semibold ${currentTheme.text}`}>{formatNearAmount(storageBounds.data.min)} Ⓝ</span>
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
                    <span className="font-semibold text-gray-300 text-lg">{near.accountId}</span>
                  </div>
                )}
                {storageBalance.error && (
                  <div className="rounded bg-red-500/10 border border-red-500/30 p-2 mt-2">
                    <p className="text-lg text-red-400">Error checking registration: {String(storageBalance.error)}</p>
                  </div>
                )}
              </div>
              <Button
                onClick={handleRegister}
                loading={registerMutation.loading}
                disabled={!storageBounds.data || !near.accountId || registerMutation.loading}
                className={`${currentTheme.bg} ${currentTheme.hover} text-black font-semibold disabled:opacity-50 disabled:cursor-not-allowed`}
              >
                {registerMutation.loading 
                  ? 'Registering...' 
                  : storageBounds.data 
                    ? `Register now (${formatNearAmount(storageBounds.data.min)} Ⓝ)` 
                    : 'Loading...'}
              </Button>
              {!storageBounds.data && (
                <p className="text-lg text-gray-500">Fetching storage requirements from contract...</p>
              )}
            </div>
          </div>
          )}
        </section>
      )}

      {/* Main Content - Only show if registered */}
      {(!near.accountId || isRegistered) && (
        <>
          {/* Circle Management - Show on Circles tab - Enhanced cards with better visual hierarchy */}
          <section 
            className={`grid gap-4 md:gap-4 md:grid-cols-2 ${activeTab !== 'circles' ? 'hidden' : ''}`}
            id="circles-panel"
            role="tabpanel"
            aria-labelledby="circles-tab"
          >
            <article className={`rounded-xl border border-gray-800/50 bg-gradient-to-br from-gray-900 to-gray-950 p-3 sm:p-4 shadow-lg hover:shadow-xl transition-all duration-300 ${currentTheme.glowSm} backdrop-blur-sm`}>
              <header className="mb-2">
                <h2 className="text-lg sm:text-xl font-bold text-white flex items-center gap-3">
                  <div className={`w-10 h-10 rounded-xl ${currentTheme.bgSoft} flex items-center justify-center ${currentTheme.glow}`}>
                    <PlusCircle className={`w-5 h-5 ${currentTheme.text}`} aria-hidden="true" />
                  </div>
                  Create Circle
                </h2>
                <p className="mt-3 text-base text-gray-400 leading-relaxed">
                  Start a new expense group with friends or colleagues.
                </p>
              </header>
              <form className="space-y-2" onSubmit={handleCreateCircle}>
                <div className="space-y-2">
                  <label htmlFor="circle-name" className="text-lg font-semibold text-gray-300 block">
                    Circle Name
                  </label>
                  <Input
                    id="circle-name"
                    value={createCircleName}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => setCreateCircleName(event.target.value)}
                    placeholder="Trip to Lisbon"
                    className={`w-full bg-black/50 border-gray-700 ${currentTheme.focusBorder} ${currentTheme.focusRing} text-lg h-12 transition-all duration-200 hover:border-gray-600`}
                    required
                    aria-required="true"
                  />
                </div>
                <div className="space-y-2">
                  <label htmlFor="circle-password" className="text-lg font-semibold text-gray-300 block">
                    Circle Password
                  </label>
                  <div className="relative">
                    <Input
                      id="circle-password"
                      type={showCreatePassword ? "text" : "password"}
                      value={createCirclePassword}
                      onChange={(event: ChangeEvent<HTMLInputElement>) => setCreateCirclePassword(event.target.value)}
                      placeholder="Enter a secure password"
                      className={`w-full bg-black/50 border-gray-700 ${currentTheme.focusBorder} ${currentTheme.focusRing} pr-12 text-lg h-12 transition-all duration-200 hover:border-gray-600`}
                      required
                      aria-required="true"
                    />
                    <button
                      type="button"
                      onClick={() => setShowCreatePassword(!showCreatePassword)}
                      className={`absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 ${currentTheme.hoverText} transition-all duration-200 p-2 rounded-lg hover:bg-white/5 min-w-[44px] min-h-[44px] flex items-center justify-center`}
                      aria-label={showCreatePassword ? "Hide password" : "Show password"}
                    >
                      {showCreatePassword ? <EyeOff className="h-5 w-5" aria-hidden="true" /> : <Eye className="h-5 w-5" aria-hidden="true" />}
                    </button>
                  </div>
                  <p className="text-lg text-gray-500 mt-2 flex items-start gap-2">
                    <svg className="w-3.5 h-3.5 mt-0.5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                      <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
                    </svg>
                    <span>Required - Share this password with circle members</span>
                  </p>
                </div>
                <Button 
                  type="submit" 
                  loading={createCircleMutation.loading} 
                  disabled={!near.accountId || !isRegistered}
                  className={`w-full ${currentTheme.bg} ${currentTheme.hover} text-black font-bold text-lg sm:text-lg h-10 sm:h-11 ${currentTheme.glow} hover:scale-[1.02] transition-all duration-200 shadow-lg`}
                  aria-label="Create new circle"
                >
                  <PlusCircle className="h-5 w-5 mr-2" aria-hidden="true" />
                  Create Circle
                </Button>
              </form>
            </article>

            <article className={`rounded-xl border border-gray-800/50 bg-gradient-to-br from-gray-900 to-gray-950 p-3 sm:p-4 shadow-lg hover:shadow-xl transition-all duration-300 ${currentTheme.glowSm} backdrop-blur-sm`}>
              <header className="mb-2">
                <h2 className="text-lg sm:text-xl font-bold text-white flex items-center gap-3">
                  <div className={`w-10 h-10 rounded-xl ${currentTheme.bgSoft} flex items-center justify-center ${currentTheme.glow}`}>
                    <Users className={`w-5 h-5 ${currentTheme.text}`} aria-hidden="true" />
                  </div>
                  Join Existing Circle
                </h2>
                <p className="mt-3 text-base text-gray-400 leading-relaxed">
                  Enter a circle ID to join as a member. You&apos;ll be added to the member list.
                </p>
              </header>
              <form className="space-y-2" onSubmit={handleJoinCircle}>
                <div className="space-y-2">
                  <label htmlFor="join-circle-id" className="text-lg font-semibold text-gray-300 block">
                    Circle ID
                  </label>
                  <Input
                    id="join-circle-id"
                    value={joinCircleId}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => setJoinCircleId(event.target.value)}
                    placeholder="circle-0"
                    className={`w-full bg-black/50 border-gray-700 ${currentTheme.focusBorder} ${currentTheme.focusRing} text-lg h-12 transition-all duration-200 hover:border-gray-600`}
                    required
                    aria-required="true"
                  />
                </div>
                <div className="space-y-2">
                  <label htmlFor="join-password" className="text-lg font-semibold text-gray-300 block">
                    Password
                  </label>
                  <div className="relative">
                    <Input
                      id="join-password"
                      type={showJoinPassword ? "text" : "password"}
                      value={joinCirclePassword}
                      onChange={(event: ChangeEvent<HTMLInputElement>) => setJoinCirclePassword(event.target.value)}
                      placeholder="Enter circle password"
                      className={`w-full bg-black/50 border-gray-700 ${currentTheme.focusBorder} ${currentTheme.focusRing} pr-12 text-lg h-12 transition-all duration-200 hover:border-gray-600`}
                      required
                      aria-required="true"
                    />
                    <button
                      type="button"
                      onClick={() => setShowJoinPassword(!showJoinPassword)}
                      className={`absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 ${currentTheme.hoverText} transition-all duration-200 p-2 rounded-lg hover:bg-white/5 min-w-[44px] min-h-[44px] flex items-center justify-center`}
                      aria-label={showJoinPassword ? "Hide password" : "Show password"}
                    >
                      {showJoinPassword ? <EyeOff className="h-5 w-5" aria-hidden="true" /> : <Eye className="h-5 w-5" aria-hidden="true" />}
                    </button>
                  </div>
                </div>
                <Button 
                  type="submit" 
                  loading={joinCircleMutation.loading} 
                  disabled={!near.accountId || !isRegistered}
                  className={`w-full ${currentTheme.bg} ${currentTheme.hover} text-black font-bold text-lg sm:text-lg h-10 sm:h-11 ${currentTheme.glow} hover:scale-[1.02] transition-all duration-200 shadow-lg`}
                  aria-label="Join circle"
                >
                  Join Circle
                </Button>
                <div className="flex items-start gap-2 p-3 rounded-xl bg-gray-800/30 border border-gray-700/30">
                  <svg className="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                    <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
                  </svg>
                  <p className="text-lg text-gray-400">
                    <strong className="text-gray-300">Tip:</strong> Ask the circle owner for the circle ID and password
                  </p>
                </div>
              </form>
            </article>
          </section>

      {/* Circles List and Details - Circle list always visible, details shown when circle selected */}
      <section className="grid gap-3 lg:grid-cols-[340px_1fr]" aria-label="Circle management">
        <aside className="space-y-2" role="complementary" aria-label="Circle list">
          <nav className={`rounded-xl border border-gray-800/50 bg-gradient-to-br from-gray-900 to-gray-950 p-3 sm:p-4 shadow-xl hover:shadow-xl transition-all duration-300 ${currentTheme.glowSm} backdrop-blur-sm`}>
            <header className="mb-2">
              <h3 className="text-lg sm:text-xl font-bold text-white flex items-center gap-2">
                <div className={`w-8 h-8 rounded-lg ${currentTheme.bgSoft} flex items-center justify-center`}>
                  <Users className={`w-4 h-4 ${currentTheme.text}`} aria-hidden="true" />
                </div>
                Your Circles
              </h3>
              <p className="mt-2 text-lg text-gray-400">
                {trackedCircleIds.length === 0 
                  ? "No circles yet. Create or join one above."
                  : `${trackedCircleIds.length} circle${trackedCircleIds.length === 1 ? '' : 's'} • Click to view details`
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
                      className={`w-full rounded-xl border px-4 py-3.5 text-left text-lg transition-all duration-200 cursor-pointer transform min-h-[64px] ${
                        selectedCircleId === circleId
                          ? `${currentTheme.border} ${currentTheme.bgSoft} ${currentTheme.text100} shadow-lg ${currentTheme.glow} scale-[1.02] ring-2 ${currentTheme.focusRing.replace('focus:', '')}`
                          : `border-gray-800 bg-gray-900/60 text-gray-200 ${currentTheme.hoverBorder} hover:bg-gray-900 hover:shadow-md hover:scale-[1.01]`
                      }`}
                      aria-pressed={selectedCircleId === circleId}
                      aria-label={`Select ${circle ? circle.name : circleId}`}
                    >
                      <div className="flex items-start gap-3">
                        <div className={`w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 ${
                          selectedCircleId === circleId 
                            ? currentTheme.bg 
                            : 'bg-gray-800'
                        }`}>
                          <Users className={`w-5 h-5 ${
                            selectedCircleId === circleId 
                              ? 'text-black' 
                              : 'text-gray-400'
                          }`} aria-hidden="true" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <span className="font-semibold block truncate">{circle ? circle.name : circleId}</span>
                          {circle && (
                            <p className="mt-1 text-lg text-gray-400 flex items-center gap-1.5">
                              <span className="flex items-center gap-1">
                                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                  <path d="M9 6a3 3 0 11-6 0 3 3 0 016 0zM17 6a3 3 0 11-6 0 3 3 0 016 0zM12.93 17c.046-.327.07-.66.07-1a6.97 6.97 0 00-1.5-4.33A5 5 0 0119 16v1h-6.07zM6 11a5 5 0 015 5v1H1v-1a5 5 0 015-5z" />
                                </svg>
                                {circle.members.length}
                              </span>
                              <span className="text-gray-600">•</span>
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

        <div className="space-y-6">
          {selectedCircle ? (
            <div className="space-y-6">
              <article className="rounded-xl border border-gray-800/50 bg-gradient-to-br from-gray-900 to-gray-950 p-3 sm:p-4 shadow-xl hover:shadow-xl transition-all duration-300 backdrop-blur-sm">
                <header className="flex flex-col gap-4">
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="flex-1">
                      <div className="flex items-center gap-3 mb-2">
                        <div className={`w-12 h-12 rounded-xl ${currentTheme.bg} flex items-center justify-center ${currentTheme.glow}`}>
                          <Users className="w-6 h-6 text-black" aria-hidden="true" />
                        </div>
                        <h2 className="text-lg sm:text-xl font-bold text-white">{selectedCircle.name}</h2>
                      </div>
                      <dl className="flex flex-col gap-2 text-lg text-gray-400">
                        <div className="flex items-center gap-2">
                          <dt className="sr-only">Owner</dt>
                          <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                            <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                          </svg>
                          <dd className="text-gray-300 break-all font-medium">
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
                      </dl>
                    </div>
                  </div>
                  
                  {/* Circle ID for sharing */}
                  <div className={`rounded-xl border border-gray-700/50 bg-gray-900/50 p-4 ${currentTheme.glowSm} backdrop-blur-sm`}>
                    <label htmlFor="circle-id-display" className="text-lg font-semibold text-gray-400 mb-2 block flex items-center gap-2">
                      <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                        <path fillRule="evenodd" d="M12.586 4.586a2 2 0 112.828 2.828l-3 3a2 2 0 01-2.828 0 1 1 0 00-1.414 1.414 4 4 0 005.656 0l3-3a4 4 0 00-5.656-5.656l-1.5 1.5a1 1 0 101.414 1.414l1.5-1.5zm-5 5a2 2 0 012.828 0 1 1 0 101.414-1.414 4 4 0 00-5.656 0l-3 3a4 4 0 105.656 5.656l1.5-1.5a1 1 0 10-1.414-1.414l-1.5 1.5a2 2 0 11-2.828-2.828l3-3z" clipRule="evenodd" />
                      </svg>
                      Circle ID (share this with others to join)
                    </label>
                    <div className="flex items-center gap-3">
                      <code id="circle-id-display" className={`flex-1 text-lg ${currentTheme.text} font-mono break-all bg-black/30 px-3 py-2 rounded-lg border border-gray-800`}>
                        {selectedCircle.id}
                      </code>
                      <button
                        type="button"
                        onClick={() => {
                          navigator.clipboard.writeText(selectedCircle.id);
                          setNotification({ type: 'success', text: 'Circle ID copied!' });
                        }}
                        className={`text-lg px-4 py-2 rounded-lg ${currentTheme.bg} hover:opacity-90 text-black font-semibold transition-all duration-200 flex-shrink-0 min-h-[44px] flex items-center gap-2 ${currentTheme.glow} hover:scale-105`}
                        aria-label="Copy circle ID to clipboard"
                      >
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                        </svg>
                        Copy
                      </button>
                    </div>
                  </div>
                </header>

                {/* EXPENSES TAB: Add Expense Form */}
                {activeTab === 'expenses' && (
                  <div className="mt-2">
                    <form onSubmit={handleAddExpense} className={`space-y-3 rounded-xl border border-gray-800/50 bg-gradient-to-br from-black/60 to-gray-950/60 p-3 sm:p-4 shadow-xl hover:shadow-xl transition-all duration-300 ${currentTheme.glowSm} backdrop-blur-sm`}>
                    <header className="flex items-center gap-3">
                      <div className={`rounded-xl ${currentTheme.bgSofter} p-3 ${currentTheme.glowSm}`}>
                        <Receipt className={`h-6 w-6 ${currentTheme.text}`} aria-hidden="true" />
                      </div>
                      <h3 className="text-lg sm:text-xl font-bold text-white">Add Expense</h3>
                    </header>
                    <div className="space-y-2">
                      <label htmlFor="expense-amount" className="text-lg font-semibold text-gray-300 block">
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
                        className={`bg-gray-900/50 border-gray-700 ${currentTheme.focusBorder} ${currentTheme.focusRing} text-lg h-12 transition-all duration-200 hover:border-gray-600`}
                        required
                        aria-required="true"
                      />
                    </div>
                    <div className="space-y-2">
                      <label htmlFor="expense-description" className="text-lg font-semibold text-gray-300 block">
                        Description
                      </label>
                      <Input
                        id="expense-description"
                        value={expenseMemo}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setExpenseMemo(event.target.value)}
                        placeholder="Dinner at restaurant"
                        className={`bg-gray-900/50 border-gray-700 ${currentTheme.focusBorder} ${currentTheme.focusRing} text-lg h-12 transition-all duration-200 hover:border-gray-600`}
                      />
                    </div>
                    <div className="space-y-2">
                      <p className="text-lg font-semibold text-gray-300 flex items-center gap-2">
                        <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                          <path d="M9 6a3 3 0 11-6 0 3 3 0 016 0zM17 6a3 3 0 11-6 0 3 3 0 016 0zM12.93 17c.046-.327.07-.66.07-1a6.97 6.97 0 00-1.5-4.33A5 5 0 0119 16v1h-6.07zM6 11a5 5 0 015 5v1H1v-1a5 5 0 015-5z" />
                        </svg>
                        Split between ({Object.values(selectedParticipants).filter(Boolean).length} selected)
                      </p>
                      <div className="flex flex-wrap gap-2" role="group" aria-label="Select expense participants">
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
                            className={`rounded-lg px-3 py-2 text-lg font-medium transition-all duration-200 break-all text-left min-h-[44px] flex items-center gap-2 ${
                              selectedParticipants[member]
                                ? `${currentTheme.bg} text-black shadow-lg ${currentTheme.glow} hover:scale-105`
                                : 'bg-gray-800 text-gray-300 hover:bg-gray-700 hover:scale-105'
                            }`}
                            aria-pressed={selectedParticipants[member]}
                            aria-label={`${selectedParticipants[member] ? 'Remove' : 'Add'} ${member}`}
                          >
                            {selectedParticipants[member] && (
                              <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
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
                      className={`w-full ${currentTheme.bg} ${currentTheme.hover} text-black font-bold text-lg sm:text-lg h-10 sm:h-11 ${currentTheme.glow} hover:scale-[1.02] transition-all duration-200 shadow-lg`}
                      aria-label="Record expense"
                    >
                      Record Expense
                    </Button>
                  </form>
                  </div>
                )}

                {/* SETTLEMENTS TAB: Settle Payment Form */}
                {activeTab === 'settlements' && (
                  <div className="mt-2">
                    <form onSubmit={handlePayNative} className={`space-y-3 rounded-xl border border-gray-800/50 bg-gradient-to-br from-black/60 to-gray-950/60 p-3 sm:p-4 shadow-xl hover:shadow-xl transition-all duration-300 ${currentTheme.glowSm} backdrop-blur-sm`}>
                    <header className="flex items-center gap-3">
                      <div className={`rounded-xl ${currentTheme.bgSofter} p-3 ${currentTheme.glowSm}`}>
                        <DollarSign className={`h-6 w-6 ${currentTheme.text}`} aria-hidden="true" />
                      </div>
                      <h3 className="text-lg sm:text-xl font-bold text-white">Settle Payment</h3>
                    </header>
                    <div className="space-y-2">
                      <label htmlFor="settlement-recipient" className="text-lg font-semibold text-gray-300 block">
                        Pay to
                      </label>
                      <select
                        id="settlement-recipient"
                        className={`w-full rounded-lg border border-gray-700 bg-gray-900/50 px-4 py-3 text-lg text-gray-100 ${currentTheme.focusBorder} ${currentTheme.focusRing} h-12 transition-all duration-200 hover:border-gray-600`}
                        value={settlementRecipient}
                        onChange={(event: ChangeEvent<HTMLSelectElement>) =>
                          setSettlementRecipient(event.target.value)
                        }
                        aria-required="true"
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
                    <div className="space-y-2">
                      <label htmlFor="settlement-amount" className="text-lg font-semibold text-gray-300 block">
                        Amount (NEAR)
                      </label>
                      <Input
                        id="settlement-amount"
                        value={settlementAmount}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setSettlementAmount(event.target.value)}
                        placeholder="1.5"
                        type="number"
                        min="0"
                        step="0.01"
                        className={`bg-gray-900/50 border-gray-700 ${currentTheme.focusBorder} ${currentTheme.focusRing} text-lg h-12 transition-all duration-200 hover:border-gray-600`}
                        required
                        aria-required="true"
                      />
                    </div>
                    <Button
                      type="submit"
                      loading={payNativeMutation.loading}
                      disabled={!settlementRecipient || !settlementAmount}
                      className={`w-full ${currentTheme.bg} ${currentTheme.hover} text-black font-bold text-lg sm:text-lg h-10 sm:h-11 ${currentTheme.glow} hover:scale-[1.02] transition-all duration-200 shadow-lg`}
                      aria-label="Send payment"
                    >
                      Send Payment
                    </Button>
                  </form>
                  </div>
                )}
              </article>

              {/* SETTLEMENTS TAB ONLY: Ledger Confirmation Section */}
              {activeTab === 'settlements' && (
              <article className={`rounded-xl border ${currentTheme.border700} bg-gradient-to-br ${currentTheme.from950} to-gray-950 p-3 sm:p-4 shadow-xl hover:shadow-xl transition-all duration-300 backdrop-blur-sm`}>
                <header className="flex items-start gap-4 mb-6">
                  <div className={`rounded-xl ${currentTheme.bgSofter} p-3 flex-shrink-0 ${currentTheme.glow}`}>
                    <svg className={`h-6 w-6 ${currentTheme.text}`} fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                  </div>
                  <div className="flex-1">
                    <h3 className="text-lg sm:text-xl font-bold text-white">Confirm Expenses</h3>
                    <p className="mt-2 text-lg sm:text-base text-gray-400 leading-relaxed">
                      All members must confirm the ledger before settlement. Once everyone confirms, payments will be suggested.
                    </p>
                  </div>
                </header>
                
                <div>
                  {isFullyConfirmed.data ? (
                    <div className={`rounded-xl border-2 ${currentTheme.border} ${currentTheme.bgSoft} p-5 flex items-start gap-3 ${currentTheme.glow}`}>
                      <svg className={`w-6 h-6 ${currentTheme.text} flex-shrink-0 mt-0.5`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                        <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                      </svg>
                      <p className={`text-lg sm:text-lg font-semibold ${currentTheme.text300}`}>
                        All members have confirmed! Ready for settlement.
                      </p>
                    </div>
                  ) : (
                    <div className="space-y-2">
                      <div className="space-y-2">
                        <div className="flex items-center gap-3">
                          <span className="text-lg sm:text-lg font-semibold text-gray-300">
                            {circleConfirmations.data?.length || 0} / {selectedCircle?.members.length || 0} confirmed
                          </span>
                          <div className="flex-1 h-3 bg-gray-800 rounded-full overflow-hidden ring-1 ring-gray-700">
                            <div 
                              className={`h-full ${currentTheme.bg} transition-all duration-500 ease-out ${currentTheme.glow}`}
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
                          <div className="rounded-xl bg-gray-900/50 border border-gray-800/50 p-4 backdrop-blur-sm">
                            <p className="text-lg font-semibold text-gray-400 mb-2 flex items-center gap-2">
                              <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                              </svg>
                              Confirmed by:
                            </p>
                            <div className="flex flex-wrap gap-2">
                              {circleConfirmations.data.map((accountId: string) => (
                                <span 
                                  key={accountId}
                                  className={`px-3 py-1.5 rounded-lg ${currentTheme.bgSofter} ${currentTheme.text300} text-lg sm:text-lg font-medium border ${currentTheme.borderSoft} break-all flex items-center gap-2`}
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
                          <div className="space-y-3 rounded-xl border border-gray-800/50 bg-black/30 p-5 backdrop-blur-sm">
                            <div className="flex items-start gap-3">
                              <div className="flex-1 space-y-2">
                                <h4 className="text-lg font-semibold text-white flex items-center gap-2">
                                  <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M6.267 3.455a3.066 3.066 0 001.745-.723 3.066 3.066 0 013.976 0 3.066 3.066 0 001.745.723 3.066 3.066 0 012.812 2.812c.051.643.304 1.254.723 1.745a3.066 3.066 0 010 3.976 3.066 3.066 0 00-.723 1.745 3.066 3.066 0 01-2.812 2.812 3.066 3.066 0 00-1.745.723 3.066 3.066 0 01-3.976 0 3.066 3.066 0 00-1.745-.723 3.066 3.066 0 01-2.812-2.812 3.066 3.066 0 00-.723-1.745 3.066 3.066 0 010-3.976 3.066 3.066 0 00.723-1.745 3.066 3.066 0 012.812-2.812zm7.44 5.252a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                                  </svg>
                                  Confirm & Settle
                                </h4>
                                <p className="text-base text-gray-400 leading-relaxed">
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
                                    <div className="space-y-3 rounded-lg bg-gray-900/60 border border-gray-800 p-4 text-lg">
                                      <div className="flex justify-between items-center">
                                        <span className="text-gray-400">Your balance:</span>
                                        <span className={`font-bold ${balance >= 0n ? currentTheme.text : 'text-rose-400'}`}>
                                          {balance >= 0n ? '+' : ''}{formatNearAmount(balance.toString())} Ⓝ
                                        </span>
                                      </div>
                                      
                                      {required > 0n && (
                                        <>
                                          <div className="flex justify-between items-center">
                                            <span className="text-gray-400">Required deposit:</span>
                                            <span className="font-bold text-rose-400">
                                              {formatNearAmount(required.toString())} Ⓝ
                                            </span>
                                          </div>
                                          {escrowed > 0n && (
                                            <div className="flex justify-between items-center">
                                              <span className="text-gray-400">Already deposited:</span>
                                              <span className={`font-bold ${currentTheme.text}`}>
                                                {formatNearAmount(escrowed.toString())} Ⓝ
                                              </span>
                                            </div>
                                          )}
                                          <div className={`rounded-lg border-2 ${currentTheme.border} ${currentTheme.bgSoft} p-3 ${currentTheme.glow}`}>
                                            <p className={`text-lg ${currentTheme.text300} flex items-center gap-2`}>
                                              <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                                <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
                                              </svg>
                                              <strong>{formatNearAmount(required.toString())} Ⓝ</strong> will be deposited when you confirm
                                            </p>
                                          </div>
                                        </>
                                      )}
                                      
                                      {required === 0n && balance >= 0n && (
                                        <p className={`text-gray-300 flex items-center gap-2`}>
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
                                  ? `${currentTheme.bgSoft} ${currentTheme.text} border-2 ${currentTheme.border} ${currentTheme.glow}` 
                                  : 'bg-gray-900/60 text-gray-400 border border-gray-800'
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
                                ? 'bg-gray-700 cursor-not-allowed text-gray-400'
                                : `${currentTheme.bg} ${currentTheme.hover} text-black ${currentTheme.glow} hover:scale-[1.02] shadow-lg`
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

              {/* SETTLEMENTS TAB ONLY: Balances & Settlement Suggestions */}
              {activeTab === 'settlements' && (
              <section className={`grid gap-3 lg:grid-cols-2`} aria-label="Balances and settlements">
                <article className={`rounded-xl border border-gray-800/50 bg-gradient-to-br from-gray-900 to-gray-950 p-3 sm:p-4 shadow-xl hover:shadow-xl transition-all duration-300 ${currentTheme.glowSm} backdrop-blur-sm`}>
                  <header className="flex items-center gap-3 mb-2">
                    <div className={`rounded-xl ${currentTheme.bgSofter} p-3 ${currentTheme.glowSm}`}>
                      <TrendingUp className={`h-6 w-6 ${currentTheme.text}`} aria-hidden="true" />
                    </div>
                    <div>
                      <h3 className="text-lg sm:text-xl font-bold text-white">Balances</h3>
                      <p className="text-lg text-gray-400 mt-0.5">
                        {circleBalances.data?.length || 0} member{(circleBalances.data?.length || 0) === 1 ? '' : 's'}
                      </p>
                    </div>
                  </header>
                  <p className="text-lg text-gray-400 mb-2 flex items-center gap-2">
                    <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                      <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
                    </svg>
                    Positive = owed money, Negative = owes money
                  </p>
                  <ul className="space-y-2.5" role="list">
                    {circleBalances.data?.map((balance: BalanceView) => (
                      <li
                        key={balance.account_id}
                        className="flex items-center justify-between gap-4 rounded-xl bg-gradient-to-r from-black/50 to-gray-900/50 px-4 py-3 border border-gray-800/50 hover:border-gray-700 transition-all duration-200 hover:shadow-md backdrop-blur-sm"
                      >
                        <div className="flex items-center gap-3 flex-1 min-w-0">
                          <div className={`w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 ${
                            BigInt(balance.net) >= 0n ? currentTheme.bgSoft : 'bg-rose-500/20'
                          }`}>
                            <svg className={`w-5 h-5 ${BigInt(balance.net) >= 0n ? currentTheme.text : 'text-rose-400'}`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                              <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                            </svg>
                          </div>
                          <span className={`text-gray-200 font-medium break-all ${balance.account_id === near.accountId ? 'font-semibold text-white' : ''}`}>
                            {balance.account_id === near.accountId ? 'You' : balance.account_id}
                          </span>
                        </div>
                        <span
                          className={`font-bold text-lg whitespace-nowrap ${
                            BigInt(balance.net) >= 0n ? currentTheme.text : 'text-rose-400'
                          }`}
                          aria-label={`Balance: ${BigInt(balance.net) >= 0n ? 'owed' : 'owes'} ${formatNearAmount(BigInt(balance.net).toString())} NEAR`}
                        >
                          {BigInt(balance.net) >= 0n ? '+' : ''}{formatNearAmount(BigInt(balance.net).toString())} Ⓝ
                        </span>
                      </li>
                    )) || <p className="text-lg text-gray-500 py-4 text-center">No balances yet. Add expenses to get started.</p>}
                  </ul>
                </article>

                <article className={`rounded-xl border border-gray-800/50 bg-gradient-to-br from-gray-900 to-gray-950 p-3 sm:p-4 shadow-xl hover:shadow-xl transition-all duration-300 ${currentTheme.glowSm} backdrop-blur-sm`}>
                  <header className="flex items-center gap-3 mb-2">
                    <div className={`rounded-xl ${currentTheme.bgSofter} p-3 ${currentTheme.glowSm}`}>
                      <Users className={`h-6 w-6 ${currentTheme.text}`} aria-hidden="true" />
                    </div>
                    <div>
                      <h3 className="text-lg sm:text-xl font-bold text-white">Settlement Suggestions</h3>
                      <p className="text-lg text-gray-400 mt-0.5">
                        {circleSuggestions.data?.length || 0} suggested transfer{(circleSuggestions.data?.length || 0) === 1 ? '' : 's'}
                      </p>
                    </div>
                  </header>
                  <p className="text-lg text-gray-400 mb-2 flex items-center gap-2">
                    <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                      <path fillRule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clipRule="evenodd" />
                    </svg>
                    Minimal transfers to settle all debts
                  </p>
                  <ul className="space-y-2" role="list">
                    {circleSuggestions.data && circleSuggestions.data.length > 0 ? (
                      circleSuggestions.data.map((suggestion: SettlementSuggestion, idx: number) => (
                        <li
                          key={`${suggestion.from}-${suggestion.to}-${idx}`}
                          className="rounded-xl bg-gradient-to-r from-black/50 to-gray-900/50 border border-gray-800/50 hover:border-gray-700 transition-all duration-200 hover:shadow-lg backdrop-blur-sm overflow-hidden"
                        >
                          <div className="p-5 flex items-center gap-4">
                            <div className="flex-1 min-w-0">
                              <div className="flex items-center gap-2 mb-2">
                                <div className="w-8 h-8 rounded-lg bg-gray-800 flex items-center justify-center flex-shrink-0">
                                  <svg className="w-4 h-4 text-gray-400" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                                  </svg>
                                </div>
                                <p className="font-semibold text-gray-100 truncate text-lg">
                                  {suggestion.from === near.accountId ? 'You' : suggestion.from}
                                </p>
                              </div>
                              <div className="flex items-center gap-2 my-2">
                                <div className={`flex-1 h-0.5 ${currentTheme.bg} opacity-50`} />
                                <svg className={`w-5 h-5 ${currentTheme.text}`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                  <path fillRule="evenodd" d="M10.293 5.293a1 1 0 011.414 0l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414-1.414L12.586 11H5a1 1 0 110-2h7.586l-2.293-2.293a1 1 0 010-1.414z" clipRule="evenodd" />
                                </svg>
                                <div className={`flex-1 h-0.5 ${currentTheme.bg} opacity-50`} />
                              </div>
                              <div className="flex items-center gap-2">
                                <div className={`w-8 h-8 rounded-lg ${currentTheme.bgSoft} flex items-center justify-center flex-shrink-0`}>
                                  <svg className={`w-4 h-4 ${currentTheme.text}`} fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                    <path fillRule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clipRule="evenodd" />
                                  </svg>
                                </div>
                                <p className={`font-semibold ${currentTheme.text} truncate text-lg`}>
                                  {suggestion.to === near.accountId ? 'You' : suggestion.to}
                                </p>
                              </div>
                              <div className="mt-2 flex items-center justify-between">
                                <span className="text-lg font-bold text-white flex items-center gap-1.5">
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
                                    className={`text-lg ${currentTheme.bg} hover:opacity-90 text-black px-4 py-2 rounded-lg font-semibold transition-all duration-200 min-h-[44px] flex items-center gap-2 ${currentTheme.glow} hover:scale-105`}
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
              <section className={`rounded-xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-3 sm:p-4 shadow-lg ${currentTheme.glowSm}`}>
                <div className="flex items-center gap-2 mb-2">
                  <div className={`rounded-lg ${currentTheme.bgSofter} p-2 ${currentTheme.glowSm}`}>
                    <Receipt className={`h-5 w-5 ${currentTheme.text}`} />
                  </div>
                  <h3 className="text-xl font-bold text-white">Recent Expenses</h3>
                </div>
                <p className="text-lg text-gray-400">All recorded expenses in this circle.</p>
                <div className="mt-2 space-y-2 text-lg">
                  {circleExpenses.isLoading ? (
                    <ListSkeleton count={3} />
                  ) : circleExpenses.data && circleExpenses.data.length > 0 ? (
                    circleExpenses.data.map((expense: Expense) => (
                      <article key={expense.id} className="rounded-xl border border-gray-800 bg-black/40 p-4">
                        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                          <h4 className="font-semibold text-white">{expense.memo || 'Untitled expense'}</h4>
                          <div className="flex items-center gap-2 text-lg">
                            <span className={`font-bold ${currentTheme.text}`}>
                              {formatNearAmount(expense.amount_yocto)} Ⓝ
                            </span>
                            <span className="text-gray-500">•</span>
                            <span className="text-gray-400">{formatTimestamp(expense.ts_ms)}</span>
                          </div>
                        </div>
                        <p className="text-lg text-gray-400 mt-1 break-all">Paid by <span className="text-gray-300">{expense.payer}</span></p>
                        <div className="mt-2 flex flex-wrap gap-2">
                          {expense.participants.map((participant) => (
                            <div key={participant.account_id} className="flex items-center gap-2 rounded-lg bg-gray-900/60 px-3 py-1.5 text-lg border border-gray-800">
                              <span className="text-gray-300 break-all">{participant.account_id}</span>
                              <span className="text-gray-500">·</span>
                              <span className={`${currentTheme.text} font-medium whitespace-nowrap`}>{(participant.weight_bps / 100).toFixed(1)}%</span>
                            </div>
                          ))}
                        </div>
                      </article>
                    ))
                  ) : (
                    <div className="py-4 text-center">
                      <EmptyState type="expenses" />
                    </div>
                  )}
                </div>
              </section>
              )}
            </div>
          ) : (
            <div className="rounded-xl border border-gray-800 bg-gradient-to-br from-gray-900 to-gray-950 p-6 sm:p-8 text-center shadow-lg">
              <div className="mx-auto max-w-md space-y-2">
                <div className={`mx-auto w-16 h-16 rounded-full ${currentTheme.bgSoft} flex items-center justify-center`}>
                  <svg className={`h-8 w-8 ${currentTheme.text}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                  </svg>
                </div>
                <h3 className="text-lg font-semibold text-white">No Circle Selected</h3>
                <p className="text-lg text-gray-400">
                  Select a circle from the sidebar or create a new one to start tracking expenses.
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
