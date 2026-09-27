import { useRef, useState } from "react";

type Pop = { id: number; x: number; y: number; value: number };

export default function Clicker({ score, onScore }: { score: number; onScore: (n: number) => void }) {
  const [combo, setCombo] = useState(0);
  const [turn, setTurn] = useState(0);
  const [pops, setPops] = useState<Pop[]>([]);
  const last = useRef(0);

  const hit = (e: React.MouseEvent) => {
    const now = performance.now();
    const next = now - last.current < 400 ? combo + 1 : 1;
    last.current = now;
    setCombo(next);
    setTurn((t) => t + 1);
    const value = 1 + Math.floor(next / 5);
    onScore(value);
    const box = (e.currentTarget.parentElement as HTMLElement).getBoundingClientRect();
    const pop = { id: now, x: e.clientX - box.left, y: e.clientY - box.top, value };
    setPops((p) => [...p.slice(-12), pop]);
  };

  return (
    <section className="card clicker">
      <h2>Minería de polvo de estrellas</h2>
      <div className="sub">Golpeá el asteroide. Rápido suma combo.</div>
      <button className="asteroid" onClick={hit} aria-label="Luna" style={{ rotate: `${turn * 45}deg` }}>
        <span className="moon-surface" />
        <span className="moon-shade" />
      </button>
      <div className="score">
        {score.toLocaleString("es-AR")} <span className="star">✦</span>
      </div>
      <div className="combo">{combo >= 5 ? `¡Combo x${combo}!` : ""}</div>
      {pops.map((p) => (
        <span key={p.id} className="pop" style={{ left: p.x, top: p.y }}>
          +{p.value}
        </span>
      ))}
    </section>
  );
}
