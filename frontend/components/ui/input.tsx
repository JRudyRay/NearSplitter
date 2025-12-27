"use client";

import { forwardRef } from "react";
import { cn } from "@/lib/utils/cn";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  error?: boolean;
  helperText?: string;
  label?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>((props, ref) => {
  const { className, type = "text", error, helperText, label, id, ...rest } = props;
  
  return (
    <div className="w-full">
      {label && (
        <label 
          htmlFor={id} 
          className="block text-sm font-semibold text-muted-fg mb-2"
        >
          {label}
        </label>
      )}
      <input
        ref={ref}
        id={id}
        type={type}
        className={cn(
          "flex h-12 w-full rounded-xl border bg-card px-4 py-3 text-base text-fg placeholder:text-muted-fg transition-all duration-200",
          "focus:outline-none focus:ring-2 focus:ring-brand-500/30 focus:border-brand-500",
          "hover:border-border/80",
          "disabled:cursor-not-allowed disabled:opacity-50 disabled:bg-muted",
          error 
            ? "border-danger focus:border-danger focus:ring-danger/30" 
            : "border-border",
          className
        )}
        {...rest}
      />
      {helperText && (
        <p className={cn(
          "mt-1.5 text-sm",
          error ? "text-danger" : "text-muted-fg"
        )}>
          {helperText}
        </p>
      )}
    </div>
  );
});

Input.displayName = "Input";
