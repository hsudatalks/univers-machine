import { connectionStatusClass } from "../lib/connectivity-state";

interface ConnectionStatusLightProps {
  className?: string;
  state?: string;
  title?: string;
}

function titleCase(value: string): string {
  if (!value) {
    return "";
  }

  return value.slice(0, 1).toUpperCase() + value.slice(1);
}

export function ConnectionStatusLight({
  className = "",
  state,
  title,
}: ConnectionStatusLightProps) {
  const normalizedState = (state || "checking").trim() || "checking";
  const label = title || titleCase(normalizedState);

  return (
    <span
      aria-label={label}
      className={`terminal-status terminal-status-dot ${connectionStatusClass(normalizedState)} ${className}`.trim()}
      role="status"
      title={label}
    />
  );
}
