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
  lastAccessedAt: number;
  ownerId: symbol | null;
  src: string;
}

export interface BrowserFrameSnapshot {
  cacheKey: string;
  frameVersion: number;
  hasOwner: boolean;
  lastAccessedAt: number;
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
  parkingLotElement.style.inset = "auto auto -200vh -200vw";
  parkingLotElement.style.width = "1px";
  parkingLotElement.style.height = "1px";
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
  frame.iframe.className = `browser-frame ${descriptor.isActive ? "is-active" : ""}`;
  frame.iframe.title = descriptor.title;
  frame.lastAccessedAt = Date.now();

  if (frame.src !== descriptor.src || frame.frameVersion !== descriptor.frameVersion) {
    frame.iframe.src = descriptor.src;
    frame.src = descriptor.src;
    frame.frameVersion = descriptor.frameVersion;
  }
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
    lastAccessedAt: Date.now(),
    ownerId: null,
    src: "",
  };

  browserFrames.set(descriptor.cacheKey, nextFrame);
  applyFrameDescriptor(nextFrame, descriptor);
  ensureParkingLotElement().append(iframe);

  return nextFrame;
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
      ensureParkingLotElement().append(frame.iframe);
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
    ensureParkingLotElement().append(frame.iframe);
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
      src: frame.src,
      title: frame.iframe.title,
    }))
    .sort((left, right) => right.lastAccessedAt - left.lastAccessedAt);
}
