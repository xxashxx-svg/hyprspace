import { useState } from "react";

// blurs sensitive text (emails) until clicked — so screen shares / streams don't leak it
export function Blurred({ text }: { text: string }) {
  const [show, setShow] = useState(false);
  return (
    <span
      className={`blurred${show ? " show" : ""}`}
      onClick={(e) => {
        e.stopPropagation();
        setShow(!show);
      }}
      title={show ? "Click to hide" : "Click to reveal"}
    >
      {text}
    </span>
  );
}
