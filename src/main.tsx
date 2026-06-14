import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LanguageProvider } from "./i18n";
import { ThemeProvider } from "./ThemeContext";
import "./index.css";

// The app is a window, not a web page, so the browser's context menu (Reload,
// View Source, Back) does not belong in it. Text fields are the exception: the
// Bitbucket API token is pasted in, and right-click → Paste is how a lot of
// people paste. Blocking it there made the field look broken.
document.addEventListener("contextmenu", (e) => {
  const target = e.target as HTMLElement | null;
  const editable = target?.closest("input, textarea, [contenteditable='true']");
  if (!editable) e.preventDefault();
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LanguageProvider>
      <ThemeProvider>
        <App />
      </ThemeProvider>
    </LanguageProvider>
  </React.StrictMode>,
);
