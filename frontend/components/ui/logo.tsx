import React from 'react';

interface LogoProps {
  size?: 'sm' | 'md' | 'lg';
  showText?: boolean;
  theme?: 'green' | 'blue' | 'purple' | 'pink';
}

export function Logo({ size = 'md', showText = true, theme = 'green' }: LogoProps) {
  const sizeClasses = {
    sm: 'w-6 h-6',
    md: 'w-8 h-8',
    lg: 'w-12 h-12',
  };

  const textSizeClasses = {
    sm: 'text-lg',
    md: 'text-xl',
    lg: 'text-3xl',
  };

  // Theme-specific colors for the logo
  const themeColors = {
    green: {
      color1: '#10b981',  // emerald-500
      color2: '#06d6a0',  // green-400
      gradient: 'from-emerald-400 to-green-300',
      glowRgba: '16,185,129'
    },
    blue: {
      color1: '#3b82f6',  // blue-500
      color2: '#06b6d4',  // cyan-500
      gradient: 'from-blue-400 to-cyan-300',
      glowRgba: '59,130,246'
    },
    purple: {
      color1: '#a855f7',  // purple-500
      color2: '#ec4899',  // pink-500
      gradient: 'from-purple-400 to-pink-300',
      glowRgba: '168,85,247'
    },
    pink: {
      color1: '#ec4899',  // pink-500
      color2: '#fb7185',  // rose-400
      gradient: 'from-pink-400 to-rose-300',
      glowRgba: '236,72,153'
    }
  };

  const currentColors = themeColors[theme];

  return (
    <div className="flex items-center gap-3">
      {/* Logo Icon - Simple split circle */}
      <div className={`${sizeClasses[size]} relative flex items-center justify-center`}>
        <svg
          viewBox="0 0 100 100"
          className="w-full h-full"
          xmlns="http://www.w3.org/2000/svg"
        >
          {/* Gradient definition - dynamic theme colors */}
          <defs>
            <linearGradient id={`logo-gradient-${theme}`} x1="0%" y1="0%" x2="100%" y2="100%">
              <stop offset="0%" stopColor={currentColors.color1} />
              <stop offset="100%" stopColor={currentColors.color2} />
            </linearGradient>
            <filter id="glow">
              <feGaussianBlur stdDeviation="3" result="coloredBlur"/>
              <feMerge>
                <feMergeNode in="coloredBlur"/>
                <feMergeNode in="SourceGraphic"/>
              </feMerge>
            </filter>
          </defs>

          {/* Simple split circle - two halves */}
          <circle cx="50" cy="50" r="40" fill="none" stroke={`url(#logo-gradient-${theme})`} strokeWidth="4" filter="url(#glow)" opacity="0.3" />
          
          {/* Left half */}
          <path
            d="M50 10 A40 40 0 0 1 50 90 L50 50 Z"
            fill={`url(#logo-gradient-${theme})`}
            opacity="0.8"
            filter="url(#glow)"
          />
          
          {/* Right half */}
          <path
            d="M50 10 A40 40 0 0 0 50 90 L50 50 Z"
            fill={`url(#logo-gradient-${theme})`}
            opacity="0.4"
          />
          
          {/* Center line - the "split" */}
          <line x1="50" y1="10" x2="50" y2="90" stroke={currentColors.color1} strokeWidth="3" filter="url(#glow)" />
          
          {/* NEAR symbol */}
          <text
            x="50"
            y="56"
            fontSize="14"
            fontWeight="bold"
            fill={currentColors.color1}
            textAnchor="middle"
            fontFamily="system-ui"
            filter="url(#glow)"
          >
            Ⓝ
          </text>
        </svg>
      </div>

      {/* Logo Text */}
      {showText && (
        <div className="flex flex-col">
          <span className={`${textSizeClasses[size]} font-bold bg-gradient-to-r ${currentColors.gradient} bg-clip-text text-transparent drop-shadow-[0_0_8px_rgba(${currentColors.glowRgba},0.5)]`}>
            NearSplitter
          </span>
          <span className="text-[10px] text-gray-400 -mt-1 tracking-wider uppercase">
            Split expenses on NEAR
          </span>
        </div>
      )}
    </div>
  );
}
