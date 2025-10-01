"use client";

import { forwardRef } from "react";
import { cn } from "@/lib/utils/cn";

type ButtonVariant = "primary" | "secondary" | "ghost";

const VARIANT_STYLES: Record<ButtonVariant, string> = {
  primary: "bg-brand-500 hover:bg-brand-400 text-white",
  secondary: "bg-slate-800 hover:bg-slate-700 text-white",
  ghost: "bg-transparent hover:bg-slate-800 text-slate-100"
};

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (props, ref) => {
    const {
      className,
      type = "button",
      variant = "primary",
      loading = false,
      disabled,
      children,
      ...rest
    } = props;
    const resolvedVariant: ButtonVariant = variant;
    return (
      <button
        ref={ref}
        type={type}
        className={cn(
          "inline-flex items-center justify-center rounded-md px-4 py-2 text-sm font-medium transition focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-400 disabled:cursor-not-allowed disabled:opacity-50",
          VARIANT_STYLES[resolvedVariant],
          className
        )}
        disabled={disabled || loading}
        {...rest}
      >
        {loading ? "Please wait…" : children}
      </button>
    );
  }
);

Button.displayName = "Button";
