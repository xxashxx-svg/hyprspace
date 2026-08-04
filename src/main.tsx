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
import { initAgentStatus } from "./stores/agentStatus";
import { initUsage } from "./stores/usage";
import { AuthGate } from "./components/AuthGate";

// NO React.StrictMode — its double-mount corrupts the xterm terminal lifecycle.
// AuthGate no longer gates: it restores the session, applies the saved theme, and mounts the app.
// An account is never required — HyprSpace runs your own CLIs on your own machine. Sign-in is
// offered from Settings → Account, and the (dormant) entitlement check still runs underneath.
initAgentStatus();
initUsage();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AuthGate>
    <App />
  </AuthGate>,
);
