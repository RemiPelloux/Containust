(() => {
  const body = document.body;
  const toggle = document.querySelector("[data-menu-toggle]");
  const search = document.querySelector("[data-doc-search]");

  if (toggle) {
    toggle.addEventListener("click", () => {
      body.classList.toggle("nav-open");
      toggle.setAttribute("aria-expanded", String(body.classList.contains("nav-open")));
    });
  }

  for (const block of document.querySelectorAll(".codeblock")) {
    const pre = block.querySelector("pre");
    if (!pre) continue;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "copy-btn";
    btn.textContent = "Copy";
    btn.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(pre.textContent ?? "");
        btn.textContent = "Copied";
        btn.classList.add("copied");
        window.setTimeout(() => {
          btn.textContent = "Copy";
          btn.classList.remove("copied");
        }, 1400);
      } catch {
        btn.textContent = "Failed";
      }
    });
    block.appendChild(btn);
  }

  const headings = [...document.querySelectorAll(".content h2[id], .content h3[id]")];
  const toc = document.querySelector("[data-toc]");
  if (toc && headings.length) {
    const title = document.createElement("p");
    title.className = "toc-title";
    title.textContent = "On this page";
    toc.appendChild(title);
    for (const heading of headings) {
      const link = document.createElement("a");
      link.href = `#${heading.id}`;
      link.textContent = heading.textContent ?? "";
      link.className = heading.tagName === "H3" ? "h3" : "h2";
      toc.appendChild(link);
    }

    const tocLinks = [...toc.querySelectorAll("a")];
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          const id = entry.target.id;
          for (const link of tocLinks) {
            link.classList.toggle("active", link.getAttribute("href") === `#${id}`);
          }
        }
      },
      { rootMargin: "-20% 0px -70% 0px", threshold: 0 },
    );
    for (const heading of headings) observer.observe(heading);
  }

  if (search) {
    const targets = [...document.querySelectorAll("[data-searchable]")];
    const filter = () => {
      const q = search.value.trim().toLowerCase();
      for (const el of targets) {
        const hay = (el.getAttribute("data-searchable") || el.textContent || "").toLowerCase();
        el.classList.toggle("hidden-match", Boolean(q) && !hay.includes(q));
      }
    };
    search.addEventListener("input", filter);
    document.addEventListener("keydown", (event) => {
      if (event.key === "/" && document.activeElement !== search) {
        event.preventDefault();
        search.focus();
      }
      if (event.key === "Escape") {
        search.blur();
        body.classList.remove("nav-open");
      }
    });
  }
})();
