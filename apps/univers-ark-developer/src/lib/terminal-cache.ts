import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "xterm";
import {
  attachTerminal,
  listenTerminalExit,
  listenTerminalOutput,
  resizeTerminal,
  writeTerminal,
} from "./tauri";

type StatusListener = () => void;

interface CachedTerminalSession {
  attachPromise?: Promise<void>;
  fitAddon: FitAddon;
  hostElement: HTMLDivElement;
  outputUnlisten?: () => void;
  exitUnlisten?: () => void;
  ownerId: symbol | null;
  status: string;
  statusListeners: Set<StatusListener>;
  targetId: string;
  terminal: Terminal;
  hasAttachedSnapshot: boolean;
}

const DEFAULT_TERMINAL_STATUS = "Connecting";
const terminalSessions = new Map<string, CachedTerminalSession>();

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

function emitStatus(session: CachedTerminalSession) {
  for (const listener of session.statusListeners) {
    listener();
  }
}

function setStatus(session: CachedTerminalSession, status: string) {
  if (session.status === status) {
    return;
  }

  session.status = status;
  emitStatus(session);
}

function syncTerminalSize(session: CachedTerminalSession) {
  void resizeTerminal(
    session.targetId,
    Math.max(session.terminal.cols, 40),
    Math.max(session.terminal.rows, 12),
  ).catch(() => undefined);
}

function fitTerminal(session: CachedTerminalSession) {
  session.fitAddon.fit();

  if (session.terminal.rows > 0) {
    session.terminal.refresh(0, session.terminal.rows - 1);
  }

  syncTerminalSize(session);
}

function refreshTerminalSnapshot(session: CachedTerminalSession) {
  if (session.attachPromise) {
    return session.attachPromise;
  }

  setStatus(session, DEFAULT_TERMINAL_STATUS);

  session.attachPromise = attachTerminal(session.targetId)
    .then((snapshot) => {
      session.terminal.reset();
      if (snapshot.output) {
        session.terminal.write(snapshot.output);
      }
      session.hasAttachedSnapshot = true;
      setStatus(session, "Connected");
      fitTerminal(session);
    })
    .catch((error) => {
      const message = error instanceof Error ? error.message : String(error);
      session.terminal.reset();
      session.terminal.writeln(`[attach failed] ${message}`);
      session.hasAttachedSnapshot = false;
      setStatus(session, "Error");
    })
    .finally(() => {
      session.attachPromise = undefined;
    });

  return session.attachPromise;
}

function createTerminalSession(targetId: string): CachedTerminalSession {
  const hostElement = document.createElement("div");
  hostElement.style.width = "100%";
  hostElement.style.height = "100%";

  const terminal = new Terminal({
    allowTransparency: true,
    convertEol: true,
    cursorBlink: false,
    fontFamily: "Iosevka, SFMono-Regular, Consolas, monospace",
    fontSize: 12,
    lineHeight: 1,
    scrollback: 1500,
    theme: {
      background: "#0d1117",
      cursor: "#d6f3dd",
      foreground: "#d6f3dd",
      selectionBackground: "#334155",
    },
  });

  const fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(hostElement);

  const session: CachedTerminalSession = {
    fitAddon,
    hostElement,
    ownerId: null,
    status: DEFAULT_TERMINAL_STATUS,
    statusListeners: new Set(),
    targetId,
    terminal,
    hasAttachedSnapshot: false,
  };

  terminal.onData((data) => {
    void writeTerminal(targetId, data).catch((error) => {
      const message = error instanceof Error ? error.message : String(error);
      terminal.writeln(`\r\n[write failed] ${message}`);
    });
  });

  terminal.onResize(({ cols, rows }) => {
    void resizeTerminal(targetId, cols, rows).catch(() => undefined);
  });

  void listenTerminalOutput((payload) => {
    if (payload.targetId !== targetId) {
      return;
    }

    terminal.write(payload.data);
    setStatus(session, "Connected");
  }).then((unlisten) => {
    session.outputUnlisten = unlisten;
  });

  void listenTerminalExit((payload) => {
    if (payload.targetId !== targetId) {
      return;
    }

    terminal.writeln(`\r\n[session closed] ${payload.reason}`);
    session.hasAttachedSnapshot = false;
    setStatus(session, "Disconnected");
  }).then((unlisten) => {
    session.exitUnlisten = unlisten;
  });

  ensureParkingLotElement().append(hostElement);
  void refreshTerminalSnapshot(session);

  return session;
}

function terminalSession(targetId: string): CachedTerminalSession {
  const existingSession = terminalSessions.get(targetId);

  if (existingSession) {
    return existingSession;
  }

  const nextSession = createTerminalSession(targetId);
  terminalSessions.set(targetId, nextSession);
  return nextSession;
}

export function getTerminalStatus(targetId: string): string {
  return terminalSessions.get(targetId)?.status ?? DEFAULT_TERMINAL_STATUS;
}

export function subscribeTerminalStatus(
  targetId: string,
  listener: StatusListener,
): () => void {
  const session = terminalSession(targetId);
  session.statusListeners.add(listener);

  return () => {
    session.statusListeners.delete(listener);
  };
}

export function claimTerminalSession(
  targetId: string,
  ownerId: symbol,
  mountElement: HTMLDivElement,
  options?: { autoFocus?: boolean },
) {
  const session = terminalSession(targetId);

  if (session.hostElement.parentElement !== mountElement) {
    mountElement.replaceChildren(session.hostElement);
  }

  session.ownerId = ownerId;

  if (!session.hasAttachedSnapshot || session.status !== "Connected") {
    void refreshTerminalSnapshot(session);
  } else {
    fitTerminal(session);
  }

  if (options?.autoFocus) {
    session.terminal.focus();
  }
}

export function releaseTerminalSession(targetId: string, ownerId: symbol) {
  const session = terminalSessions.get(targetId);

  if (!session || session.ownerId !== ownerId) {
    return;
  }

  session.ownerId = null;
  ensureParkingLotElement().append(session.hostElement);
}

export function fitClaimedTerminalSession(targetId: string, ownerId: symbol) {
  const session = terminalSessions.get(targetId);

  if (!session || session.ownerId !== ownerId) {
    return;
  }

  fitTerminal(session);
}

export function focusClaimedTerminalSession(targetId: string, ownerId: symbol) {
  const session = terminalSessions.get(targetId);

  if (!session || session.ownerId !== ownerId) {
    return;
  }

  session.terminal.focus();
}
