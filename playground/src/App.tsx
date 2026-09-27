import { useEffect, useState } from "react";
import Achievements, { type Badge } from "./components/Achievements";
import Barista from "./components/Barista";
import Clicker from "./components/Clicker";
import Leaderboard from "./components/Leaderboard";
import Menu from "./components/Menu";
import type { Drink } from "./drinks";
import Starfield from "./components/Starfield";

const COLORS = ["#8b5cf6", "#34d399", "#f472b6", "#fbbf24", "#60a5fa"];

export default function App() {
  const [score, setScore] = useState(0);
  const [total, setTotal] = useState(0);
  const [orders, setOrders] = useState<Drink[]>([]);
  const [lines, setLines] = useState([{ me: false, text: "¡Bienvenide al Café Orbital! ¿Qué te sirvo hoy?" }]);
  const [playing, setPlaying] = useState(false);
  const [track, setTrack] = useState(0);
  const [toast, setToast] = useState<string | null>(null);

  const badges: Badge[] = [
    { id: "first", icon: "⛏️", label: "Primer golpe", done: total >= 1 },
    { id: "hundred", icon: "💯", label: "100 de polvo", done: total >= 100 },
    { id: "order", icon: "☕", label: "Primer pedido", done: orders.length >= 1 },
    { id: "chat", icon: "💬", label: "Amigo de Zorp", done: lines.length >= 3 },
    { id: "dj", icon: "🎧", label: "DJ orbital", done: playing },
    { id: "hole", icon: "🕳️", label: "Tragado", done: orders.some((o) => o.id === "hole") },
  ];
  const unlocked = badges.filter((b) => b.done).length;

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 2200);
    return () => clearTimeout(t);
  }, [toast]);

  const buy = (d: Drink) => {
    setScore((s) => s - d.price);
    setOrders((o) => [...o, d]);
    setToast(`${d.emoji} ${d.name} en camino por el teletransportador`);
  };

  return (
    <>
      <Starfield />
      <div className="app">
        <header className="topbar">
          <div className="brand">
            <span className="brand-planet" />
            Café Orbital
          </div>
          <div className="wallet">
            <span>✦</span>
            <strong>{score.toLocaleString("es-AR")}</strong>
            <span style={{ color: "var(--muted)" }}>· {orders.length} pedidos</span>
          </div>
        </header>

        <div className="grid">
          <Clicker
            score={score}
            onScore={(n) => {
              setScore((s) => s + n);
              setTotal((t) => t + n);
            }}
          />
          <Menu score={score} onBuy={buy} />
        </div>

        <div className="row">
          <Barista lines={lines} onSay={setLines} />
          <Leaderboard score={total} />
          <Achievements badges={badges} playing={playing} track={track} onToggle={() => setPlaying((p) => !p)} onNext={() => setTrack((t) => t + 1)} />
        </div>
      </div>

      {unlocked > 0 && (
        <div key={unlocked}>
          {Array.from({ length: 40 }, (_, i) => (
            <span key={i} className="confetti" style={{ left: `${(i * 37) % 100}vw`, background: COLORS[i % COLORS.length], animationDelay: `${(i % 10) * 40}ms` }} />
          ))}
        </div>
      )}
      {toast && <div className="toast">{toast}</div>}
    </>
  );
}
