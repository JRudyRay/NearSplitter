import React from 'react';
import { CircleDot, Users, Receipt, Zap } from 'lucide-react';

interface EmptyStateProps {
  type: 'circles' | 'expenses' | 'settlements';
  actionButton?: React.ReactNode;
}

export function EmptyState({ type, actionButton }: EmptyStateProps) {
  const config = {
    circles: {
      icon: CircleDot,
      title: 'No circles yet',
      description: 'Create your first circle to start tracking shared expenses with friends, roommates, or travel buddies.',
      tips: [
        'Perfect for roommates sharing rent and utilities',
        'Track expenses during group trips',
        'Split costs for events and parties',
      ],
    },
    expenses: {
      icon: Receipt,
      title: 'No expenses yet',
      description: 'Add your first expense to start tracking who paid for what. You can split costs evenly or with custom percentages.',
      tips: [
        'Add expenses as they happen to stay organized',
        'Use clear descriptions to remember what they were for',
        'Split evenly or customize shares per person',
      ],
    },
    settlements: {
      icon: Zap,
      title: 'Ready to settle',
      description: 'When everyone confirms, settlements will execute automatically from escrow. No manual transfers needed!',
      tips: [
        'All members must confirm for auto-settlement',
        'Deposits are held securely in escrow',
        'Optimal settlements are calculated automatically',
      ],
    },
  };

  const { icon: Icon, title, description, tips } = config[type];

  return (
    <div className="flex flex-col items-center justify-center py-12 px-4">
      <div className="max-w-md w-full space-y-6">
        {/* Icon */}
        <div className="flex justify-center">
          <div className="rounded-full bg-gradient-to-br from-blue-500/20 to-purple-500/20 p-6 ring-1 ring-white/10">
            <Icon className="w-12 h-12 text-blue-400" />
          </div>
        </div>

        {/* Content */}
        <div className="text-center space-y-3">
          <h3 className="text-xl font-bold text-white">{title}</h3>
          <p className="text-gray-400 text-sm">{description}</p>
        </div>

        {/* Tips */}
        <div className="bg-white/5 rounded-xl p-4 space-y-2">
          {tips.map((tip, index) => (
            <div key={index} className="flex items-start gap-3 text-sm">
              <div className="mt-1 w-1.5 h-1.5 rounded-full bg-blue-400 flex-shrink-0" />
              <span className="text-gray-300">{tip}</span>
            </div>
          ))}
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
