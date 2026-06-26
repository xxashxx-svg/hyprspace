// HyprSpace mark: an isometric cube — a "space" / dimension. Indigo top face, neutral sides.
// Uses style (not the fill attr) so the CSS custom properties resolve, and follows the theme accent.
export function Logo({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 3 L20.5 8 L12 13 L3.5 8 Z" style={{ fill: "var(--accent)" }} />
      <path d="M3.5 8 L12 13 L12 21 L3.5 16 Z" style={{ fill: "var(--text-1)" }} />
      <path d="M20.5 8 L20.5 16 L12 21 L12 13 Z" style={{ fill: "var(--text-3)" }} />
    </svg>
  );
}
