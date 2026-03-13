import {
  type BrowserContextMenuSnapshot,
  type BrowserNavigationSnapshot,
  deriveBrowserPath,
  isBrowserBridgeMessage,
} from "./browser-navigation";

interface BrowserFrameDescriptor {
  cacheKey: string;
  frameVersion: number;
  isActive: boolean;
  src: string;
  title: string;
}

interface CachedBrowserFrame {
  cacheKey: string;
  frameVersion: number;
  iframe: HTMLIFrameElement;
  lastError: string | null;
  lastAccessedAt: number;
  lastLoadedAt: number;
  ownerId: symbol | null;
  sessionState: "idle" | "loading" | "loaded" | "error";
  src: string;
}

export interface BrowserFrameSnapshot {
  cacheKey: string;
  frameVersion: number;
  hasOwner: boolean;
  lastAccessedAt: number;
  lastError: string | null;
  lastLoadedAt: number;
  sessionState: "idle" | "loading" | "loaded" | "error";
  src: string;
  title: string;
}

const browserFrames = new Map<string, CachedBrowserFrame>();
const browserContextMenuSnapshots = new Map<string, BrowserContextMenuSnapshot>();
const browserNavigationSnapshots = new Map<string, BrowserNavigationSnapshot>();
const HOT_BROWSER_FRAME_LIMIT = 12;
const browserContextMenuListeners = new Set<() => void>();
const browserNavigationListeners = new Set<() => void>();
const BROWSER_IFRAME_SANDBOX =
  "allow-scripts allow-same-origin allow-forms allow-downloads";

let parkingLotElement: HTMLDivElement | null = null;
let bridgeListenerInstalled = false;

function ensureParkingLotElement(): HTMLDivElement {
  if (parkingLotElement) {
    return parkingLotElement;
  }

  parkingLotElement = document.createElement("div");
  parkingLotElement.setAttribute("aria-hidden", "true");
  parkingLotElement.style.position = "fixed";
  parkingLotElement.style.left = "-200vw";
  parkingLotElement.style.top = "0";
  parkingLotElement.style.width = "1440px";
  parkingLotElement.style.height = "900px";
  parkingLotElement.style.overflow = "hidden";
  parkingLotElement.style.pointerEvents = "none";
  parkingLotElement.style.opacity = "0";
  document.body.append(parkingLotElement);

  return parkingLotElement;
}

function applyFrameDescriptor(
  frame: CachedBrowserFrame,
  descriptor: BrowserFrameDescriptor,
) {
  frame.iframe.className = `browser-frame ${descriptor.isActive ? "is-active" : ""}`.trim();
  frame.iframe.title = descriptor.title;
  frame.lastAccessedAt = Date.now();

  if (frame.src !== descriptor.src || frame.frameVersion !== descriptor.frameVersion) {
    frame.sessionState = descriptor.src ? "loading" : "idle";
    frame.lastError = null;
    frame.iframe.src = descriptor.src;
    frame.src = descriptor.src;
    frame.frameVersion = descriptor.frameVersion;
    browserContextMenuSnapshots.delete(descriptor.cacheKey);
    browserNavigationSnapshots.set(descriptor.cacheKey, {
      cacheKey: descriptor.cacheKey,
      currentPath: null,
      currentUrl: null,
      entryPath: deriveBrowserPath(descriptor.src),
      entryUrl: descriptor.src,
      mode: "none",
      title: descriptor.title,
      updatedAt: Date.now(),
    });
    notifyBrowserContextMenuListeners();
    notifyBrowserNavigationListeners();
  }
}

function moveFrameToParkingLot(frame: CachedBrowserFrame) {
  frame.iframe.className = "browser-frame is-parked";
  ensureParkingLotElement().append(frame.iframe);
}

function cachedBrowserFrame(
  descriptor: BrowserFrameDescriptor,
): CachedBrowserFrame {
  ensureBrowserBridgeListener();
  const existingFrame = browserFrames.get(descriptor.cacheKey);

  if (existingFrame) {
    applyFrameDescriptor(existingFrame, descriptor);
    return existingFrame;
  }

  const iframe = document.createElement("iframe");
  iframe.className = "browser-frame";
  iframe.referrerPolicy = "no-referrer";
  iframe.setAttribute("sandbox", BROWSER_IFRAME_SANDBOX);

  const nextFrame: CachedBrowserFrame = {
    cacheKey: descriptor.cacheKey,
    frameVersion: descriptor.frameVersion,
    iframe,
    lastError: null,
    lastAccessedAt: Date.now(),
    lastLoadedAt: 0,
    ownerId: null,
    sessionState: "idle",
    src: "",
  };

  iframe.addEventListener("load", () => {
    nextFrame.sessionState = "loaded";
    nextFrame.lastLoadedAt = Date.now();
    nextFrame.lastError = null;
    const existingSnapshot = browserNavigationSnapshots.get(descriptor.cacheKey);
    browserNavigationSnapshots.set(descriptor.cacheKey, {
      cacheKey: descriptor.cacheKey,
      currentPath: existingSnapshot?.currentPath ?? null,
      currentUrl: existingSnapshot?.currentUrl ?? null,
      entryPath: deriveBrowserPath(nextFrame.src),
      entryUrl: nextFrame.src,
      mode: existingSnapshot?.mode ?? "none",
      title: existingSnapshot?.title ?? descriptor.title,
      updatedAt: Date.now(),
    });
    notifyBrowserNavigationListeners();
  });

  iframe.addEventListener("error", () => {
    nextFrame.sessionState = "error";
    nextFrame.lastError = "Failed to load iframe content.";
  });

  browserFrames.set(descriptor.cacheKey, nextFrame);
  applyFrameDescriptor(nextFrame, descriptor);
  moveFrameToParkingLot(nextFrame);

  return nextFrame;
}

export function preloadBrowserFrames(
  descriptors: Array<Omit<BrowserFrameDescriptor, "isActive">>,
) {
  for (const descriptor of descriptors) {
    const frame = cachedBrowserFrame({
      ...descriptor,
      isActive: false,
    });

    frame.ownerId = null;
    moveFrameToParkingLot(frame);
  }

  pruneBrowserFramesToLimit(HOT_BROWSER_FRAME_LIMIT);
}

export function syncBrowserFrames(
  ownerId: symbol,
  stageElement: HTMLDivElement,
  descriptors: BrowserFrameDescriptor[],
) {
  const nextKeys = new Set(descriptors.map((descriptor) => descriptor.cacheKey));

  for (const frame of browserFrames.values()) {
    if (frame.ownerId === ownerId && !nextKeys.has(frame.cacheKey)) {
      frame.ownerId = null;
      moveFrameToParkingLot(frame);
    }
  }

  for (const descriptor of descriptors) {
    const frame = cachedBrowserFrame(descriptor);
    frame.ownerId = ownerId;

    if (frame.iframe.parentElement !== stageElement) {
      stageElement.append(frame.iframe);
    }
  }

  pruneBrowserFramesToLimit(HOT_BROWSER_FRAME_LIMIT);
}

export function releaseBrowserFrames(ownerId: symbol) {
  for (const frame of browserFrames.values()) {
    if (frame.ownerId !== ownerId) {
      continue;
    }

    frame.ownerId = null;
    moveFrameToParkingLot(frame);
  }
}

export function resetBrowserFrame(cacheKey: string) {
  const frame = browserFrames.get(cacheKey);

  if (frame) {
    frame.iframe.remove();
    browserFrames.delete(cacheKey);
  }

  if (browserNavigationSnapshots.delete(cacheKey)) {
    notifyBrowserNavigationListeners();
  }

  if (browserContextMenuSnapshots.delete(cacheKey)) {
    notifyBrowserContextMenuListeners();
  }
}

export function pruneBrowserFrames(retainedKeys: string[]) {
  const retainedKeySet = new Set(retainedKeys);
  let removedNavigation = false;
  let removedContextMenu = false;

  for (const [cacheKey, frame] of browserFrames.entries()) {
    if (retainedKeySet.has(cacheKey)) {
      continue;
    }

    frame.iframe.remove();
    browserFrames.delete(cacheKey);
    removedContextMenu = browserContextMenuSnapshots.delete(cacheKey) || removedContextMenu;
    removedNavigation = browserNavigationSnapshots.delete(cacheKey) || removedNavigation;
  }

  if (removedContextMenu) {
    notifyBrowserContextMenuListeners();
  }

  if (removedNavigation) {
    notifyBrowserNavigationListeners();
  }
}

function pruneBrowserFramesToLimit(limit: number) {
  if (browserFrames.size <= limit) {
    return;
  }

  const pruneCandidates = [...browserFrames.values()]
    .filter((frame) => frame.ownerId === null)
    .sort((left, right) => left.lastAccessedAt - right.lastAccessedAt);

  let overflowCount = browserFrames.size - limit;
  let removedNavigation = false;
  let removedContextMenu = false;

  for (const frame of pruneCandidates) {
    if (overflowCount <= 0) {
      break;
    }

    frame.iframe.remove();
    browserFrames.delete(frame.cacheKey);
    removedContextMenu =
      browserContextMenuSnapshots.delete(frame.cacheKey) || removedContextMenu;
    removedNavigation =
      browserNavigationSnapshots.delete(frame.cacheKey) || removedNavigation;
    overflowCount -= 1;
  }

  if (removedContextMenu) {
    notifyBrowserContextMenuListeners();
  }

  if (removedNavigation) {
    notifyBrowserNavigationListeners();
  }
}

function notifyBrowserContextMenuListeners() {
  for (const listener of browserContextMenuListeners) {
    listener();
  }
}

function notifyBrowserNavigationListeners() {
  for (const listener of browserNavigationListeners) {
    listener();
  }
}

function ensureBrowserBridgeListener() {
  if (bridgeListenerInstalled || typeof window === "undefined") {
    return;
  }

  window.addEventListener("message", handleBrowserBridgeMessage);
  bridgeListenerInstalled = true;
}

function handleBrowserBridgeMessage(event: MessageEvent) {
  if (!event.source || !isBrowserBridgeMessage(event.data)) {
    return;
  }

  for (const frame of browserFrames.values()) {
    if (frame.iframe.contentWindow !== event.source) {
      continue;
    }

    const expectedOrigin = browserOriginFromUrl(frame.src);
    if (expectedOrigin && event.origin !== expectedOrigin) {
      continue;
    }

    if (event.data.type === "contextmenu") {
      browserContextMenuSnapshots.set(frame.cacheKey, {
        cacheKey: frame.cacheKey,
        currentPath:
          event.data.payload.path?.trim() || deriveBrowserPath(event.data.payload.href),
        currentUrl: event.data.payload.href,
        entryPath: deriveBrowserPath(frame.src),
        entryUrl: frame.src,
        imageUrl: event.data.payload.imageUrl?.trim() || null,
        linkUrl: event.data.payload.linkUrl?.trim() || null,
        mode: event.data.mode ?? "cooperative",
        selectionText: event.data.payload.selectionText?.trim() || null,
        title: event.data.payload.title?.trim() || frame.iframe.title,
        updatedAt: Date.now(),
        x: event.data.payload.x,
        y: event.data.payload.y,
      });
      notifyBrowserContextMenuListeners();
      break;
    }

    browserNavigationSnapshots.set(frame.cacheKey, {
      cacheKey: frame.cacheKey,
      currentPath:
        event.data.payload.path?.trim() || deriveBrowserPath(event.data.payload.href),
      currentUrl: event.data.payload.href,
      entryPath: deriveBrowserPath(frame.src),
      entryUrl: frame.src,
      mode: event.data.mode ?? "cooperative",
      title: event.data.payload.title?.trim() || frame.iframe.title,
      updatedAt: Date.now(),
    });
    notifyBrowserNavigationListeners();
    break;
  }
}

function browserOriginFromUrl(url: string): string | null {
  try {
    return new URL(url).origin;
  } catch {
    return null;
  }
}

export function getBrowserNavigationSnapshot(
  cacheKey: string,
): BrowserNavigationSnapshot | null {
  return browserNavigationSnapshots.get(cacheKey) ?? null;
}

export function getBrowserContextMenuSnapshot(
  cacheKey: string,
): BrowserContextMenuSnapshot | null {
  return browserContextMenuSnapshots.get(cacheKey) ?? null;
}

export function clearBrowserContextMenu(cacheKey: string) {
  if (browserContextMenuSnapshots.delete(cacheKey)) {
    notifyBrowserContextMenuListeners();
  }
}

export function subscribeBrowserContextMenu(listener: () => void): () => void {
  browserContextMenuListeners.add(listener);
  return () => {
    browserContextMenuListeners.delete(listener);
  };
}

export function subscribeBrowserNavigation(listener: () => void): () => void {
  browserNavigationListeners.add(listener);
  return () => {
    browserNavigationListeners.delete(listener);
  };
}

export function listBrowserFrameSnapshots(): BrowserFrameSnapshot[] {
  return [...browserFrames.values()]
    .map((frame) => ({
      cacheKey: frame.cacheKey,
      frameVersion: frame.frameVersion,
      hasOwner: frame.ownerId !== null,
      lastAccessedAt: frame.lastAccessedAt,
      lastError: frame.lastError,
      lastLoadedAt: frame.lastLoadedAt,
      sessionState: frame.sessionState,
      src: frame.src,
      title: frame.iframe.title,
    }))
    .sort((left, right) => right.lastAccessedAt - left.lastAccessedAt);
}
