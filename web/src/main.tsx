import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
// Langue et thème appliqués à <html> avant le premier rendu.
import "./i18n";
import "./theme";
import { App } from "./App";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
