type ShellStateProps = {
  label: string;
  message: string;
};

export function ShellState({ label, message }: ShellStateProps) {
  return (
    <main className="shell shell-state">
      <section className="state-panel">
        <span className="state-label">{label}</span>
        <p className="state-copy">{message}</p>
      </section>
    </main>
  );
}
