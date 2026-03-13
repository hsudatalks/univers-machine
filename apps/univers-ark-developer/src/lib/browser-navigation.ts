export const ARK_BROWSER_BRIDGE_SOURCE = "ark-browser-bridge";

export type BrowserNavigationMode = "none" | "cooperative" | "proxy-injected";

export interface BrowserNavigationPayload {
  href: string;
  path?: string | null;
  title?: string | null;
}

export interface BrowserContextMenuPayload extends BrowserNavigationPayload {
  linkUrl?: string | null;
  imageUrl?: string | null;
  selectionText?: string | null;
  x: number;
  y: number;
}

export type BrowserBridgeMessage =
  | {
    source: typeof ARK_BROWSER_BRIDGE_SOURCE;
    mode?: BrowserNavigationMode;
    type: "ready" | "navigation";
    payload: BrowserNavigationPayload;
  }
  | {
    source: typeof ARK_BROWSER_BRIDGE_SOURCE;
    mode?: BrowserNavigationMode;
    type: "contextmenu";
    payload: BrowserContextMenuPayload;
  };

export interface BrowserContextMenuSnapshot {
  cacheKey: string;
  currentPath: string | null;
  currentUrl: string;
  entryPath: string;
  entryUrl: string;
  imageUrl: string | null;
  linkUrl: string | null;
  mode: BrowserNavigationMode;
  selectionText: string | null;
  title: string | null;
  updatedAt: number;
  x: number;
  y: number;
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
  if (
    candidate.source !== ARK_BROWSER_BRIDGE_SOURCE ||
    !candidate.payload ||
    typeof candidate.payload !== "object" ||
    typeof candidate.payload.href !== "string"
  ) {
    return false;
  }

  if (candidate.type === "ready" || candidate.type === "navigation") {
    return true;
  }

  return (
    candidate.type === "contextmenu" &&
    typeof (candidate.payload as Partial<BrowserContextMenuPayload>).x === "number" &&
    typeof (candidate.payload as Partial<BrowserContextMenuPayload>).y === "number"
  );
}
