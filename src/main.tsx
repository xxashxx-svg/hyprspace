import ReactDOM from "react-dom/client";
// DM Sans = the UI typeface (T3 Code's font); mono fonts below are for the terminals
import "@fontsource-variable/dm-sans";
import "@fontsource/cascadia-code/400.css";
import "@fontsource/cascadia-code/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "./styles/tokens.css";
import "./App.css";
import App from "./App";
import { AuthGate } from "./components/AuthGate";
import { AccessGate } from "./components/AccessGate";

// NO React.StrictMode — its double-mount corrupts the xterm terminal lifecycle.
// AccessGate is the temporary private-beta code wall (outermost); AuthGate is sign-in.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AccessGate>
    <AuthGate>
      <App />
    </AuthGate>
  </AccessGate>,
);
