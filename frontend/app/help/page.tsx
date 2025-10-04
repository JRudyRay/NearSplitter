'use client';

import React from 'react';
import { CircleDot, Wallet, Receipt, CheckCircle2, Coins, Zap, Shield } from 'lucide-react';
import { Logo } from '@/components/ui/logo';

export default function HelpPage() {
  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-blue-900 to-purple-900">
      <div className="max-w-4xl mx-auto px-4 py-8">
        {/* Header */}
        <div className="mb-8 flex items-center justify-between">
          <Logo size="md" />
          <a 
            href="/"
            className="px-4 py-2 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300 rounded-lg transition-colors"
          >
            ← Back to App
          </a>
        </div>

        {/* Hero Section */}
        <div className="text-center mb-12">
          <h1 className="text-4xl font-bold text-white mb-4">
            How to Use NearSplitter
          </h1>
          <p className="text-gray-300 text-lg">
            Split expenses fairly and settle automatically on the NEAR blockchain
          </p>
        </div>

        {/* Quick Start */}
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-2xl p-6 mb-8">
          <h2 className="text-2xl font-bold text-white mb-4 flex items-center gap-2">
            <Zap className="w-6 h-6 text-yellow-400" />
            Quick Start Guide
          </h2>
          <div className="space-y-4">
            {[
              { step: 1, title: 'Connect Wallet', desc: 'Click "Connect Wallet" and sign in with your NEAR wallet', icon: Wallet },
              { step: 2, title: 'Register', desc: 'Pay a small one-time storage fee (~0.025 Ⓝ) to register', icon: CheckCircle2 },
              { step: 3, title: 'Create Circle', desc: 'Create a new circle and invite friends with a code', icon: CircleDot },
              { step: 4, title: 'Add Expenses', desc: 'Add who paid and how to split the cost', icon: Receipt },
              { step: 5, title: 'Settle Up', desc: 'Confirm ledger when ready - settlements happen automatically!', icon: Coins },
            ].map((item) => (
              <div key={item.step} className="flex gap-4 items-start">
                <div className="w-8 h-8 rounded-full bg-gradient-to-br from-blue-500 to-purple-500 flex items-center justify-center flex-shrink-0 text-white font-bold">
                  {item.step}
                </div>
                <div className="flex-1">
                  <h3 className="text-white font-semibold flex items-center gap-2">
                    <item.icon className="w-4 h-4" />
                    {item.title}
                  </h3>
                  <p className="text-gray-400 text-sm">{item.desc}</p>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Features */}
        <div className="grid md:grid-cols-2 gap-6 mb-8">
          <FeatureCard
            icon={<CircleDot className="w-8 h-8 text-blue-400" />}
            title="Circles"
            description="Create private groups with invite codes for roommates, trips, or events. Each circle tracks its own expenses separately."
          />
          <FeatureCard
            icon={<Receipt className="w-8 h-8 text-green-400" />}
            title="Flexible Splitting"
            description="Split expenses evenly or with custom percentages. Perfect for when someone pays more or less of a shared cost."
          />
          <FeatureCard
            icon={<Zap className="w-8 h-8 text-yellow-400" />}
            title="Auto Settlement"
            description="When everyone confirms, the app automatically calculates and executes optimal settlements from escrow. No manual transfers needed!"
          />
          <FeatureCard
            icon={<Shield className="w-8 h-8 text-purple-400" />}
            title="Blockchain Security"
            description="All transactions are secured on NEAR Protocol. Your money is held in escrow until settlement, ensuring fairness and transparency."
          />
        </div>

        {/* FAQ */}
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-2xl p-6 mb-8">
          <h2 className="text-2xl font-bold text-white mb-6">Frequently Asked Questions</h2>
          <div className="space-y-6">
            <FAQItem
              question="What is the storage fee?"
              answer="NEAR requires ~0.025 Ⓝ to store your account data on the blockchain. This is a one-time fee and can be withdrawn when you unregister."
            />
            <FAQItem
              question="How does auto-settlement work?"
              answer="When all members confirm the ledger, they automatically enable autopay. If you owe money, you deposit it in escrow. The contract then automatically calculates the optimal settlements and transfers funds directly from escrow."
            />
            <FAQItem
              question="Can I settle without everyone confirming?"
              answer="Currently, automatic settlement requires all members to confirm. This ensures everyone agrees on the final amounts before money moves."
            />
            <FAQItem
              question="What if I overpay into escrow?"
              answer="Any excess funds in escrow are automatically refunded to you after settlements are executed."
            />
            <FAQItem
              question="Is my data private?"
              answer="Circles can be private with invite codes. However, remember that all blockchain transactions are publicly viewable on NEAR."
            />
            <FAQItem
              question="What happens if someone doesn't pay?"
              answer="The autopay settlement only executes when ALL members confirm and deposit their owed amounts. This protects creditors from non-payment."
            />
          </div>
        </div>

        {/* Tips */}
        <div className="bg-gradient-to-r from-blue-500/10 to-purple-500/10 border border-blue-500/20 rounded-2xl p-6">
          <h2 className="text-2xl font-bold text-white mb-4">💡 Pro Tips</h2>
          <ul className="space-y-3 text-gray-300">
            <li className="flex gap-2">
              <span className="text-blue-400">•</span>
              <span>Add expenses regularly to avoid forgetting who paid for what</span>
            </li>
            <li className="flex gap-2">
              <span className="text-blue-400">•</span>
              <span>Use descriptive names for expenses to remember them later</span>
            </li>
            <li className="flex gap-2">
              <span className="text-blue-400">•</span>
              <span>Share the invite code through a secure channel (Signal, WhatsApp)</span>
            </li>
            <li className="flex gap-2">
              <span className="text-blue-400">•</span>
              <span>Check your balance before confirming to know how much you&apos;ll need to deposit</span>
            </li>
            <li className="flex gap-2">
              <span className="text-blue-400">•</span>
              <span>The contract is on testnet - funds are not real NEAR tokens</span>
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}

function FeatureCard({ icon, title, description }: { icon: React.ReactNode; title: string; description: string }) {
  return (
    <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-6 hover:bg-white/10 transition-colors">
      <div className="mb-3">{icon}</div>
      <h3 className="text-lg font-semibold text-white mb-2">{title}</h3>
      <p className="text-gray-400 text-sm">{description}</p>
    </div>
  );
}

function FAQItem({ question, answer }: { question: string; answer: string }) {
  return (
    <div>
      <h3 className="text-white font-semibold mb-2">{question}</h3>
      <p className="text-gray-400 text-sm">{answer}</p>
    </div>
  );
}
