import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class"],
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}", "./lib/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        brand: {
          50: "#f0fdf9",
          100: "#ccfbef",
          200: "#99f6e0",
          300: "#66f4be",  // near-mint-light
          400: "#33ebb5",
          500: "#00ec97",  // near-mint (primary)
          600: "#00d386",  // near-mint-dark
          700: "#00a86b",
          800: "#008557",
          900: "#006d48"
        },
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
          900: "#0f0f0f",
          950: "#0a0a0a"
        }
      }
    }
  },
  plugins: []
};

export default config;
