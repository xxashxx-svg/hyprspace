import ReactDOM from "react-dom/client";
import App from "./App";

// NO React.StrictMode — its double-mount corrupts the xterm terminal lifecycle.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
