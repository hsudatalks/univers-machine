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
const HOT_BROWSER_FRAME_LIMIT = 40;

let parkingLotElement: HTMLDivElement | null = null;

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
  }
}

function moveFrameToParkingLot(frame: CachedBrowserFrame) {
  frame.iframe.className = "browser-frame is-parked";
  ensureParkingLotElement().append(frame.iframe);
}

function cachedBrowserFrame(
  descriptor: BrowserFrameDescriptor,
): CachedBrowserFrame {
  const existingFrame = browserFrames.get(descriptor.cacheKey);

  if (existingFrame) {
    applyFrameDescriptor(existingFrame, descriptor);
    return existingFrame;
  }

  const iframe = document.createElement("iframe");
  iframe.className = "browser-frame";
  iframe.referrerPolicy = "no-referrer";

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

export function pruneBrowserFrames(retainedKeys: string[]) {
  const retainedKeySet = new Set(retainedKeys);

  for (const [cacheKey, frame] of browserFrames.entries()) {
    if (retainedKeySet.has(cacheKey)) {
      continue;
    }

    frame.iframe.remove();
    browserFrames.delete(cacheKey);
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

  for (const frame of pruneCandidates) {
    if (overflowCount <= 0) {
      break;
    }

    frame.iframe.remove();
    browserFrames.delete(frame.cacheKey);
    overflowCount -= 1;
  }
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
