type FileTileArtworkProps = {
  kind: "directory" | "file" | "symlink";
  tone: "default" | "code" | "json" | "text";
};

function FolderArtwork() {
  return (
    <svg
      aria-hidden="true"
      className="file-tile-artwork"
      viewBox="0 0 64 64"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <linearGradient id="folder-tab" x1="0" x2="1" y1="0" y2="1">
          <stop offset="0%" stopColor="#d9ecff" />
          <stop offset="100%" stopColor="#8cc0ff" />
        </linearGradient>
        <linearGradient id="folder-body" x1="0" x2="0.85" y1="0" y2="1">
          <stop offset="0%" stopColor="#9ecbff" />
          <stop offset="100%" stopColor="#5b97ff" />
        </linearGradient>
      </defs>
      <path
        d="M10 19c0-2.8 2.2-5 5-5h12.6c2.1 0 4.1.8 5.6 2.3l1.5 1.5c.9.9 2 1.2 3.3 1.2H49c2.8 0 5 2.2 5 5v4H10z"
        fill="url(#folder-tab)"
      />
      <path
        d="M10 24h44v22c0 3.3-2.7 6-6 6H16c-3.3 0-6-2.7-6-6z"
        fill="url(#folder-body)"
      />
      <path
        d="M14 27h36c1.7 0 3 1.3 3 3v1H11v-1c0-1.7 1.3-3 3-3z"
        fill="rgba(255,255,255,0.22)"
      />
      <path
        d="M16 25.5h34"
        stroke="rgba(255,255,255,0.35)"
        strokeLinecap="round"
        strokeWidth="2"
      />
    </svg>
  );
}

function FileArtwork({ tone }: { tone: FileTileArtworkProps["tone"] }) {
  const accent =
    tone === "json"
      ? "#8bd0ff"
      : tone === "text"
        ? "#a2b5ff"
        : tone === "code"
          ? "#d5b3ff"
          : "#c7d0dc";

  const marks =
    tone === "json" ? (
      <text
        fill="#0d1117"
        fontFamily="SFMono-Regular, Menlo, monospace"
        fontSize="9"
        fontWeight="700"
        x="20"
        y="42"
      >
        {"{}"}
      </text>
    ) : tone === "text" ? (
      <>
        <path d="M20 31h20" stroke="#3b4552" strokeLinecap="round" strokeWidth="2.2" />
        <path d="M20 37h18" stroke="#556070" strokeLinecap="round" strokeWidth="2.2" />
        <path d="M20 43h14" stroke="#556070" strokeLinecap="round" strokeWidth="2.2" />
      </>
    ) : tone === "code" ? (
      <>
        <path
          d="M24 31l-4 5 4 5"
          fill="none"
          stroke="#314056"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2.4"
        />
        <path
          d="M40 31l4 5-4 5"
          fill="none"
          stroke="#314056"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2.4"
        />
      </>
    ) : null;

  return (
    <svg
      aria-hidden="true"
      className="file-tile-artwork"
      viewBox="0 0 64 64"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <linearGradient id={`file-body-${tone}`} x1="0" x2="1" y1="0" y2="1">
          <stop offset="0%" stopColor="#ffffff" />
          <stop offset="100%" stopColor="#dfe6ef" />
        </linearGradient>
      </defs>
      <path
        d="M18 10h18l12 12v28c0 2.2-1.8 4-4 4H22c-2.2 0-4-1.8-4-4z"
        fill={`url(#file-body-${tone})`}
      />
      <path d="M36 10v9c0 2.2 1.8 4 4 4h8" fill={accent} opacity="0.9" />
      <path
        d="M18 10h18l12 12v28c0 2.2-1.8 4-4 4H22c-2.2 0-4-1.8-4-4z"
        fill="none"
        stroke="rgba(60,72,88,0.18)"
        strokeWidth="1.4"
      />
      <rect fill={accent} height="4" opacity="0.95" rx="2" width="26" x="19" y="16" />
      {marks}
    </svg>
  );
}

function SymlinkArtwork() {
  return (
    <div className="file-tile-stack">
      <FileArtwork tone="default" />
      <span className="file-tile-badge" aria-hidden="true">
        ↗
      </span>
    </div>
  );
}

export function FileTileArtwork({ kind, tone }: FileTileArtworkProps) {
  if (kind === "directory") {
    return <FolderArtwork />;
  }

  if (kind === "symlink") {
    return <SymlinkArtwork />;
  }

  return <FileArtwork tone={tone} />;
}
