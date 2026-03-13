export const ARK_BROWSER_BRIDGE_SOURCE = "ark-browser-bridge";

export type BrowserNavigationMode = "none" | "cooperative" | "proxy-injected";

export interface BrowserNavigationPayload {
  href: string;
  path?: string | null;
  title?: string | null;
}

export interface BrowserBridgeMessage {
  source: typeof ARK_BROWSER_BRIDGE_SOURCE;
  mode?: BrowserNavigationMode;
  type: "ready" | "navigation";
  payload: BrowserNavigationPayload;
}

export interface BrowserNavigationSnapshot {
  cacheKey: string;
  currentPath: string | null;
  currentUrl: string | null;
  entryPath: string;
  entryUrl: string;
  mode: BrowserNavigationMode;
  title: string | null;
  updatedAt: number;
}

export function deriveBrowserPath(url: string): string {
  try {
    const parsed = new URL(url);
    const path = `${parsed.pathname}${parsed.search}${parsed.hash}`;
    return path || "/";
  } catch {
    return url;
  }
}

export function isBrowserBridgeMessage(value: unknown): value is BrowserBridgeMessage {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<BrowserBridgeMessage>;
  return (
    candidate.source === ARK_BROWSER_BRIDGE_SOURCE &&
    (candidate.type === "ready" || candidate.type === "navigation") &&
    !!candidate.payload &&
    typeof candidate.payload === "object" &&
    typeof candidate.payload.href === "string"
  );
}
