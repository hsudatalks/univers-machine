import { Lock, Monitor, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { VncScreen, type VncScreenHandle } from "react-vnc";
import { startVncSession, stopVncSession } from "../lib/tauri";
import { Button } from "./ui/button";

interface ServerDesktopPaneProps {
    active: boolean;
    targetId: string;
    serverLabel: string;
}

type DesktopState =
    | { kind: "idle" }
    | { kind: "connecting" }
    | { kind: "connected"; wsUrl: string }
    | { kind: "credentials"; wsUrl: string }
    | { kind: "error"; message: string };

export function ServerDesktopPane({
    active,
    targetId,
    serverLabel,
}: ServerDesktopPaneProps) {
    const vncRef = useRef<VncScreenHandle>(null);
    const [state, setState] = useState<DesktopState>({ kind: "idle" });
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");

    const connect = useCallback(async () => {
        setState({ kind: "connecting" });

        try {
            const session = await startVncSession(targetId);
            setState({ kind: "connected", wsUrl: `ws://127.0.0.1:${session.wsPort}` });
        } catch (error) {
            setState({
                kind: "error",
                message: error instanceof Error ? error.message : String(error),
            });
        }
    }, [targetId]);

    const disconnect = useCallback(async () => {
        if (vncRef.current) {
            vncRef.current.disconnect();
        }

        try {
            await stopVncSession(targetId);
        } catch {
            // Swallow stop errors
        }

        setUsername("");
        setPassword("");
        setState({ kind: "idle" });
    }, [targetId]);

    const submitCredentials = useCallback(() => {
        if (vncRef.current && password) {
            const creds: Record<string, string> = { password };
            if (username) creds.username = username;
            vncRef.current.sendCredentials(creds);
            setState((prev) =>
                prev.kind === "credentials"
                    ? { kind: "connected", wsUrl: prev.wsUrl }
                    : prev
            );
        }
    }, [username, password]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            stopVncSession(targetId).catch(() => { });
        };
    }, [targetId]);

    const vncMounted = state.kind === "connected" || state.kind === "credentials";

    return (
        <article className="panel desktop-panel">
            <header className="desktop-panel-header">
                <div className="desktop-panel-title">
                    <Monitor size={14} />
                    <span>{serverLabel} Desktop</span>
                </div>

                <div className="desktop-panel-actions">
                    {(state.kind === "connected" || state.kind === "credentials") && (
                        <Button
                            aria-label="Disconnect VNC"
                            onClick={disconnect}
                            size="sm"
                            variant="ghost"
                        >
                            Disconnect
                        </Button>
                    )}
                    {(state.kind === "error" || state.kind === "idle") && (
                        <Button
                            aria-label="Connect to desktop"
                            onClick={connect}
                            size="sm"
                            variant="default"
                        >
                            <RefreshCw size={14} />
                            {state.kind === "error" ? "Retry" : "Connect"}
                        </Button>
                    )}
                </div>
            </header>

            <div className="desktop-panel-content">
                {state.kind === "idle" && (
                    <section className="state-panel">
                        <Monitor size={32} className="state-icon" />
                        <span className="state-label">Remote Desktop</span>
                        <p className="state-copy">
                            Connect to the VNC server on this machine to view and control its desktop.
                        </p>
                        <Button onClick={connect} variant="default" size="sm">
                            Connect
                        </Button>
                    </section>
                )}

                {state.kind === "connecting" && (
                    <section className="state-panel">
                        <div className="desktop-spinner" />
                        <span className="state-label">Connecting...</span>
                        <p className="state-copy">
                            Establishing SSH tunnel and VNC connection to {serverLabel}.
                        </p>
                    </section>
                )}

                {state.kind === "error" && (
                    <section className="state-panel">
                        <Monitor size={32} className="state-icon state-icon-error" />
                        <span className="state-label">Connection Failed</span>
                        <p className="state-copy">{state.message}</p>
                        <Button onClick={connect} variant="default" size="sm">
                            <RefreshCw size={14} />
                            Retry
                        </Button>
                    </section>
                )}

                {state.kind === "credentials" && (
                    <section className="state-panel desktop-credentials">
                        <Lock size={32} className="state-icon" />
                        <span className="state-label">VNC Password Required</span>
                        <p className="state-copy">
                            The VNC server on {serverLabel} requires authentication.
                        </p>
                        <form
                            className="desktop-credentials-form"
                            onSubmit={(e) => { e.preventDefault(); submitCredentials(); }}
                        >
                            <input
                                type="text"
                                className="desktop-credentials-input"
                                placeholder="Username (optional)"
                                value={username}
                                onChange={(e) => setUsername(e.target.value)}
                                autoFocus
                            />
                            <input
                                type="password"
                                className="desktop-credentials-input"
                                placeholder="VNC password"
                                value={password}
                                onChange={(e) => setPassword(e.target.value)}
                            />
                            <Button type="submit" variant="default" size="sm" disabled={!password}>
                                Authenticate
                            </Button>
                        </form>
                    </section>
                )}

                {vncMounted && (
                    <VncScreen
                        ref={vncRef}
                        url={state.kind === "connected" ? state.wsUrl : (state as any).wsUrl}
                        scaleViewport
                        resizeSession
                        focusOnClick
                        autoConnect
                        qualityLevel={2}
                        compressionLevel={9}
                        showDotCursor
                        style={{
                            width: "100%",
                            height: "100%",
                            cursor: "default",
                            display: (active && state.kind !== "credentials") ? "block" : "none",
                        }}
                        onCredentialsRequired={() => {
                            setState((prev) =>
                                prev.kind === "connected"
                                    ? { kind: "credentials", wsUrl: prev.wsUrl }
                                    : prev
                            );
                        }}
                        onDisconnect={() => {
                            setState((prev) => {
                                if (prev.kind === "credentials") return prev;
                                return {
                                    kind: "error",
                                    message: "VNC connection was closed by the remote host.",
                                };
                            });
                        }}
                    />
                )}
            </div>
        </article>
    );
}
