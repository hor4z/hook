export type Badge = { id: string; icon: string; label: string; done: boolean };

const TRACKS = ["Lo-fi en la Luna", "Synthwave Andrómeda", "Jazz de Júpiter"];

export default function Achievements({ badges, playing, track, onToggle, onNext }: { badges: Badge[]; playing: boolean; track: number; onToggle: () => void; onNext: () => void }) {
  return (
    <section className="card">
      <h2>Logros</h2>
      <div className="sub">
        {badges.filter((b) => b.done).length} de {badges.length} desbloqueados
      </div>
      <div className="badges">
        {badges.map((b) => (
          <div key={b.id} className={b.done ? "badge on" : "badge"}>
            <div>
              <span className="icon">{b.done ? b.icon : "🔒"}</span>
              {b.label}
            </div>
          </div>
        ))}
      </div>
      <div className="juke">
        <div className={playing ? "vinyl spin" : "vinyl"} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{TRACKS[track % TRACKS.length]}</div>
          <div style={{ fontSize: 12, color: "var(--muted)" }}>Rocola orbital</div>
        </div>
        <button className="btn" onClick={onToggle}>
          {playing ? "⏸" : "▶"}
        </button>
        <button className="btn" style={{ background: "rgba(255,255,255,0.08)" }} onClick={onNext}>
          ⏭
        </button>
      </div>
    </section>
  );
}
