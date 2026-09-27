import { useState } from "react";

const REPLIES = [
  "Blip blop. Eso suena delicioso en mi planeta.",
  "¿Sabías que en Neptuno el café se toma frío? Muy frío.",
  "Te recomiendo el Frappé Saturno, tiene anillos de caramelo.",
  "Mis tres ojos dicen que hoy es un gran día para un combo.",
  "Zorp. Eso no lo entendí, pero te sirvo otro latte.",
];

type Line = { me: boolean; text: string };

export default function Barista({ lines, onSay }: { lines: Line[]; onSay: (l: Line[]) => void }) {
  const [draft, setDraft] = useState("");

  const send = () => {
    if (!draft.trim()) return;
    const reply = REPLIES[Math.floor(Math.random() * REPLIES.length)];
    onSay([...lines, { me: true, text: draft.trim() }, { me: false, text: reply }].slice(-6));
    setDraft("");
  };

  return (
    <section className="card">
      <h2>👽 Zorp, el barista</h2>
      <div className="sub">Charlá mientras esperás tu pedido.</div>
      <div className="chat">
        {lines.map((l, i) => (
          <div key={i} className={l.me ? "bubble me" : "bubble"}>
            {l.text}
          </div>
        ))}
        <div className="chat-input">
          <input value={draft} onChange={(e) => setDraft(e.target.value)} onKeyDown={(e) => e.key === "Enter" && send()} placeholder="Decile algo a Zorp" />
          <button className="btn send" onClick={send}>
            Enviar
          </button>
        </div>
      </div>
    </section>
  );
}
