'use client';

import React, { type ChangeEvent, type FormEvent } from 'react';
import { Eye, EyeOff, Info, PlusCircle, Users } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

export function CirclesTab({
  active,
  canSubmit,
  createCircleName,
  setCreateCircleName,
  createCirclePassword,
  setCreateCirclePassword,
  showCreatePassword,
  setShowCreatePassword,
  joinCircleId,
  setJoinCircleId,
  joinCirclePassword,
  setJoinCirclePassword,
  showJoinPassword,
  setShowJoinPassword,
  validationErrors,
  onCreateCircle,
  onJoinCircle,
  creating,
  joining,
}: {
  active: boolean;
  canSubmit: boolean;
  createCircleName: string;
  setCreateCircleName: (v: string) => void;
  createCirclePassword: string;
  setCreateCirclePassword: (v: string) => void;
  showCreatePassword: boolean;
  setShowCreatePassword: (v: boolean) => void;
  joinCircleId: string;
  setJoinCircleId: (v: string) => void;
  joinCirclePassword: string;
  setJoinCirclePassword: (v: string) => void;
  showJoinPassword: boolean;
  setShowJoinPassword: (v: boolean) => void;
  validationErrors: Record<string, string>;
  onCreateCircle: (e: FormEvent<HTMLFormElement>) => void;
  onJoinCircle: (e: FormEvent<HTMLFormElement>) => void;
  creating: boolean;
  joining: boolean;
}) {
  return (
    <section
      className={`grid gap-3 md:gap-3 md:grid-cols-2 ${!active ? 'hidden' : ''}`}
      id="circles-panel"
      role="tabpanel"
      aria-labelledby="circles-tab"
    >
      <article className="rounded-xl border border-border/60 bg-card/80 p-3 shadow-lg transition-all duration-300 shadow-near-glow-sm backdrop-blur-sm">
        <header className="mb-3">
          <h2 className="text-base sm:text-lg font-bold text-fg flex items-center gap-2">
            <div className="w-8 h-8 rounded-lg bg-brand-500/10 flex items-center justify-center shadow-near-glow">
              <PlusCircle className="w-4 h-4 text-brand-500" aria-hidden="true" />
            </div>
            Create Circle
          </h2>
          <p className="mt-1.5 text-sm text-muted-fg">Start a new expense group</p>
        </header>
        <form className="space-y-2.5" onSubmit={onCreateCircle}>
          <div className="space-y-1.5">
            <label htmlFor="circle-name" className="text-sm font-semibold text-muted-fg block">
              Circle Name
            </label>
            <Input
              id="circle-name"
              value={createCircleName}
              onChange={(event: ChangeEvent<HTMLInputElement>) => setCreateCircleName(event.target.value)}
              placeholder="Trip to Lisbon"
              className="w-full text-sm h-10"
              required
              aria-required="true"
              error={Boolean(validationErrors.circleName)}
              helperText={validationErrors.circleName}
            />
          </div>
          <div className="space-y-1.5">
            <label htmlFor="circle-password" className="text-sm font-semibold text-muted-fg flex items-center gap-2">
              <span>Circle Password</span>
              <button
                type="button"
                className="inline-flex items-center justify-center rounded-md p-1 text-muted-fg hover:text-fg hover:bg-muted/60 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-500"
                aria-label="Password info"
                title="Required. Share this password with circle members so they can join."
              >
                <Info className="h-4 w-4" aria-hidden="true" />
              </button>
            </label>
            <div className="relative">
              <Input
                id="circle-password"
                type={showCreatePassword ? 'text' : 'password'}
                value={createCirclePassword}
                onChange={(event: ChangeEvent<HTMLInputElement>) => setCreateCirclePassword(event.target.value)}
                placeholder="Enter a secure password"
                className="w-full pr-12 text-base h-12"
                required
                aria-required="true"
                error={Boolean(validationErrors.circlePassword)}
                helperText={validationErrors.circlePassword}
              />
              <button
                type="button"
                onClick={() => setShowCreatePassword(!showCreatePassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-fg hover:text-fg transition-all duration-200 p-2 rounded-lg hover:bg-muted/60 min-w-[44px] min-h-[44px] flex items-center justify-center"
                aria-label={showCreatePassword ? 'Hide password' : 'Show password'}
              >
                {showCreatePassword ? <EyeOff className="h-5 w-5" aria-hidden="true" /> : <Eye className="h-5 w-5" aria-hidden="true" />}
              </button>
            </div>
            <p className="text-sm text-muted-fg mt-2 flex items-start gap-2">
              <svg className="w-3.5 h-3.5 mt-0.5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                <path
                  fillRule="evenodd"
                  d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z"
                  clipRule="evenodd"
                />
              </svg>
              <span>Required - Share this password with circle members</span>
            </p>
          </div>
          <Button
            type="submit"
            loading={creating}
            disabled={!canSubmit}
            className="w-full text-sm h-9 shadow-near-glow hover:scale-[1.02] transition-all duration-200 shadow-lg"
            aria-label="Create new circle"
          >
            <PlusCircle className="h-4 w-4 mr-1.5" aria-hidden="true" />
            Create Circle
          </Button>
        </form>
      </article>

      <article className="rounded-xl border border-border/60 bg-card/80 p-3 shadow-lg transition-all duration-300 shadow-near-glow-sm backdrop-blur-sm">
        <header className="mb-3">
          <h2 className="text-base sm:text-lg font-bold text-fg flex items-center gap-2">
            <div className="w-8 h-8 rounded-lg bg-brand-500/10 flex items-center justify-center shadow-near-glow">
              <Users className="w-4 h-4 text-brand-500" aria-hidden="true" />
            </div>
            Join Existing Circle
          </h2>
          <p className="mt-1.5 text-sm text-muted-fg">Enter Circle ID to join</p>
        </header>
        <form className="space-y-2.5" onSubmit={onJoinCircle}>
          <div className="space-y-1.5">
            <label htmlFor="join-circle-id" className="text-sm font-semibold text-muted-fg block">
              Circle ID
            </label>
            <Input
              id="join-circle-id"
              value={joinCircleId}
              onChange={(event: ChangeEvent<HTMLInputElement>) => setJoinCircleId(event.target.value)}
              placeholder="circle-0"
              className="w-full text-sm h-10"
              required
              aria-required="true"
              error={Boolean(validationErrors.circleId)}
              helperText={validationErrors.circleId}
            />
          </div>
          <div className="space-y-1.5">
            <label htmlFor="join-password" className="text-sm font-semibold text-muted-fg flex items-center gap-2">
              <span>Password</span>
              <button
                type="button"
                className="inline-flex items-center justify-center rounded-md p-1 text-muted-fg hover:text-fg hover:bg-muted/60 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-500"
                aria-label="Password info"
                title="Required. Ask the circle owner for the password to join."
              >
                <Info className="h-4 w-4" aria-hidden="true" />
              </button>
            </label>
            <div className="relative">
              <Input
                id="join-password"
                type={showJoinPassword ? 'text' : 'password'}
                value={joinCirclePassword}
                onChange={(event: ChangeEvent<HTMLInputElement>) => setJoinCirclePassword(event.target.value)}
                placeholder="Enter circle password"
                className="w-full pr-10 text-sm h-10"
                required
                aria-required="true"
              />
              <button
                type="button"
                onClick={() => setShowJoinPassword(!showJoinPassword)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-fg hover:text-fg transition-all duration-200 p-1.5 rounded-lg hover:bg-muted/60 min-w-[36px] min-h-[36px] flex items-center justify-center"
                aria-label={showJoinPassword ? 'Hide password' : 'Show password'}
              >
                {showJoinPassword ? <EyeOff className="h-4 w-4" aria-hidden="true" /> : <Eye className="h-4 w-4" aria-hidden="true" />}
              </button>
            </div>
          </div>
          <Button
            type="submit"
            loading={joining}
            disabled={!canSubmit}
            className="w-full text-sm h-9 shadow-near-glow hover:scale-[1.02] transition-all duration-200 shadow-lg"
            aria-label="Join circle"
          >
            Join Circle
          </Button>
          <div className="flex items-start gap-2 p-2.5 rounded-lg bg-muted/40 border border-border/60">
            <svg className="w-4 h-4 text-amber-400 flex-shrink-0 mt-0.5" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
              <path
                fillRule="evenodd"
                d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z"
                clipRule="evenodd"
              />
            </svg>
            <p className="text-sm text-muted-fg">
              <strong className="text-fg">Tip:</strong> Ask the owner for ID and password
            </p>
          </div>
        </form>
      </article>
    </section>
  );
}
