'use client';

import { useCallback, useEffect, useMemo, useState, type ChangeEvent, type FormEvent, type MouseEvent } from 'react';
import { Loader2, PlusCircle, Wallet } from 'lucide-react';
import { useSimpleNear } from '@/components/providers/simple-near-provider';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useContractView } from '@/lib/hooks/use-contract-view';
import { useContractCall } from '@/lib/hooks/use-contract-call';
import { useLocalStorage } from '@/lib/hooks/use-local-storage';
import { formatNearAmount, formatTimestamp, parseNearAmount, shortenAccountId } from '@/lib/utils/format';
import { buildEqualShares, uniq } from '@/lib/utils/shares';
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
  const near = useSimpleNear();
  const [notification, setNotification] = useState<MessageState | null>(null);
  const [trackedKey, setTrackedKey] = useState<string>('nearsplitter:guest:circles');
  const [trackedCircleIds, setTrackedCircleIds] = useLocalStorage<string[]>(trackedKey, []);
  const [circleMap, setCircleMap] = useState<Record<string, Circle>>({});
  const [selectedCircleId, setSelectedCircleId] = useState<string | null>(null);

  const [createCircleName, setCreateCircleName] = useState('');
  const [joinCircleId, setJoinCircleId] = useState('');
  const [trackCircleId, setTrackCircleId] = useState('');
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

  const storageBounds = useContractView<StorageBalanceBounds>('storage_balance_bounds', {});
  const storageBalance = useContractView<StorageBalance | null>(
    near.accountId ? 'storage_balance_of' : null,
    near.accountId ? { account_id: near.accountId } : null,
    { refreshInterval: 15_000 }
  );

  const ownerCircles = useContractView<Circle[]>(
    near.accountId ? 'list_circles_by_owner' : null,
    near.accountId ? { owner: near.accountId, from: 0, limit: 100 } : null,
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

  const selectedCircle = selectedCircleId ? circleMap[selectedCircleId] : null;
  const membersSignature = useMemo(
    () => (selectedCircle ? selectedCircle.members.join('|') : ''),
    [selectedCircle]
  );
  const isRegistered = Boolean(storageBalance.data?.total);

  useEffect(() => {
    if (!near.accountId) {
      setTrackedKey('nearsplitter:guest:circles');
      return;
    }
    setTrackedKey(`nearsplitter:${near.accountId}:circles`);
  }, [near.accountId]);

  useEffect(() => {
    if (!ownerCircles.data) {
      return;
    }
    const owned = ownerCircles.data as Circle[];
    setCircleMap((prev: Record<string, Circle>) => {
      const next = { ...prev };
      for (const circle of owned) {
        next[circle.id] = circle;
      }
      return next;
    });
    setTrackedCircleIds((prev: string[]) => uniq([...prev, ...owned.map((c: Circle) => c.id)]));
  }, [ownerCircles.data, setTrackedCircleIds]);

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

  useEffect(() => {
    const missing = trackedCircleIds.filter((id: string) => !circleMap[id]);
    if (missing.length === 0) {
      return;
    }
    (async () => {
      const resolved = await Promise.allSettled(missing.map((id: string) => getCircle(id)));
      const next: Record<string, Circle> = {};
      resolved.forEach((result: PromiseSettledResult<Circle>, idx: number) => {
        if (result.near.status === 'fulfilled') {
          next[result.value.id] = result.value;
        } else {
          console.warn('Failed to resolve circle', missing[idx], result.reason);
          setNotification({ type: 'error', text: `Unable to load circle ${missing[idx]}` });
        }
      });
      if (Object.keys(next).length > 0) {
        setCircleMap((prev: Record<string, Circle>) => ({ ...prev, ...next }));
      }
    })().catch((error) => console.error('Failed to hydrate circles', error));
  }, [trackedCircleIds, circleMap]);

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
  }, [near]);

  const handleRegister = useCallback(async () => {
    if (!storageBounds.data) {
      return;
    }
    try {
      await registerMutation.execute('storage_deposit', { account_id: near.accountId }, {
        deposit: storageBounds.data.min,
        gas: GAS_150_TGAS
      });
      await storageBalance.mutate();
      setNotification({ type: 'success', text: 'Storage deposit registered successfully.' });
    } catch (error) {
      setNotification({ type: 'error', text: (error as Error).message });
    }
  }, [near.accountId, registerMutation, storageBounds.data, storageBalance]);

  const handleCreateCircle = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!createCircleName.trim()) {
        setNotification({ type: 'error', text: 'Circle name cannot be empty.' });
        return;
      }
      try {
        await createCircleMutation.execute('create_circle', { name: createCircleName.trim() });
        setCreateCircleName('');
        await ownerCircles.mutate();
        setNotification({ type: 'success', text: 'Circle created!' });
      } catch (error) {
        setNotification({ type: 'error', text: (error as Error).message });
      }
    },
    [createCircleName, createCircleMutation, ownerCircles]
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
        await joinCircleMutation.execute('join_circle', { circle_id: trimmed });
        setJoinCircleId('');
  setTrackedCircleIds((prev: string[]) => uniq([...prev, trimmed]));
        await ownerCircles.mutate();
        setNotification({ type: 'success', text: 'Joined circle successfully.' });
      } catch (error) {
        setNotification({ type: 'error', text: (error as Error).message });
      }
    },
    [joinCircleId, joinCircleMutation, ownerCircles, setTrackedCircleIds]
  );

  const handleTrackCircle = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      const trimmed = trackCircleId.trim();
      if (!trimmed) {
        setNotification({ type: 'error', text: 'Provide a circle ID to track.' });
        return;
      }
      try {
        const circle = await getCircle(trimmed);
  setCircleMap((prev: Record<string, Circle>) => ({ ...prev, [circle.id]: circle }));
  setTrackedCircleIds((prev: string[]) => uniq([...prev, circle.id]));
        setTrackCircleId('');
        setNotification({ type: 'success', text: `Tracking circle ${circle.name}.` });
      } catch (error) {
        setNotification({ type: 'error', text: `Circle not found: ${(error as Error).message}` });
      }
    },
    [trackCircleId, setTrackedCircleIds]
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

  const removeTrackedCircle = useCallback(
    (circleId: string) => {
  setTrackedCircleIds((prev: string[]) => prev.filter((id: string) => id !== circleId));
      if (selectedCircleId === circleId) {
        setSelectedCircleId(null);
      }
    },
    [selectedCircleId, setTrackedCircleIds]
  );

  return (
    <main className="mx-auto flex max-w-6xl flex-col gap-12 px-6 py-12">
      <header className="flex flex-col gap-6 rounded-2xl border border-slate-800 bg-slate-900/60 p-6 shadow-xl shadow-black/30 sm:flex-row sm:items-center sm:justify-between">
        <div className="space-y-2">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">NearSplitter</h1>
          <p className="max-w-2xl text-sm text-slate-300">
            Manage shared expenses on NEAR testnet. Create circles, record expenses, and settle balances with
            streamlined payouts.
          </p>
        </div>
        <div className="flex h-full items-center gap-3">
          {near.status === 'loading' && <Loader2 className="h-5 w-5 animate-spin text-slate-300" />}
          {near.accountId ? (
            <div className="flex items-center gap-3">
              <span className="rounded-full bg-slate-800 px-3 py-1 text-sm text-slate-200">
                {shortenAccountId(near.accountId)}
              </span>
              <Button variant="secondary" onClick={handleSignOut}>
                Sign out
              </Button>
            </div>
          ) : (
            <Button onClick={handleSignIn} className="gap-2">
              <Wallet className="h-4 w-4" /> Connect wallet
            </Button>
          )}
        </div>
      </header>

      {notification && (
        <div
          className={`rounded-lg border px-4 py-3 text-sm ${
            notification.type === 'success'
              ? 'border-emerald-800 bg-emerald-900/50 text-emerald-200'
              : 'border-rose-800 bg-rose-900/50 text-rose-200'
          }`}
        >
          {notification.text}
        </div>
      )}

      <section className="grid gap-6 md:grid-cols-2">
        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-6">
          <h2 className="text-xl font-semibold">Storage registration</h2>
          <p className="mt-2 text-sm text-slate-300">
            Register once per account to cover storage costs required by the NearSplitter contract.
          </p>
          <div className="mt-4 space-y-2 text-sm text-slate-200">
            <p>
              near.status:{' '}
              <span className={isRegistered ? 'text-emerald-400' : 'text-rose-400'}>
                {isRegistered ? 'Registered' : 'Not registered'}
              </span>
            </p>
            {storageBounds.data && (
              <p>Required deposit: {formatNearAmount(storageBounds.data.min)} Ⓝ</p>
            )}
            {storageBalance.data && (
              <p>
                Available refund: {formatNearAmount(storageBalance.data.available)} Ⓝ / Total:{' '}
                {formatNearAmount(storageBalance.data.total)} Ⓝ
              </p>
            )}
          </div>
          <Button
            className="mt-4"
            variant="primary"
            onClick={handleRegister}
            loading={registerMutation.loading}
            disabled={!near.accountId || isRegistered || !storageBounds.data}
          >
            Register storage
          </Button>
        </div>

        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-6">
          <h2 className="text-xl font-semibold">Manage circles</h2>
          <form className="mt-4 space-y-3" onSubmit={handleCreateCircle}>
            <label className="block text-sm font-medium text-slate-200">Create a new circle</label>
            <div className="flex gap-2">
              <Input
                value={createCircleName}
                onChange={(event: ChangeEvent<HTMLInputElement>) => setCreateCircleName(event.target.value)}
                placeholder="Trip to Lisbon"
                required
              />
              <Button type="submit" loading={createCircleMutation.loading} disabled={!near.accountId || !isRegistered}>
                <PlusCircle className="h-4 w-4" />
              </Button>
            </div>
          </form>

          <form className="mt-4 space-y-3" onSubmit={handleJoinCircle}>
            <label className="block text-sm font-medium text-slate-200">Join an existing circle</label>
            <div className="flex gap-2">
              <Input
                value={joinCircleId}
                onChange={(event: ChangeEvent<HTMLInputElement>) => setJoinCircleId(event.target.value)}
                placeholder="circle-0"
              />
              <Button type="submit" loading={joinCircleMutation.loading} disabled={!near.accountId || !isRegistered}>
                Join
              </Button>
            </div>
          </form>

          <form className="mt-4 space-y-3" onSubmit={handleTrackCircle}>
            <label className="block text-sm font-medium text-slate-200">Track a circle by ID</label>
            <div className="flex gap-2">
              <Input
                value={trackCircleId}
                onChange={(event: ChangeEvent<HTMLInputElement>) => setTrackCircleId(event.target.value)}
                placeholder="circle-123"
              />
              <Button type="submit" disabled={!trackCircleId.trim()}>Track</Button>
            </div>
          </form>
        </div>
      </section>

      <section className="grid gap-6 lg:grid-cols-[320px_1fr]">
        <aside className="space-y-4">
          <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
            <h3 className="text-lg font-semibold">Tracked circles</h3>
            <p className="mt-1 text-sm text-slate-400">
              Click a circle to load its expenses, balances, and settlement insights.
            </p>
            <ul className="mt-4 space-y-2">
              {trackedCircleIds.length === 0 && <li className="text-sm text-slate-500">No circles yet.</li>}
              {trackedCircleIds.map((circleId: string) => {
                const circle = circleMap[circleId];
                return (
                  <li key={circleId}>
                    <button
                      type="button"
                      onClick={() => setSelectedCircleId(circleId)}
                      className={`w-full rounded-lg border px-3 py-2 text-left text-sm transition ${
                        selectedCircleId === circleId
                          ? 'border-brand-500 bg-brand-500/10 text-brand-100'
                          : 'border-slate-800 bg-slate-900/60 text-slate-200 hover:border-slate-700'
                      }`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="font-medium">{circle ? circle.name : circleId}</span>
                        <button
                          type="button"
                          onClick={(event: MouseEvent<HTMLButtonElement>) => {
                            event.stopPropagation();
                            removeTrackedCircle(circleId);
                          }}
                          className="text-xs text-slate-400 hover:text-rose-400"
                        >
                          Remove
                        </button>
                      </div>
                      {circle && (
                        <p className="mt-1 text-xs text-slate-400">
                          {circle.members.length} members • Created {formatTimestamp(circle.created_ms)}
                        </p>
                      )}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        </aside>

        <div className="space-y-6">
          {selectedCircle ? (
            <div className="space-y-6">
              <section className="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div>
                    <h2 className="text-2xl font-semibold">{selectedCircle.name}</h2>
                    <p className="text-sm text-slate-400">
                      Owner: {shortenAccountId(selectedCircle.owner)} • Members: {selectedCircle.members.length}
                    </p>
                  </div>
                </div>

                <div className="mt-6 grid gap-4 md:grid-cols-2">
                  <form onSubmit={handleAddExpense} className="space-y-4 rounded-xl border border-slate-800 bg-slate-900/40 p-4">
                    <h3 className="text-lg font-semibold">Add expense</h3>
                    <div className="space-y-1">
                      <label className="text-xs font-medium text-slate-300">Amount (NEAR)</label>
                      <Input
                        value={expenseAmount}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setExpenseAmount(event.target.value)}
                        placeholder="5.0"
                        type="number"
                        min="0"
                        step="0.01"
                        required
                      />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-medium text-slate-300">Memo</label>
                      <Input
                        value={expenseMemo}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setExpenseMemo(event.target.value)}
                        placeholder="Dinner"
                      />
                    </div>
                    <div className="space-y-2">
                      <p className="text-xs font-medium text-slate-300">Participants</p>
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
                            className={`rounded-full px-3 py-1 text-xs transition ${
                              selectedParticipants[member]
                                ? 'bg-brand-500 text-white'
                                : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
                            }`}
                          >
                            {shortenAccountId(member)}
                          </button>
                        ))}
                      </div>
                    </div>
                    <Button
                      type="submit"
                      loading={addExpenseMutation.loading}
                      disabled={participantIds.length === 0 || !expenseAmount}
                    >
                      Record expense
                    </Button>
                  </form>

                  <form onSubmit={handlePayNative} className="space-y-4 rounded-xl border border-slate-800 bg-slate-900/40 p-4">
                    <h3 className="text-lg font-semibold">Settle with NEAR</h3>
                    <div className="space-y-1">
                      <label className="text-xs font-medium text-slate-300">Recipient</label>
                      <select
                        className="w-full rounded-md border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-slate-100"
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
                      <label className="text-xs font-medium text-slate-300">Amount (NEAR)</label>
                      <Input
                        value={settlementAmount}
                        onChange={(event: ChangeEvent<HTMLInputElement>) => setSettlementAmount(event.target.value)}
                        placeholder="1.5"
                        type="number"
                        min="0"
                        step="0.01"
                      />
                    </div>
                    <Button
                      type="submit"
                      loading={payNativeMutation.loading}
                      disabled={!settlementRecipient || !settlementAmount}
                    >
                      Send payment
                    </Button>
                  </form>
                </div>
              </section>

              <section className="grid gap-4 md:grid-cols-2">
                <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-6">
                  <h3 className="text-lg font-semibold">Balances</h3>
                  <p className="text-xs text-slate-400">Positive values mean others owe this member.</p>
                  <ul className="mt-4 space-y-2 text-sm">
                    {circleBalances.data?.map((balance: BalanceView) => (
                      <li
                        key={balance.account_id}
                        className="flex items-center justify-between rounded-lg bg-slate-900/60 px-3 py-2"
                      >
                        <span>{shortenAccountId(balance.account_id)}</span>
                        <span
                          className={
                            BigInt(balance.net) >= 0n ? 'text-emerald-400 font-medium' : 'text-rose-400 font-medium'
                          }
                        >
                          {formatNearAmount(BigInt(balance.net).toString())} Ⓝ
                        </span>
                      </li>
                    )) || <p className="text-xs text-slate-500">No balances yet.</p>}
                  </ul>
                </div>

                <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-6">
                  <h3 className="text-lg font-semibold">Settlement suggestions</h3>
                  <p className="text-xs text-slate-400">
                    Auto-generated minimal transfers. Click to prefill the settlement form.
                  </p>
                  <ul className="mt-4 space-y-3 text-sm">
                    {circleSuggestions.data && circleSuggestions.data.length > 0 ? (
                      circleSuggestions.data.map((suggestion: SettlementSuggestion, idx: number) => (
                        <li
                          key={`${suggestion.from}-${suggestion.to}-${idx}`}
                          className="flex items-center justify-between rounded-lg bg-slate-900/60 px-3 py-2"
                        >
                          <div>
                            <p className="font-medium text-slate-100">
                              {shortenAccountId(suggestion.from)} ➝ {shortenAccountId(suggestion.to)}
                            </p>
                            <p className="text-xs text-slate-400">Token: {suggestion.token ?? 'NEAR'}</p>
                          </div>
                          <button
                            type="button"
                            onClick={() => {
                              setSettlementRecipient(suggestion.to);
                              setSettlementAmount(formatNearAmount(suggestion.amount));
                            }}
                            className="text-xs text-brand-300 hover:text-brand-200"
                          >
                            Prefill {formatNearAmount(suggestion.amount)}
                          </button>
                        </li>
                      ))
                    ) : (
                      <p className="text-xs text-slate-500">No suggestions yet—add expenses to generate them.</p>
                    )}
                  </ul>
                </div>
              </section>

              <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-6">
                <h3 className="text-lg font-semibold">Recent expenses</h3>
                <p className="text-xs text-slate-400">Latest activity within this circle.</p>
                <div className="mt-4 space-y-3 text-sm">
                  {circleExpenses.data && circleExpenses.data.length > 0 ? (
                    circleExpenses.data.map((expense: Expense) => (
                      <article key={expense.id} className="rounded-xl border border-slate-800 bg-slate-900/50 p-4">
                        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                          <h4 className="font-semibold text-slate-100">{expense.memo || 'Untitled expense'}</h4>
                          <span className="text-sm text-slate-300">
                            {formatNearAmount(expense.amount_yocto)} Ⓝ • {formatTimestamp(expense.ts_ms)}
                          </span>
                        </div>
                        <p className="text-xs text-slate-400">Paid by {shortenAccountId(expense.payer)}</p>
                        <div className="mt-2 grid gap-2 md:grid-cols-2">
                          {expense.participants.map((participant) => (
                            <div key={participant.account_id} className="flex justify-between rounded-lg bg-slate-900/60 px-3 py-1">
                              <span>{shortenAccountId(participant.account_id)}</span>
                              <span>{(participant.weight_bps / 100).toFixed(2)}%</span>
                            </div>
                          ))}
                        </div>
                      </article>
                    ))
                  ) : (
                    <p className="text-xs text-slate-500">No expenses recorded yet.</p>
                  )}
                </div>
              </section>
            </div>
          ) : (
            <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-12 text-center text-sm text-slate-400">
              Select or track a circle to see its activity.
            </div>
          )}
        </div>
      </section>
    </main>
  );
}
