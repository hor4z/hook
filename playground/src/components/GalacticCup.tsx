export default function GalacticCup({ size = 40 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" className="galactic-cup" aria-hidden>
      <defs>
        <linearGradient id="gc-body" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#312e81" />
          <stop offset="1" stopColor="#1e1b4b" />
        </linearGradient>
        <radialGradient id="gc-nebula" cx="0.35" cy="0.4" r="0.8">
          <stop offset="0" stopColor="#f0abfc" />
          <stop offset="0.45" stopColor="#8b5cf6" />
          <stop offset="1" stopColor="#1d4ed8" />
        </radialGradient>
        <linearGradient id="gc-steam" x1="0" y1="1" x2="0" y2="0">
          <stop offset="0" stopColor="#c4b5fd" stopOpacity="0.9" />
          <stop offset="1" stopColor="#f0abfc" stopOpacity="0" />
        </linearGradient>
      </defs>
      <path className="gc-steam" d="M24 18c-4-5 4-7 0-12M34 18c-4-5 4-7 0-12" stroke="url(#gc-steam)" strokeWidth="3" strokeLinecap="round" fill="none" />
      <path d="M46 30h4a7 7 0 0 1 0 14h-5" stroke="#6366f1" strokeWidth="4" fill="none" strokeLinecap="round" />
      <path d="M12 24h36v18a14 14 0 0 1-14 14h-8a14 14 0 0 1-14-14z" fill="url(#gc-body)" stroke="#818cf8" strokeWidth="1.5" />
      <ellipse cx="30" cy="25" rx="17" ry="4.5" fill="url(#gc-nebula)" />
      <path d="M33 22.5a3.2 3.2 0 1 0 0 5 4 4 0 1 1 0-5z" fill="#fef9c3" />
      <circle className="gc-star" cx="20" cy="36" r="1.3" fill="#fde68a" />
      <circle className="gc-star gc-star-2" cx="36" cy="44" r="1" fill="#f5d0fe" />
      <circle className="gc-star gc-star-3" cx="27" cy="48" r="0.9" fill="#bfdbfe" />
      <path className="gc-star gc-star-2" d="M40 34l1 2.2 2.2 1-2.2 1-1 2.2-1-2.2-2.2-1 2.2-1z" fill="#fef3c7" />
      <rect x="10" y="55" width="40" height="3" rx="1.5" fill="#4338ca" opacity="0.6" />
    </svg>
  );
}
