import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { api } from "./api";
import { SettingsProvider } from "./SettingsContext";
import { loadSettings } from "./settings";
import "./App.css";

const initial = loadSettings();

// The Docker CLI path must be known before the first status check runs.
api
  .setDockerPath(initial.dockerPath)
  .catch(() => {})
  .finally(() => {
    ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
      <React.StrictMode>
        <SettingsProvider initial={initial}>
          <App />
        </SettingsProvider>
      </React.StrictMode>,
    );
  });
