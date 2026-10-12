import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { runNativeSmoke } from "./smoke";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

void runNativeSmoke().catch((error: unknown) => {
  // Only a missing/broken native test command reaches this boundary. Keep the
  // normal UI usable and make an explicitly requested harness failure visible.
  console.error("Keyloom native smoke harness could not initialize", error);
});
