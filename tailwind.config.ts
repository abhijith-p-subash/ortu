import type { Config } from "tailwindcss";

export default {
  content: ["./src/**/*.{html,js,svelte,ts}"],

  theme: {
    extend: {
      // Semantic, theme-aware colors backed by CSS variables (RGB triplets) so
      // they flip between dark and light via [data-theme]. All support Tailwind
      // opacity modifiers (e.g. text-fg/70, bg-overlay/[0.05]).
      colors: {
        app: "rgb(var(--app) / <alpha-value>)",         // deepest background
        surface: "rgb(var(--surface) / <alpha-value>)", // panels / modals
        raised: "rgb(var(--raised) / <alpha-value>)",   // raised surfaces
        fg: "rgb(var(--fg) / <alpha-value>)",           // primary foreground / text
        overlay: "rgb(var(--overlay) / <alpha-value>)", // neutral hover / border overlay
      },
    },
  },

  plugins: [],
} as Config;
