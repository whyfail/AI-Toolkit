/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      // —— Apple-style typography scale ——
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          '"SF Pro Text"',
          '"Helvetica Neue"',
          '"Segoe UI"',
          "system-ui",
          "sans-serif",
        ],
        display: [
          "-apple-system",
          "BlinkMacSystemFont",
          '"SF Pro Display"',
          '"Helvetica Neue"',
          "system-ui",
          "sans-serif",
        ],
        mono: [
          '"SF Mono"',
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "Monaco",
          "Consolas",
          "monospace",
        ],
      },
      fontSize: {
        // 8pt grid aligned with Apple HIG text styles
        display: [
          "clamp(1.75rem, 2.4vw, 2.25rem)",
          { lineHeight: "1.05", letterSpacing: "-0.022em", fontWeight: "600" },
        ],
        title: ["1.375rem", { lineHeight: "1.15", letterSpacing: "-0.015em", fontWeight: "600" }],
        body: ["0.875rem", { lineHeight: "1.5", letterSpacing: "0", fontWeight: "400" }],
        caption: ["0.75rem", { lineHeight: "1.4", letterSpacing: "0.01em", fontWeight: "500" }],
        micro: ["0.6875rem", { lineHeight: "1.35", letterSpacing: "0.02em", fontWeight: "500" }],
      },
      borderRadius: {
        chip: "8px",
        card: "12px",
        sheet: "18px",
        modal: "22px",
      },
      boxShadow: {
        1: "0 1px 2px rgba(15, 23, 42, 0.06)",
        2: "0 8px 24px rgba(15, 23, 42, 0.08), 0 1px 2px rgba(15, 23, 42, 0.04)",
        3: "0 24px 70px rgba(15, 23, 42, 0.18), 0 2px 6px rgba(15, 23, 42, 0.06)",
        "accent-1": "0 1px 2px rgba(10, 132, 255, 0.3), 0 8px 24px rgba(10, 132, 255, 0.18)",
        "accent-2": "0 1px 2px rgba(10, 132, 255, 0.4), 0 14px 30px rgba(10, 132, 255, 0.26)",
        "danger-1": "0 1px 2px rgba(255, 59, 48, 0.3), 0 8px 24px rgba(255, 59, 48, 0.18)",
      },
      transitionTimingFunction: {
        // Apple's standard curves — derived from Fluid Interfaces
        spring: "cubic-bezier(0.32, 0.72, 0, 1)",
        "spring-bounce": "cubic-bezier(0.34, 1.56, 0.64, 1)",
        "out-quint": "cubic-bezier(0.16, 1, 0.3, 1)",
        "in-out-quart": "cubic-bezier(0.42, 0, 0.58, 1)",
      },
      transitionDuration: {
        press: "100ms",
        hover: "180ms",
        sheet: "380ms",
        modal: "280ms",
        tab: "220ms",
      },
      keyframes: {
        "scale-in": {
          from: { opacity: "0", transform: "scale(0.96)" },
          to: { opacity: "1", transform: "scale(1)" },
        },
        "fade-in": {
          from: { opacity: "0" },
          to: { opacity: "1" },
        },
        "slide-up": {
          from: { opacity: "0", transform: "translateY(8px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
        shimmer: {
          "0%, 100%": { opacity: "0.4" },
          "50%": { opacity: "0.8" },
        },
      },
      animation: {
        "scale-in": "scale-in 280ms cubic-bezier(0.16, 1, 0.3, 1) both",
        "fade-in": "fade-in 180ms cubic-bezier(0.16, 1, 0.3, 1) both",
        "slide-up": "slide-up 280ms cubic-bezier(0.16, 1, 0.3, 1) both",
        shimmer: "shimmer 1.4s ease-in-out infinite",
      },
    },
  },
  plugins: [],
};