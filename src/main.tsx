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
// AuthGate is the sign-in gate: nothing below it mounts until you're signed in, unless no
// supabase project is configured (see .env.example), in which case it falls open.
initAgentStatus();
initUsage();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AuthGate>
    <App />
  </AuthGate>,
);
