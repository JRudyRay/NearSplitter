/*
  NearSplitter Logo Component
  - Sleek geometric "N" with split diagonal
  - Theme-aware: adapts to dark/light mode
*/

'use client';

import React from 'react';

interface LogoProps {
  size?: 'sm' | 'md' | 'lg';
  showText?: boolean;
  variant?: 'full' | 'mark';
}

export function Logo({ size = 'md', showText = true, variant = 'full' }: LogoProps) {
  const sizeClasses = {
    sm: 'w-8 h-8',
    md: 'w-10 h-10',
    lg: 'w-14 h-14',
  };

  const textSizeClasses = {
    sm: 'text-lg',
    md: 'text-xl',
    lg: 'text-2xl',
  };

  return (
    <div className="flex items-center gap-2.5">
      {/* Logo Mark - Geometric N with split concept */}
      <div className={`${sizeClasses[size]} relative flex items-center justify-center flex-shrink-0`}>
        <svg
          viewBox="0 0 40 40"
          className="w-full h-full"
          xmlns="http://www.w3.org/2000/svg"
          aria-hidden="true"
        >
          {/* Rounded square background */}
          <rect x="0" y="0" width="40" height="40" rx="8" className="fill-card" />
          
          {/* Left vertical bar - theme color */}
          <rect x="7" y="8" width="5" height="24" rx="1" className="fill-fg" />
          
          {/* Right vertical bar - brand green */}
          <rect x="28" y="8" width="5" height="24" rx="1" className="fill-brand-500" />
          
          {/* Diagonal connector - gradient effect via two overlapping paths */}
          <path
            d="M12 10 L28 30 L28 26 L12 6 Z"
            className="fill-fg opacity-70"
          />
          <path
            d="M12 14 L28 34 L28 30 L12 10 Z"
            className="fill-brand-500 opacity-80"
          />
        </svg>
      </div>

      {/* Logo Text */}
      {showText && variant === 'full' && (
        <span className={`${textSizeClasses[size]} font-bold tracking-tight`}>
          <span className="text-fg">Near</span>
          <span className="text-brand-500">Splitter</span>
        </span>
      )}
    </div>
  );
}

/**
 * Standalone mark for favicon - static colors for image generation
 */
export function LogoMark({ className = 'w-8 h-8' }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 40 40"
      className={className}
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* Background */}
      <rect x="0" y="0" width="40" height="40" rx="8" fill="#1a1a1a" />
      
      {/* Left bar - light */}
      <rect x="7" y="8" width="5" height="24" rx="1" fill="#e5e7eb" />
      
      {/* Right bar - green */}
      <rect x="28" y="8" width="5" height="24" rx="1" fill="#00EC97" />
      
      {/* Diagonal split */}
      <path d="M12 10 L28 30 L28 26 L12 6 Z" fill="#e5e7eb" opacity="0.7" />
      <path d="M12 14 L28 34 L28 30 L12 10 Z" fill="#00EC97" opacity="0.8" />
    </svg>
  );
}
