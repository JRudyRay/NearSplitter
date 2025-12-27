import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class"],
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}", "./lib/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Semantic tokens (CSS variables) for dark/light theming
        bg: "rgb(var(--bg) / <alpha-value>)",
        fg: "rgb(var(--fg) / <alpha-value>)",
        muted: "rgb(var(--muted) / <alpha-value>)",
        "muted-fg": "rgb(var(--muted-fg) / <alpha-value>)",
        card: "rgb(var(--card) / <alpha-value>)",
        "card-fg": "rgb(var(--card-fg) / <alpha-value>)",
        border: "rgb(var(--border) / <alpha-value>)",
        ring: "rgb(var(--ring) / <alpha-value>)",
        accent: "rgb(var(--accent) / <alpha-value>)",
        "accent-fg": "rgb(var(--accent-fg) / <alpha-value>)",
        danger: "rgb(var(--danger) / <alpha-value>)",
        "danger-fg": "rgb(var(--danger-fg) / <alpha-value>)",

        // NEAR Protocol Brand Colors
        near: {
          mint: "#00EC97",
          "mint-bright": "#00FFa3",
          "mint-dark": "#00D386",
          "mint-dim": "#00a86b",
          black: "#000000",
          dark: "#0a0a0a",
          surface: "#111111",
          card: "#161616",
          elevated: "#1a1a1a",
        },
        // Primary brand color (NEAR mint green)
        brand: {
          50: "#ecfdf5",
          100: "#d1fae5",
          200: "#a7f3d0",
          300: "#6ee7b7",
          400: "#34d399",
          500: "#00EC97",  // NEAR mint - primary
          600: "#00D386",  // NEAR mint dark
          700: "#00a86b",
          800: "#047857",
          900: "#065f46",
          950: "#022c22",
        },
        // Neutral grays
        gray: {
          50: "#fafafa",
          100: "#f5f5f5",
          200: "#e5e5e5",
          300: "#d4d4d4",
          400: "#a3a3a3",
          500: "#737373",
          600: "#404040",
          700: "#2a2a2a",
          800: "#1a1a1a",
          900: "#111111",
          950: "#0a0a0a",
        },
      },
      boxShadow: {
        "near-glow": "0 0 12px rgba(0, 236, 151, 0.25)",
        "near-glow-sm": "0 0 10px rgba(0, 236, 151, 0.2)",
        "near-glow-lg": "0 0 25px rgba(0, 236, 151, 0.25), 0 0 50px rgba(0, 236, 151, 0.1)",
      },
      animation: {
        "glow-pulse": "glow-pulse 2s ease-in-out infinite",
        "fade-in": "fade-in 0.3s ease-out forwards",
        "slide-up": "slide-up 0.4s ease-out forwards",
        "scale-in": "scale-in 0.2s ease-out forwards",
      },
      keyframes: {
        "glow-pulse": {
          "0%, 100%": { boxShadow: "0 0 12px rgba(0, 236, 151, 0.25)" },
          "50%": { boxShadow: "0 0 18px rgba(0, 236, 151, 0.25), 0 0 36px rgba(0, 236, 151, 0.1)" },
        },
        "fade-in": {
          from: { opacity: "0", transform: "translateY(10px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
        "slide-up": {
          from: { opacity: "0", transform: "translateY(20px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
        "scale-in": {
          from: { opacity: "0", transform: "scale(0.95)" },
          to: { opacity: "1", transform: "scale(1)" },
        },
      },
      borderRadius: {
        "xl": "12px",
        "2xl": "16px",
        "3xl": "24px",
      },
    },
  },
  plugins: [],
};

export default config;
