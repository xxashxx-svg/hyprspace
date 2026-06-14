import ReactDOM from "react-dom/client";
// bundle the mono fonts so terminals render crisp + identical regardless of what's installed
import "@fontsource/cascadia-code/400.css";
import "@fontsource/cascadia-code/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import App from "./App";
import { LicenseGate } from "./components/LicenseGate";

// NO React.StrictMode — its double-mount corrupts the xterm terminal lifecycle.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <LicenseGate>
    <App />
  </LicenseGate>,
);
