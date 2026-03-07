import { FitAddon } from "@xterm/addon-fit";
import { useEffect, useRef, useState } from "react";
import { Terminal } from "xterm";
import {
  attachTerminal,
  listenTerminalExit,
  listenTerminalOutput,
  resizeTerminal,
  writeTerminal,
} from "../lib/tauri";
import type { DeveloperTarget } from "../types";

interface TerminalPaneProps {
  target: DeveloperTarget;
}

export function TerminalPane({ target }: TerminalPaneProps) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const activeTargetIdRef = useRef(target.id);
  const [status, setStatus] = useState("Connecting");

  useEffect(() => {
    activeTargetIdRef.current = target.id;
  }, [target.id]);

  useEffect(() => {
    if (!mountRef.current) {
      return undefined;
    }

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
    terminal.open(mountRef.current);
    fitAddon.fit();
    terminal.focus();

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    const syncSize = () => {
      const currentTargetId = activeTargetIdRef.current;
      if (!currentTargetId) {
        return;
      }

      void resizeTerminal(
        currentTargetId,
        Math.max(terminal.cols, 40),
        Math.max(terminal.rows, 12),
      ).catch(() => undefined);
    };

    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      syncSize();
    });

    resizeObserver.observe(mountRef.current);

    const onWindowResize = () => {
      fitAddon.fit();
      syncSize();
    };

    window.addEventListener("resize", onWindowResize);

    const inputDisposable = terminal.onData((data) => {
      void writeTerminal(activeTargetIdRef.current, data).catch((error) => {
        const message = error instanceof Error ? error.message : String(error);
        terminal.writeln(`\r\n[write failed] ${message}`);
      });
    });

    const resizeDisposable = terminal.onResize(({ cols, rows }) => {
      void resizeTerminal(activeTargetIdRef.current, cols, rows).catch(
        () => undefined,
      );
    });

    let unlistenOutput: (() => void) | null = null;
    let unlistenExit: (() => void) | null = null;

    void listenTerminalOutput((payload) => {
      if (payload.targetId !== activeTargetIdRef.current) {
        return;
      }

      terminal.write(payload.data);
      setStatus("Connected");
    }).then((unlisten) => {
      unlistenOutput = unlisten;
    });

    void listenTerminalExit((payload) => {
      if (payload.targetId !== activeTargetIdRef.current) {
        return;
      }

      terminal.writeln(`\r\n[session closed] ${payload.reason}`);
      setStatus("Disconnected");
    }).then((unlisten) => {
      unlistenExit = unlisten;
    });

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener("resize", onWindowResize);
      inputDisposable.dispose();
      resizeDisposable.dispose();
      unlistenOutput?.();
      unlistenExit?.();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const terminal = terminalRef.current;

    if (!terminal) {
      return undefined;
    }

    void attachTerminal(target.id)
      .then((snapshot) => {
        if (cancelled || !terminalRef.current) {
          return;
        }

        terminalRef.current.reset();
        if (snapshot.output) {
          terminalRef.current.write(snapshot.output);
        }

        fitAddonRef.current?.fit();
        void resizeTerminal(
          target.id,
          Math.max(terminalRef.current.cols, 40),
          Math.max(terminalRef.current.rows, 12),
        ).catch(() => undefined);
        terminalRef.current.focus();
        setStatus("Connected");
      })
      .catch((error) => {
        if (cancelled || !terminalRef.current) {
          return;
        }

        const message = error instanceof Error ? error.message : String(error);
        terminalRef.current.reset();
        terminalRef.current.writeln(`[attach failed] ${message}`);
        setStatus("Error");
      });

    return () => {
      cancelled = true;
    };
  }, [target.id]);

  return (
    <>
      <header className="panel-header terminal-header">
        <span className="panel-title">Terminal</span>

        <div className="terminal-meta">
          <span className="panel-meta">{target.host}</span>
          <span className={`terminal-status status-${status.toLowerCase()}`}>
            {status}
          </span>
        </div>
      </header>

      <div
        className="terminal-mount"
        onClick={() => terminalRef.current?.focus()}
        ref={mountRef}
      />
    </>
  );
}
