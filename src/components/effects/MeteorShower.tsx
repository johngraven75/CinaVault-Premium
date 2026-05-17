import React, { useRef, useEffect } from "react";

interface Props {
  className?: string;
  meteorCount?: number;
}

export default function MeteorShower({ className = "", meteorCount = 50 }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    let animId: number;

    const resize = () => {
      canvas.width = canvas.offsetWidth * 2;
      canvas.height = canvas.offsetHeight * 2;
      ctx.scale(2, 2);
    };
    resize();
    window.addEventListener("resize", resize);

    const colors = [
      "255,255,255", // white
      "100,149,237", // cornflower blue
      "255,215,0",   // gold
      "255,69,0",    // orange red
      "255,20,147",  // deep pink
      "0,255,255",   // cyan
      "255,255,0",   // yellow
      "138,43,226"   // blue violet
    ];

    const meteors = Array.from({ length: meteorCount }, () => {
      const size = Math.random() * 2 + 1;
      const speed = Math.random() * 3 + 2;
      const angle = Math.random() * Math.PI * 2;
      return {
        x: Math.random() * canvas.offsetWidth,
        y: Math.random() * canvas.offsetHeight,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        length: Math.random() * 80 + 20,
        size,
        color: colors[Math.floor(Math.random() * colors.length)],
        life: Math.random() * 0.5 + 0.5, // 0.5 to 1.0
        maxLife: Math.random() * 0.5 + 0.5
      };
    });

    const draw = () => {
      const w = canvas.offsetWidth;
      const h = canvas.offsetHeight;
      ctx.clearRect(0, 0, w * 2, h * 2); // clear with scaled dimensions

      meteors.forEach((m) => {
        m.x += m.vx;
        m.y += m.vy;
        m.life -= 0.01;

        if (m.life <= 0 || m.x < -100 || m.x > w + 100 || m.y < -100 || m.y > h + 100) {
          // reset meteor
          m.x = Math.random() * w;
          m.y = Math.random() * h;
          m.life = m.maxLife;
          const angle = Math.random() * Math.PI * 2;
          const speed = Math.random() * 3 + 2;
          m.vx = Math.cos(angle) * speed;
          m.vy = Math.sin(angle) * speed;
          m.length = Math.random() * 80 + 20;
          m.color = colors[Math.floor(Math.random() * colors.length)];
        }

        const alpha = m.life / m.maxLife;
        ctx.beginPath();
        ctx.moveTo(m.x, m.y);
        ctx.lineTo(m.x - m.vx * m.length * 0.1, m.y - m.vy * m.length * 0.1);
        ctx.lineWidth = m.size;
        ctx.strokeStyle = `rgba(${m.color},${alpha})`;
        ctx.setLineDash([]);
        ctx.stroke();

        // Add a faint tail
        for (let i = 1; i < 3; i++) {
          const tailAlpha = alpha * (1 - i * 0.3);
          ctx.beginPath();
          ctx.moveTo(m.x - m.vx * m.length * 0.1 * i, m.y - m.vy * m.length * 0.1 * i);
          ctx.lineTo(m.x - m.vx * m.length * 0.1 * (i + 1), m.y - m.vy * m.length * 0.1 * (i + 1));
          ctx.lineWidth = m.size * (1 - i * 0.3);
          ctx.strokeStyle = `rgba(${m.color},${tailAlpha * 0.5})`;
          ctx.stroke();
        }
      });

      animId = requestAnimationFrame(draw);
    };

    draw();

    return () => {
      cancelAnimationFrame(animId);
      window.removeEventListener("resize", resize);
    };
  }, [meteorCount]);

  return <canvas ref={canvasRef} className={`meteor-canvas ${className}`} />;
}