import React from 'react';
import { CircleDot, Receipt, Zap, ArrowRight } from 'lucide-react';

interface EmptyStateProps {
  type: 'circles' | 'expenses' | 'settlements';
  actionButton?: React.ReactNode;
}

export function EmptyState({ type, actionButton }: EmptyStateProps) {
  const config = {
    circles: {
      icon: CircleDot,
      title: 'Create Your First Circle',
      description: 'Circles are groups for splitting expenses. Perfect for roommates, trips, or any shared costs.',
      tips: [
        { emoji: '🏠', text: 'Roommates sharing rent and utilities' },
        { emoji: '✈️', text: 'Group trips and vacations' },
        { emoji: '🎉', text: 'Events, parties, and dinners' },
      ],
      ctaText: 'Get started by creating a circle above',
    },
    expenses: {
      icon: Receipt,
      title: 'No Expenses Yet',
      description: 'Add expenses as they happen. NearSplitter automatically calculates who owes what.',
      tips: [
        { emoji: '💡', text: 'Add expenses right when you pay' },
        { emoji: '📝', text: 'Use clear descriptions for easy tracking' },
        { emoji: '⚖️', text: 'Split evenly or customize shares' },
      ],
      ctaText: 'Add your first expense using the form',
    },
    settlements: {
      icon: Zap,
      title: 'Ready to Settle Up',
      description: 'When all members confirm, settlements execute automatically from escrow. No manual transfers needed!',
      tips: [
        { emoji: '✅', text: 'All members must confirm for auto-settlement' },
        { emoji: '🔒', text: 'Deposits are held securely in escrow' },
        { emoji: '⚡', text: 'Optimal settlements are calculated automatically' },
      ],
      ctaText: 'Confirm your expenses when ready',
    },
  };

  const { icon: Icon, title, description, tips, ctaText } = config[type];

  return (
    <div className="flex flex-col items-center justify-center py-10 px-4 animate-fade-in">
      <div className="max-w-md w-full space-y-6">
        {/* Icon with glow effect */}
        <div className="flex justify-center">
          <div className="rounded-2xl bg-brand-500/10 p-5 ring-1 ring-brand-500/30 shadow-near-glow animate-glow-pulse">
            <Icon className="w-10 h-10 text-brand-500" strokeWidth={1.5} />
          </div>
        </div>

        {/* Content */}
        <div className="text-center space-y-3">
          <h3 className="text-xl font-bold text-fg">{title}</h3>
          <p className="text-muted-fg text-sm leading-relaxed">{description}</p>
        </div>

        {/* Tips with better visual hierarchy */}
        <div className="bg-card/60 rounded-xl p-4 space-y-3 border border-border">
          {tips.map((tip, index) => (
            <div key={index} className="flex items-start gap-3 text-sm">
              <span className="text-lg flex-shrink-0">{tip.emoji}</span>
              <span className="text-fg/90 pt-0.5">{tip.text}</span>
            </div>
          ))}
        </div>

        {/* CTA hint */}
        <div className="flex items-center justify-center gap-2 text-brand-500 text-sm font-medium">
          <ArrowRight className="w-4 h-4 animate-bounce-subtle" />
          <span>{ctaText}</span>
        </div>

        {/* Action Button */}
        {actionButton && (
          <div className="flex justify-center pt-2">
            {actionButton}
          </div>
        )}
      </div>
    </div>
  );
}
