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
  ownerId: symbol | null;
  src: string;
}

const browserFrames = new Map<string, CachedBrowserFrame>();

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
