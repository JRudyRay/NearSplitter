"use client";

import { forwardRef } from "react";
import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils/cn";

type ButtonVariant = "primary" | "secondary" | "ghost";

const VARIANT_STYLES: Record<ButtonVariant, string> = {
  primary: "bg-brand-500 hover:bg-brand-600 text-black font-semibold shadow-lg shadow-brand-500/20 hover:shadow-brand-500/30",
  secondary: "bg-gray-800 hover:bg-gray-700 text-white border border-gray-700",
  ghost: "bg-transparent hover:bg-gray-800 text-gray-100"
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
          "inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-all focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-500 disabled:cursor-not-allowed disabled:opacity-50",
          VARIANT_STYLES[resolvedVariant],
          className
        )}
        disabled={disabled || loading}
        {...rest}
      >
        {loading && <Loader2 className="h-4 w-4 animate-spin" />}
        {loading ? "Processing..." : children}
      </button>
    );
  }
);

Button.displayName = "Button";
