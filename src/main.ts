import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import "pdfjs-dist/web/pdf_viewer.css";

// Block browser reload shortcuts (F5, Ctrl+R, Command+R, Ctrl+F5) app-wide so the desktop app never reloads accidentally
window.addEventListener(
  "keydown",
  (e: KeyboardEvent) => {
    if (e.key === "F5" || ((e.ctrlKey || e.metaKey) && (e.key === "r" || e.key === "R"))) {
      e.preventDefault();
    }
  },
  { capture: true, passive: false }
);

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
