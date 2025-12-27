'use client';

import React from 'react';
import { CircleDot, Wallet, Receipt, CheckCircle2, Coins, Zap, Shield, ArrowLeft } from 'lucide-react';
import { Logo } from '@/components/ui/logo';
import { Button } from '@/components/ui/button';

export default function HelpPage() {
  return (
    <div className="min-h-screen bg-gradient-to-b from-bg via-muted to-bg">
      <div className="max-w-4xl mx-auto px-4 py-8">
        {/* Header */}
        <header className="mb-10 flex items-center justify-between">
          <Logo size="md" />
          <a href="/">
            <Button variant="secondary" leftIcon={<ArrowLeft className="w-4 h-4" />}>
              Back to App
            </Button>
          </a>
        </header>

        {/* Hero Section */}
        <div className="text-center mb-12 animate-fade-in">
          <h1 className="text-4xl md:text-5xl font-bold text-fg mb-4 near-text-glow">
            How to Use NearSplitter
          </h1>
          <p className="text-muted-fg text-lg max-w-2xl mx-auto">
            Split expenses fairly and settle automatically on the NEAR blockchain. 
            No more awkward money conversations!
          </p>
        </div>

        {/* Quick Start - Step by Step */}
        <div className="near-card p-6 mb-8 animate-slide-up">
          <h2 className="text-2xl font-bold text-fg mb-6 flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-brand-500/20 flex items-center justify-center shadow-near-glow-sm">
              <Zap className="w-5 h-5 text-brand-500" />
            </div>
            Quick Start Guide
          </h2>
          <div className="space-y-6">
            {[
              { step: 1, title: 'Connect Your Wallet', desc: 'Click "Connect Wallet" and sign in with your NEAR wallet (testnet)', icon: Wallet },
              { step: 2, title: 'One-Time Registration', desc: 'Pay a small storage fee (~0.03 Ⓝ) to register your account', icon: CheckCircle2 },
              { step: 3, title: 'Create or Join a Circle', desc: 'Create a new circle with a password, or join an existing one', icon: CircleDot },
              { step: 4, title: 'Add Expenses', desc: 'Record an expense and select who to split the cost with', icon: Receipt },
              { step: 5, title: 'Settle Up', desc: 'Confirm when ready - settlements happen automatically!', icon: Coins },
            ].map((item, index) => (
              <div key={item.step} className="flex gap-4 items-start group">
                <div className="w-10 h-10 rounded-full bg-brand-500 flex items-center justify-center flex-shrink-0 text-black font-bold shadow-near-glow group-hover:shadow-near-glow-lg transition-all duration-300">
                  {item.step}
                </div>
                <div className="flex-1 pt-1">
                  <h3 className="text-fg font-semibold text-lg flex items-center gap-2 mb-1">
                    <item.icon className="w-4 h-4 text-brand-500" />
                    {item.title}
                  </h3>
                  <p className="text-muted-fg">{item.desc}</p>
                </div>
                {index < 4 && (
                  <div className="absolute left-[19px] mt-10 w-0.5 h-6 bg-brand-500/30 hidden md:block" />
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Features Grid */}
        <div className="grid md:grid-cols-2 gap-4 mb-8">
          <FeatureCard
            icon={<CircleDot className="w-7 h-7 text-brand-500" />}
            title="Circles"
            description="Create private groups with passwords for roommates, trips, or events. Each circle tracks its own expenses separately."
          />
          <FeatureCard
            icon={<Receipt className="w-7 h-7 text-brand-500" />}
            title="Flexible Splitting"
            description="Split expenses evenly among selected members. Perfect for shared dinners, utilities, or group activities."
          />
          <FeatureCard
            icon={<Zap className="w-7 h-7 text-brand-500" />}
            title="Auto Settlement"
            description="When everyone confirms, the smart contract calculates optimal settlements and executes them automatically."
          />
          <FeatureCard
            icon={<Shield className="w-7 h-7 text-brand-500" />}
            title="Blockchain Security"
            description="All transactions are secured on NEAR Protocol. Your money is held in escrow until settlement, ensuring fairness."
          />
        </div>

        {/* FAQ Section */}
        <div className="near-card p-6 mb-8">
          <h2 className="text-2xl font-bold text-fg mb-6 flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-brand-500/20 flex items-center justify-center">
              <span className="text-xl">❓</span>
            </div>
            Frequently Asked Questions
          </h2>
          <div className="space-y-6">
            <FAQItem
              question="What is the storage fee?"
              answer="NEAR requires ~0.03 Ⓝ to store your account data on the blockchain. This is a one-time fee and can be withdrawn when you unregister."
            />
            <FAQItem
              question="How does auto-settlement work?"
              answer="When all members confirm the ledger, they automatically enable autopay. If you owe money, you deposit it in escrow. The contract then calculates the optimal settlements and transfers funds automatically."
            />
            <FAQItem
              question="Can I settle without everyone confirming?"
              answer="No, automatic settlement requires all members to confirm. This ensures everyone agrees on the final amounts before money moves."
            />
            <FAQItem
              question="What if I overpay into escrow?"
              answer="Any excess funds in escrow are automatically refunded to you after settlements are executed."
            />
            <FAQItem
              question="Is my data private?"
              answer="Circles require passwords to join. However, remember that all blockchain transactions are publicly viewable on NEAR."
            />
            <FAQItem
              question="What happens if someone doesn&apos;t pay?"
              answer="Settlement only executes when ALL members confirm and deposit their owed amounts. This protects everyone from non-payment."
            />
          </div>
        </div>

        {/* Pro Tips */}
        <div className="near-card-highlight p-6">
          <h2 className="text-2xl font-bold text-fg mb-4 flex items-center gap-2">
            <span className="text-2xl">💡</span>
            Pro Tips
          </h2>
          <ul className="space-y-3 text-fg">
            {[
              'Add expenses as they happen to avoid forgetting',
              'Use clear descriptions to remember what each expense was for',
              'Share the circle password through a secure channel',
              'Check your balance before confirming to know how much you need',
              'This is testnet - funds are not real NEAR tokens',
            ].map((tip, index) => (
              <li key={index} className="flex items-start gap-3">
                <span className="text-brand-500 mt-1">✓</span>
                <span>{tip}</span>
              </li>
            ))}
          </ul>
        </div>

        {/* Footer */}
        <footer className="mt-12 text-center text-muted-fg text-sm">
          <p>Built with ❤️ on NEAR Protocol</p>
        </footer>
      </div>
    </div>
  );
}

function FeatureCard({ icon, title, description }: { icon: React.ReactNode; title: string; description: string }) {
  return (
    <div className="near-card p-5 hover:border-brand-500/50 transition-all duration-300 group">
      <div className="w-12 h-12 rounded-xl bg-brand-500/10 flex items-center justify-center mb-4 group-hover:bg-brand-500/20 group-hover:shadow-near-glow-sm transition-all duration-300">
        {icon}
      </div>
      <h3 className="text-lg font-semibold text-fg mb-2">{title}</h3>
      <p className="text-muted-fg text-sm leading-relaxed">{description}</p>
    </div>
  );
}

function FAQItem({ question, answer }: { question: string; answer: string }) {
  return (
    <div className="border-b border-border pb-4 last:border-0 last:pb-0">
      <h3 className="text-fg font-semibold mb-2 flex items-start gap-2">
        <span className="text-brand-500">Q:</span>
        {question}
      </h3>
      <p className="text-muted-fg text-sm leading-relaxed pl-6">{answer}</p>
    </div>
  );
}
