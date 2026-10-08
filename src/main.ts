import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import "pdfjs-dist/web/pdf_viewer.css";

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
