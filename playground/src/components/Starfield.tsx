import { useEffect, useRef } from "react";

export default function Starfield() {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current!;
    const ctx = canvas.getContext("2d")!;
    let frame = 0;
    const stars = Array.from({ length: 220 }, () => ({ x: Math.random(), y: Math.random(), z: Math.random() * 0.8 + 0.2 }));
    const resize = () => {
      canvas.width = innerWidth * devicePixelRatio;
      canvas.height = innerHeight * devicePixelRatio;
    };
    resize();
    addEventListener("resize", resize);
    const draw = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      for (const s of stars) {
        s.x -= 0.00012 * s.z;
        if (s.x < 0) s.x = 1;
        ctx.globalAlpha = 0.35 + s.z * 0.6;
        ctx.fillStyle = "#fff";
        ctx.fillRect(s.x * canvas.width, s.y * canvas.height, s.z * 2 * devicePixelRatio, s.z * 2 * devicePixelRatio);
      }
      frame = requestAnimationFrame(draw);
    };
    draw();
    return () => {
      cancelAnimationFrame(frame);
      removeEventListener("resize", resize);
    };
  }, []);

  return <canvas ref={ref} className="stars" style={{ width: "100vw", height: "100vh" }} />;
}
