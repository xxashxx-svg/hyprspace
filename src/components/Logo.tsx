// HyprSpace mark: three tiled panes. Inherits color via currentColor.
export function Logo({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <rect x="2" y="2" width="8" height="20" rx="1.8" />
      <rect x="11.5" y="2" width="10.5" height="9" rx="1.8" />
      <rect x="11.5" y="13" width="10.5" height="9" rx="1.8" />
    </svg>
  );
}
