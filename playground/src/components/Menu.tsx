import { DRINKS, type Drink } from "../drinks";
import GalacticCup from "./GalacticCup";

export default function Menu({ score, onBuy }: { score: number; onBuy: (d: Drink) => void }) {
  return (
    <section className="card">
      <h2>Menú galáctico</h2>
      <div className="sub">Pagá con polvo de estrellas.</div>
      <div className="menu">
        {DRINKS.map((d) => (
          <button key={d.id} className="drink" disabled={score < d.price} onClick={() => onBuy(d)}>
            <span className="emoji">{d.galactic ? <GalacticCup /> : d.emoji}</span>
            <span>
              <div className="name">{d.name}</div>
              <div className="price">{d.price} ✦</div>
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}
