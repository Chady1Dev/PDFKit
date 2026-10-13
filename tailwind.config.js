/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{svelte,ts}"],
  theme: {
    extend: {
      fontFamily: {
        brand: ["'PaytoneOne'", "sans-serif"],
      },
    },
  },
  plugins: [],
};
