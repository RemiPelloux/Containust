(() => {
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (reduced) return;

  const brand = document.querySelector(".brand-line");
  if (brand) {
    brand.animate(
      [
        { letterSpacing: "0.14em", filter: "blur(4px)" },
        { letterSpacing: "-0.03em", filter: "blur(0)" },
      ],
      { duration: 1100, easing: "cubic-bezier(0.22, 1, 0.36, 1)", fill: "forwards" },
    );
  }

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        entry.target.classList.add("in-view");
        observer.unobserve(entry.target);
      }
    },
    { threshold: 0.18 },
  );

  for (const el of document.querySelectorAll(".pillars article, .band-inner, .compose, .close")) {
    el.style.opacity = "0";
    el.style.transform = "translateY(22px)";
    el.style.transition = "opacity 700ms cubic-bezier(0.22, 1, 0.36, 1), transform 700ms cubic-bezier(0.22, 1, 0.36, 1)";
    observer.observe(el);
  }

  const style = document.createElement("style");
  style.textContent = `.in-view { opacity: 1 !important; transform: none !important; }`;
  document.head.appendChild(style);
})();
