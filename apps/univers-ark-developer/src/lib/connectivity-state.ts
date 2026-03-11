export function connectionStateVariant(
  state: string | undefined,
): "neutral" | "success" | "warning" | "destructive" {
  switch (state) {
    case "running":
    case "ready":
    case "connected":
    case "direct":
      return "success";
    case "checking":
    case "starting":
    case "pending":
      return "warning";
    case "error":
    case "stopped":
    case "disconnected":
      return "destructive";
    default:
      return "neutral";
  }
}

export function connectionStatusClass(state: string | undefined): string {
  const normalized = (state || "checking").trim().toLowerCase() || "checking";
  return `status-${normalized}`;
}
