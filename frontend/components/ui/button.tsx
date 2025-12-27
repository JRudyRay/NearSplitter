"use client";

import { forwardRef } from "react";
import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils/cn";

type ButtonVariant = "primary" | "secondary" | "ghost" | "outline";
type ButtonSize = "sm" | "md" | "lg";

const VARIANT_STYLES: Record<ButtonVariant, string> = {
  primary: "bg-brand-500 hover:bg-brand-600 text-black font-bold shadow-near-glow hover:shadow-near-glow-lg transition-all duration-200 hover:-translate-y-0.5 active:translate-y-0",
  secondary: "bg-muted hover:bg-muted/80 text-fg border border-border hover:border-brand-500/50 transition-all duration-200",
  ghost: "bg-transparent hover:bg-muted/60 text-fg/90 hover:text-fg transition-all duration-200",
  outline: "bg-transparent border-2 border-brand-500 text-brand-500 hover:bg-brand-500/10 font-semibold transition-all duration-200"
};

const SIZE_STYLES: Record<ButtonSize, string> = {
  sm: "px-3 py-1.5 text-sm rounded-lg",
  md: "px-4 py-2.5 text-base rounded-xl",
  lg: "px-6 py-3 text-lg rounded-xl"
};

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (props, ref) => {
    const {
      className,
      type = "button",
      variant = "primary",
      size = "md",
      loading = false,
      disabled,
      children,
      leftIcon,
      rightIcon,
      ...rest
    } = props;
    
    return (
      <button
        ref={ref}
        type={type}
        className={cn(
          "inline-flex items-center justify-center gap-2 font-medium focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-500 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:transform-none",
          VARIANT_STYLES[variant],
          SIZE_STYLES[size],
          className
        )}
        disabled={disabled || loading}
        {...rest}
      >
        {loading ? (
          <>
            <Loader2 className="h-4 w-4 animate-spin" />
            <span>Processing...</span>
          </>
        ) : (
          <>
            {leftIcon && <span className="flex-shrink-0">{leftIcon}</span>}
            {children}
            {rightIcon && <span className="flex-shrink-0">{rightIcon}</span>}
          </>
        )}
      </button>
    );
  }
);

Button.displayName = "Button";
