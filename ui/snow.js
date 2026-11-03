// Nieve del tema de Navidad: cuadraditos que caen por encima de la ventana sin tapar los clics.
window.Snow = (() => {
  const c = () => document.getElementById("snow");
  const W = 1000, H = 620, flakes = [];
  let on = false, last = 0;
  for (let i = 0; i < 110; i++) {
    flakes.push({ x: Math.random() * W, y: Math.random() * H, s: 2 + (i % 3), v: 14 + Math.random() * 26, w: Math.random() * 6.28, a: 0.35 + Math.random() * 0.5 });
  }
  function frame(t) {
    const el = c();
    if (!on || !el) return;
    const dt = Math.min(0.1, (t - last) / 1000);
    last = t;
    const g = el.getContext("2d");
    g.clearRect(0, 0, W, H);
    for (const f of flakes) {
      f.y += f.v * dt;
      f.w += dt;
      if (f.y > H) { f.y = -4; f.x = Math.random() * W; }
      g.fillStyle = `rgba(255,255,255,${f.a})`;
      g.fillRect(Math.round(f.x + Math.sin(f.w) * 8), Math.round(f.y), f.s, f.s);
    }
    requestAnimationFrame(frame);
  }
  return {
    toggle(v) {
      const el = c();
      if (v === on) return;
      on = v;
      if (el) el.getContext("2d").clearRect(0, 0, W, H);
      if (v) { last = performance.now(); requestAnimationFrame(frame); }
    }
  };
})();
if (document.documentElement.dataset.theme === "navidad") Snow.toggle(true);
