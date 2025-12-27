import React from 'react';
import { cn } from '@/lib/utils/cn';

interface SkeletonProps {
  className?: string;
  variant?: 'text' | 'circle' | 'rectangle' | 'card';
  width?: string;
  height?: string;
  count?: number;
}

export function Skeleton({ 
  className, 
  variant = 'rectangle',
  width,
  height,
  count = 1 
}: SkeletonProps) {
  const baseClasses = 'animate-pulse bg-gradient-to-r from-muted via-border to-muted bg-[length:200%_100%]';
  
  const variantClasses = {
    text: 'h-4 rounded',
    circle: 'rounded-full',
    rectangle: 'rounded-lg',
    card: 'rounded-xl h-32'
  };

  const items = Array.from({ length: count }, (_, i) => (
    <div
      key={i}
      className={cn(
        baseClasses,
        variantClasses[variant],
        className
      )}
      style={{
        width: width || '100%',
        height: height || (variant === 'text' ? '1rem' : variant === 'circle' ? '3rem' : undefined),
        animation: 'shimmer 2s infinite linear'
      }}
    />
  ));

  return count === 1 ? items[0] : <div className="space-y-3">{items}</div>;
}

// Specialized skeleton components
export function CardSkeleton({ className }: { className?: string }) {
  return (
    <div className={cn("rounded-xl border border-border bg-card/50 p-4 space-y-3", className)}>
      <div className="flex items-center gap-3">
        <Skeleton variant="circle" width="2.5rem" height="2.5rem" />
        <div className="flex-1 space-y-2">
          <Skeleton variant="text" width="60%" />
          <Skeleton variant="text" width="40%" />
        </div>
      </div>
      <Skeleton variant="text" count={2} />
    </div>
  );
}

export function ListSkeleton({ count = 3 }: { count?: number }) {
  return (
    <div className="space-y-2">
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className="rounded-lg border border-border bg-card/40 p-3 space-y-2">
          <div className="flex items-center justify-between">
            <Skeleton variant="text" width="50%" />
            <Skeleton variant="text" width="20%" />
          </div>
          <Skeleton variant="text" width="70%" />
        </div>
      ))}
    </div>
  );
}

export function FormSkeleton() {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Skeleton variant="text" width="30%" height="1.25rem" />
        <Skeleton variant="rectangle" height="3rem" />
      </div>
      <div className="space-y-2">
        <Skeleton variant="text" width="30%" height="1.25rem" />
        <Skeleton variant="rectangle" height="3rem" />
      </div>
      <Skeleton variant="rectangle" height="2.5rem" />
    </div>
  );
}

export function CircleCardSkeleton() {
  return (
    <div className="rounded-xl border border-border bg-card/50 p-4 space-y-3">
      <div className="flex items-start justify-between">
        <div className="flex-1 space-y-2">
          <Skeleton variant="text" width="70%" height="1.5rem" />
          <Skeleton variant="text" width="40%" />
        </div>
        <Skeleton variant="circle" width="2rem" height="2rem" />
      </div>
      <div className="flex gap-2">
        <Skeleton variant="circle" width="1.5rem" height="1.5rem" />
        <Skeleton variant="circle" width="1.5rem" height="1.5rem" />
        <Skeleton variant="circle" width="1.5rem" height="1.5rem" />
      </div>
    </div>
  );
}
