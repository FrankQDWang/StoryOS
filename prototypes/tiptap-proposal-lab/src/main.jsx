import React from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/noto-serif-sc/400.css";
import "@fontsource/noto-serif-sc/600.css";
import "@fontsource/noto-sans-sc/400.css";
import "@fontsource/noto-sans-sc/500.css";
import { App } from "./App.jsx";
import "./styles.css";

createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
