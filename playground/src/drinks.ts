export type Drink = { id: string; emoji: string; name: string; price: number; galactic?: boolean };

export const DRINKS: Drink[] = [
  { id: "latte", emoji: "☕", name: "Latte de Luna", price: 10, galactic: true },
  { id: "nebula", emoji: "🧋", name: "Té Nebulosa", price: 25 },
  { id: "saturn", emoji: "🪐", name: "Frappé Saturno", price: 60 },
  { id: "comet", emoji: "☄️", name: "Espresso Cometa", price: 120 },
  { id: "aurora", emoji: "🌌", name: "Smoothie Aurora", price: 250 },
  { id: "hole", emoji: "🕳️", name: "Agujero Negro", price: 500 },
];
