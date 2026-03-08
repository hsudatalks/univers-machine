import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "xterm";
import {
  attachTerminal,
  clipboardRead,
  clipboardWrite,
  listenTerminalExit,
  listenTerminalOutput,
  resizeTerminal,
  restartTerminal,
  writeTerminal,
} from "./tauri";

type StatusListener = () => void;

interface CachedTerminalSession {
  attachPromise?: Promise<void>;
  fitAddon: FitAddon;
  fontScale: number;
  hostElement: HTMLDivElement;
  outputUnlisten?: () => void;
  exitUnlisten?: () => void;
  ownerId: symbol | null;
  readyForInput: boolean;
  pendingWrites: string[];
  status: string;
  statusListeners: Set<StatusListener>;
  targetId: string;
  terminal: Terminal;
  hasAttachedSnapshot: boolean;
}

const DEFAULT_TERMINAL_STATUS = "Connecting";
const DEFAULT_TERMINAL_FONT_SIZE = 12;
// eslint-disable-next-line no-control-regex
const DEVICE_ATTRIBUTES_RESPONSE_PATTERN = /^\x1b\[[\?>][\d;]*c$/;
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

function applyTerminalFontScale(
  session: CachedTerminalSession,
  fontScale: number | undefined,
) {
  const nextFontScale = Number.isFinite(fontScale) ? Math.max(fontScale ?? 1, 0.5) : 1;

  if (session.fontScale === nextFontScale) {
    return;
  }

  session.fontScale = nextFontScale;
  session.terminal.options.fontSize = Math.max(
    9,
    Math.round(DEFAULT_TERMINAL_FONT_SIZE * nextFontScale * 10) / 10,
  );
}

// Suppress Device Attributes (DA) responses at the parser level.
// When the remote shell sends a DA query (ESC[c or ESC[>c), xterm.js
// normally responds with ESC[?...c or ESC[>0;10;1c via onData. These
// responses get sent to the PTY and echoed as literal text "0;10;1c".
// We intercept DA queries so xterm.js never generates a response.
// IMPORTANT: terminal.reset() clears registered handlers, so this
// must be called after every reset.
function suppressDeviceAttributeResponses(terminal: Terminal) {
  terminal.parser.registerCsiHandler({ final: "c" }, () => true);
  terminal.parser.registerCsiHandler({ final: "c", prefix: ">" }, () => true);
  terminal.parser.registerCsiHandler({ final: "c", prefix: "?" }, () => true);
}

// Strip residual DA response text that leaked into the PTY buffer
// from previous sessions. The full response was ESC[>0;10;1c but the
// ESC was consumed by the remote terminal, leaving "[>N;N;Nc" or
// "N;N;Nc" as literal text.
// eslint-disable-next-line no-control-regex
const DA_RESIDUE_PATTERN = /\[?>?\d+;\d+;\d+c/g;

function loadTerminalSnapshot(
  session: CachedTerminalSession,
  loader: (targetId: string) => Promise<{ output: string }>,
) {
  if (session.attachPromise) {
    return session.attachPromise;
  }

  setStatus(session, DEFAULT_TERMINAL_STATUS);
  session.readyForInput = false;
  session.pendingWrites = [];
  session.hasAttachedSnapshot = false;

  session.attachPromise = loader(session.targetId)
    .then((snapshot) => {
      session.terminal.reset();
      suppressDeviceAttributeResponses(session.terminal);
      if (snapshot.output) {
        session.terminal.write(snapshot.output.replaceAll(DA_RESIDUE_PATTERN, ""));
      }
      session.hasAttachedSnapshot = true;
      setStatus(session, "Connected");
      fitTerminal(session);

      setTimeout(() => {
        session.readyForInput = true;
        for (const data of session.pendingWrites) {
          // Filter out Device Attributes responses (e.g. ESC[>0;10;1c)
          // that xterm.js sends automatically — they are not user input.
          if (DEVICE_ATTRIBUTES_RESPONSE_PATTERN.test(data)) {
            continue;
          }
          void writeTerminal(session.targetId, data).catch(() => undefined);
        }
        session.pendingWrites = [];
      }, 500);
    })
    .catch((error) => {
      const message = error instanceof Error ? error.message : String(error);
      session.terminal.reset();
      suppressDeviceAttributeResponses(session.terminal);
      session.terminal.writeln(`[attach failed] ${message}`);
      session.hasAttachedSnapshot = false;
      setStatus(session, "Error");
    })
    .finally(() => {
      session.attachPromise = undefined;
    });

  return session.attachPromise;
}

function refreshTerminalSnapshot(session: CachedTerminalSession) {
  return loadTerminalSnapshot(session, attachTerminal);
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
    macOptionClickForcesSelection: true,
    rightClickSelectsWord: true,
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

  suppressDeviceAttributeResponses(terminal);

  const session: CachedTerminalSession = {
    fitAddon,
    fontScale: 1,
    hostElement,
    ownerId: null,
    status: DEFAULT_TERMINAL_STATUS,
    statusListeners: new Set(),
    targetId,
    terminal,
    hasAttachedSnapshot: false,
    readyForInput: false,
    pendingWrites: [],
  };

  // Auto-copy selected text to native clipboard via Rust arboard
  terminal.onSelectionChange(() => {
    const selection = terminal.getSelection();
    if (selection) {
      void clipboardWrite(selection).catch(() => undefined);
    }
  });

  // Ctrl+Shift+C = copy selection, Ctrl+Shift+V = paste from clipboard
  terminal.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown") {
      return true;
    }

    // Ctrl+Shift+C — copy selected text
    if (event.ctrlKey && event.shiftKey && event.code === "KeyC") {
      const selection = terminal.getSelection();
      if (selection) {
        void clipboardWrite(selection).catch(() => undefined);
      }
      return false;
    }

    // Ctrl+Shift+V — paste
    if (event.ctrlKey && event.shiftKey && event.code === "KeyV") {
      void clipboardRead()
        .then((text) => {
          if (text && session.readyForInput) {
            void writeTerminal(targetId, text).catch(() => undefined);
          }
        })
        .catch(() => undefined);
      return false;
    }

    return true;
  });

  // Workaround for xterm.js regression #4781: Shift+drag selection
  // vanishes on mouse release when tmux mouse mode is active.
  // When Shift is held during mousedown, temporarily tell xterm.js
  // to disable mouse reporting. Re-enable on mouseup.
  // NOTE: terminal.write() sends data AS IF from the application to
  // the terminal emulator — this is the correct direction for DECSET
  // sequences (unlike writeTerminal which writes to the PTY stdin).
  hostElement.addEventListener(
    "mousedown",
    (event) => {
      if (event.shiftKey) {
        // Tell xterm.js to disable mouse tracking modes
        terminal.write(
          "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l",
        );

        const reenable = () => {
          window.removeEventListener("mouseup", reenable);
          // Give xterm.js time to finalize the selection, then
          // re-enable mouse reporting so tmux keeps working
          setTimeout(() => {
            terminal.write(
              "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h",
            );
          }, 150);
        };

        window.addEventListener("mouseup", reenable);
      }
    },
    true,
  );

  // Right-click paste from clipboard
  hostElement.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    void clipboardRead()
      .then((text) => {
        if (text && session.readyForInput) {
          void writeTerminal(targetId, text).catch(() => undefined);
        }
      })
      .catch(() => undefined);
  });

  terminal.onData((data) => {
    // Filter out Device Attributes responses (e.g. ESC[>0;10;1c)
    // that xterm.js sends automatically — they are not user input.
    if (DEVICE_ATTRIBUTES_RESPONSE_PATTERN.test(data)) {
      return;
    }

    if (!session.readyForInput) {
      session.pendingWrites.push(data);
      return;
    }

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

    terminal.write(payload.data.replaceAll(DA_RESIDUE_PATTERN, ""));
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
  options?: { autoFocus?: boolean; fontScale?: number },
) {
  const session = terminalSession(targetId);

  if (session.hostElement.parentElement !== mountElement) {
    mountElement.replaceChildren(session.hostElement);
  }

  session.ownerId = ownerId;
  applyTerminalFontScale(session, options?.fontScale);

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

export function restartTerminalSession(targetId: string) {
  const session = terminalSession(targetId);
  return loadTerminalSnapshot(session, restartTerminal);
}
