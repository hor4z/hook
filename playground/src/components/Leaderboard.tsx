const RIVALS = [
  { name: "Capitana Vega", pts: 1840, avatar: "🧑‍🚀" },
  { name: "Robot R-2", pts: 1210, avatar: "🤖" },
  { name: "Gato Cósmico", pts: 760, avatar: "🐱" },
  { name: "Pulpo Marte", pts: 330, avatar: "🐙" },
];

export default function Leaderboard({ score }: { score: number }) {
  const all = [...RIVALS, { name: "Vos", pts: score, avatar: "⭐" }].sort((a, b) => b.pts - a.pts);
  return (
    <section className="card board">
      <h2>Tabla de la galaxia</h2>
      <div className="sub">Los mejores mineros de la semana.</div>
      <ol>
        {all.map((p, i) => (
          <li key={p.name} className={p.name === "Vos" ? "me" : undefined}>
            <span className="rank">{i + 1}</span>
            <span>{p.avatar}</span>
            <span>{p.name}</span>
            <span className="pts">{p.pts.toLocaleString("es-AR")}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}
