import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class"],
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}", "./lib/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        brand: {
          50: "#f4f9ff",
          100: "#d9e9ff",
          200: "#b3d3ff",
          300: "#8abdff",
          400: "#589dff",
          500: "#2f7df4",
          600: "#1f62d1",
          700: "#1748a8",
          800: "#133a87",
          900: "#102f6d"
        }
      }
    }
  },
  plugins: []
};

export default config;
