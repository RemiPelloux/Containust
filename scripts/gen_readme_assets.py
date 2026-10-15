#!/usr/bin/env python3
"""Generate the animated SVGs embedded in README.md.

GitHub renders README images through a sandboxed <img>, which strips scripts
and external fonts but keeps CSS animations inside the SVG. Every animation
here is CSS-only and switched off under `prefers-reduced-motion`, leaving the
final frame fully visible.

    python3 scripts/gen_readme_assets.py   # writes docs/assets/readme/*.svg
"""

from __future__ import annotations

import html
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "docs" / "assets" / "readme"

VOID = "#071018"
DOCK = "#0e1c2e"
STEEL = "#8fa8bc"
COPPER = "#d4894a"
EMBER = "#f0a56a"
PAPER = "#f4efe6"
INK = "#e8e2d6"
GREEN = "#7fd1a8"
YELLOW = "#f2c66d"
CYAN = "#7cc4e8"
RED = "#ef8a80"
LINE = "rgba(143,168,188,0.18)"

SERIF = "Fraunces, 'Iowan Old Style', Georgia, 'Times New Roman', serif"
SANS = "Manrope, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
MONO = "'IBM Plex Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"

REDUCED = (
    "@media (prefers-reduced-motion: reduce) { * { animation: none !important; }"
    " .pulse, .flow { opacity: 0; } }"
)


def esc(text: str) -> str:
    return html.escape(text, quote=False)


def svg(width: int, height: int, title: str, body: str, style: str) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="t">
<title id="t">{esc(title)}</title>
<style>
{style}
{REDUCED}
</style>
{body}
</svg>
"""


def grid(width: int, height: int, step: int = 32) -> str:
    lines = [
        f'<path d="M{x} 0V{height}"/>' for x in range(step, width, step)
    ] + [f'<path d="M0 {y}H{width}"/>' for y in range(step, height, step)]
    return f'<g stroke="{LINE}" stroke-width="1" opacity="0.35">{"".join(lines)}</g>'


# --------------------------------------------------------------------------
# Hero
# --------------------------------------------------------------------------


def node(x: int, y: int, name: str, lines: list[str], cls: str) -> str:
    w, h = 244, 44 + 22 * len(lines)
    rows = "".join(
        f'<text x="{x + 18}" y="{y + 54 + 22 * i}" class="mono dim">{esc(line)}</text>'
        for i, line in enumerate(lines)
    )
    return f"""<g class="node {cls}">
  <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" fill="{DOCK}" stroke="{LINE}"/>
  <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" fill="none" class="halo"/>
  <circle cx="{x + 18}" cy="{y + 20}" r="4" class="led"/>
  <text x="{x + 30}" y="{y + 24}" class="mono head">COMPONENT {esc(name)}</text>
  {rows}
</g>"""


def hero() -> str:
    width, height = 1280, 460
    style = f"""
.serif {{ font-family: {SERIF}; }}
.sans {{ font-family: {SANS}; }}
.mono {{ font-family: {MONO}; font-size: 13px; }}
.head {{ fill: {EMBER}; font-weight: 600; letter-spacing: .02em; }}
.dim {{ fill: {STEEL}; }}
.rise {{ animation: rise 1.1s cubic-bezier(.22,1,.36,1) both; }}
.d1 {{ animation-delay: .15s; }} .d2 {{ animation-delay: .3s; }} .d3 {{ animation-delay: .45s; }}
@keyframes rise {{ from {{ opacity: 0; transform: translateY(14px); }} to {{ opacity: 1; transform: none; }} }}
.glow {{ animation: breathe 7s ease-in-out infinite; transform-origin: 900px 200px; }}
@keyframes breathe {{ 0%,100% {{ opacity: .55; transform: scale(1); }} 50% {{ opacity: .9; transform: scale(1.08); }} }}
.wire {{ stroke: {STEEL}; stroke-opacity: .35; stroke-width: 2; fill: none; }}
.pulse {{ stroke: {EMBER}; stroke-width: 3; fill: none; stroke-linecap: round;
  stroke-dasharray: 14 520; animation: flow 3.2s linear infinite; }}
.p2 {{ animation-delay: 1.1s; }} .p3 {{ animation-delay: 2.1s; }}
@keyframes flow {{ from {{ stroke-dashoffset: 534; }} to {{ stroke-dashoffset: 0; }} }}
.node {{ animation: rise 1s cubic-bezier(.22,1,.36,1) both; }}
.n1 {{ animation-delay: .5s; }} .n2 {{ animation-delay: .7s; }} .n3 {{ animation-delay: .9s; }}
.halo {{ stroke: {COPPER}; stroke-width: 1.5; opacity: 0; animation: halo 6s ease-in-out infinite; }}
.n1 .halo {{ animation-delay: 1s; }} .n2 .halo {{ animation-delay: 3s; }} .n3 .halo {{ animation-delay: 5s; }}
@keyframes halo {{ 0%,100% {{ opacity: 0; }} 12% {{ opacity: 1; }} 30% {{ opacity: 0; }} }}
.led {{ fill: {GREEN}; animation: led 2.4s ease-in-out infinite; }}
@keyframes led {{ 0%,100% {{ opacity: 1; }} 50% {{ opacity: .35; }} }}
.ghost {{ animation: ghost 6s ease-in-out infinite; }}
@keyframes ghost {{ 0%,100% {{ opacity: .55; }} 50% {{ opacity: .25; }} }}
.caret {{ fill: {EMBER}; animation: blink 1.1s steps(1) infinite; }}
@keyframes blink {{ 50% {{ opacity: 0; }} }}
"""
    body = f"""
<defs>
  <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="{VOID}"/><stop offset="1" stop-color="{DOCK}"/>
  </linearGradient>
  <radialGradient id="ember" cx="0.5" cy="0.5" r="0.5">
    <stop offset="0" stop-color="{COPPER}" stop-opacity=".45"/>
    <stop offset="1" stop-color="{COPPER}" stop-opacity="0"/>
  </radialGradient>
  <clipPath id="frame"><rect width="{width}" height="{height}" rx="20"/></clipPath>
</defs>
<g clip-path="url(#frame)">
  <rect width="{width}" height="{height}" fill="url(#bg)"/>
  {grid(width, height)}
  <ellipse class="glow" cx="900" cy="200" rx="420" ry="260" fill="url(#ember)"/>

  <text x="64" y="104" class="mono dim rise" style="font-size:14px;letter-spacing:.22em">DAEMON-LESS · LOCAL-FIRST · AIR-GAPPED</text>
  <text x="60" y="196" class="serif rise d1" style="font-size:92px;font-weight:700" fill="{PAPER}">Containust</text>
  <text x="64" y="246" class="sans rise d2" style="font-size:26px;font-weight:600" fill="{EMBER}">Containers without the daemon.</text>
  <text x="64" y="290" class="sans rise d2" style="font-size:18px" fill="{INK}">Declare a stack in <tspan class="mono" style="font-size:17px" fill="{EMBER}">.ctst</tspan>, plan the diff, apply it —</text>
  <text x="64" y="316" class="sans rise d2" style="font-size:18px" fill="{INK}">no root daemon, no registry required, no network if you say so.</text>

  <g class="rise d3">
    <rect x="64" y="350" width="372" height="48" rx="10" fill="{VOID}" stroke="{LINE}"/>
    <text x="84" y="380" class="mono" style="font-size:16px" fill="{STEEL}">$ <tspan fill="{PAPER}">ctst run</tspan> <tspan fill="{EMBER}">stack.ctst</tspan></text>
    <rect x="304" y="366" width="9" height="18" class="caret"/>
  </g>

  <path class="wire" d="M854 134 C 920 134, 914 222, 976 222"/>
  <path class="wire" d="M854 324 C 920 324, 914 252, 976 252"/>
  <path class="pulse" d="M854 134 C 920 134, 914 222, 976 222"/>
  <path class="pulse p2" d="M854 324 C 920 324, 914 252, 976 252"/>

  {node(610, 90, "web", ['image = "preset://alpine"', "port  = 8080"], "n1")}
  {node(610, 280, "worker", ['image = "tar:///img/w.tar"', 'memory = "256MiB"'], "n2")}
  {node(976, 192, "db", ['image = "oci://postgres@…"', "port  = 5432"], "n3")}

  <g class="ghost">
    <rect x="998" y="330" width="200" height="50" rx="10" fill="none" stroke="{RED}" stroke-dasharray="5 5"/>
    <text x="1098" y="360" text-anchor="middle" class="mono" style="font-size:15px" fill="{RED}">dockerd</text>
    <path d="M1056 355 H1140" stroke="{RED}" stroke-width="2"/>
    <text x="1098" y="404" text-anchor="middle" class="mono dim" style="font-size:12px">not required</text>
  </g>
</g>
"""
    return svg(width, height, "Containust — containers without the daemon", body, style)


# --------------------------------------------------------------------------
# Terminal demo
# --------------------------------------------------------------------------

LOOP = 18.0  # seconds per cycle


def terminal() -> str:
    width = 920
    # (kind, at_seconds, text segments [(text, color)])
    script: list[tuple[str, float, list[tuple[str, str]]]] = [
        ("cmd", 0.6, [("ctst plan ", PAPER), ("stack.ctst", EMBER)]),
        ("out", 2.0, [("Plan for stack.ctst", PAPER)]),
        ("out", 2.2, [("  + create   ", GREEN), ("db", PAPER)]),
        ("out", 2.4, [("  + create   ", GREEN), ("api", PAPER)]),
        ("out", 2.6, [("  + create   ", GREEN), ("web", PAPER)]),
        ("out", 2.9, [("  Plan: 3 to create, 0 to replace, 0 unchanged.", STEEL)]),
        ("cmd", 4.2, [("ctst run -d ", PAPER), ("stack.ctst", EMBER)]),
        ("out", 5.6, [("  ● db   ", GREEN), ("[a41f…]", STEEL), ("  -> localhost:5432", CYAN)]),
        ("out", 5.9, [("  ● api  ", GREEN), ("[7c02…]", STEEL), ("  -> localhost:8080", CYAN)]),
        ("out", 6.2, [("  ● web  ", GREEN), ("[e9b3…]", STEEL), ("  -> localhost:3000", CYAN)]),
        ("cmd", 8.0, [("sed -i 's/pg15/pg16/' stack.ctst && ctst plan", PAPER)]),
        ("out", 9.6, [("  ~ replace  ", YELLOW), ("db ", PAPER), ("(spec changed)", STEEL)]),
        ("out", 9.8, [("  ~ replace  ", YELLOW), ("api ", PAPER), ("(upstream 'db' replaced)", STEEL)]),
        ("out", 10.0, [("  = keep     ", STEEL), ("web", PAPER)]),
        ("cmd", 11.6, [("ctst run -d ", PAPER), ("stack.ctst", EMBER), (" && ctst plan", PAPER)]),
        ("out", 13.2, [("  Replacing container 'db' (spec changed)...", STEEL)]),
        ("out", 13.5, [("  Replacing container 'api' (upstream 'db' replaced)...", STEEL)]),
        ("out", 14.0, [("  No changes. ", GREEN), ("Running stack matches stack.ctst.", INK)]),
    ]
    top, line_h = 64, 24
    height = top + line_h * len(script) + 30

    keyframes = []
    rows = []
    for i, (kind, at, segments) in enumerate(script):
        start = at / LOOP * 100
        y = top + i * line_h
        spans = "".join(f'<tspan fill="{c}">{esc(t)}</tspan>' for t, c in segments)
        if kind == "cmd":
            chars = sum(len(t) for t, _ in segments)
            typed = min(start + chars * 0.05 / LOOP * 100, 94)
            keyframes.append(
                f"@keyframes r{i} {{ 0%,{start:.2f}% {{ opacity:0; }}"
                f" {start + 0.01:.2f}%,96% {{ opacity:1; }} 100% {{ opacity:0; }} }}"
            )
            keyframes.append(
                f"@keyframes c{i} {{ 0%,{start:.2f}% {{ transform: translateX(0); }}"
                f" {typed:.2f}%,100% {{ transform: translateX({chars * 9.2 + 12:.0f}px); }} }}"
            )
            rows.append(
                f'<g style="animation: r{i} {LOOP}s linear infinite">'
                f'<text x="28" y="{y}" class="mono"><tspan fill="{COPPER}">$ </tspan>{spans}</text>'
                f'<rect x="46" y="{y - 17}" width="{width - 60}" height="23" fill="{VOID}" '
                f'style="transform: translateX({chars * 9.2 + 12:.0f}px); '
                f'animation: c{i} {LOOP}s steps({chars}, end) infinite"/></g>'
            )
        else:
            keyframes.append(
                f"@keyframes r{i} {{ 0%,{start:.2f}% {{ opacity:0; transform: translateY(4px); }}"
                f" {start + 1.2:.2f}%,96% {{ opacity:1; transform: none; }} 100% {{ opacity:0; }} }}"
            )
            rows.append(
                f'<text x="28" y="{y}" class="mono" style="animation: r{i} {LOOP}s ease-out infinite">{spans}</text>'
            )

    style = f"""
.mono {{ font-family: {MONO}; font-size: 15px; white-space: pre; }}
.bar {{ font-family: {SANS}; font-size: 13px; fill: {STEEL}; }}
{chr(10).join(keyframes)}
"""
    body = f"""
<rect width="{width}" height="{height}" rx="16" fill="{VOID}"/>
<rect x=".5" y=".5" width="{width - 1}" height="{height - 1}" rx="16" fill="none" stroke="{LINE}"/>
<rect width="{width}" height="38" rx="16" fill="{DOCK}"/>
<rect y="22" width="{width}" height="16" fill="{DOCK}"/>
<circle cx="24" cy="19" r="6" fill="#ef6f61"/><circle cx="44" cy="19" r="6" fill="#f2c14e"/><circle cx="64" cy="19" r="6" fill="#5fc27e"/>
<text x="{width // 2}" y="24" text-anchor="middle" class="bar">ctst — declarative plan &amp; apply</text>
<clipPath id="inner"><rect x="2" y="38" width="{width - 4}" height="{height - 40}"/></clipPath>
<g clip-path="url(#inner)">
{chr(10).join(rows)}
</g>
"""
    return svg(width, height, "ctst plan and ctst run: declarative apply demo", body, style)


# --------------------------------------------------------------------------
# Architecture
# --------------------------------------------------------------------------


def arch() -> str:
    width, height = 1280, 404
    stages = [
        (40, ".ctst file", ["COMPONENT · CONNECT", "EXPOSE · IMPORT"]),
        (278, "Parse & validate", ["nom grammar", "offline policy gate"]),
        (516, "Dependency graph", ["petgraph topo-sort", "CONNECT env wiring"]),
        (754, "Plan diff", ["+ create   ~ replace", "> start    = keep", "- remove (--prune)"]),
    ]
    boxes = []
    for i, (x, title, lines) in enumerate(stages):
        sub = "".join(
            f'<text x="{x + 20}" y="{176 + 22 * j}" class="mono dim">{esc(t)}</text>'
            for j, t in enumerate(lines)
        )
        boxes.append(
            f"""<g class="stage s{i}">
  <rect x="{x}" y="110" width="210" height="134" rx="14" fill="{DOCK}" stroke="{LINE}"/>
  <rect x="{x}" y="110" width="210" height="134" rx="14" fill="none" class="lit"/>
  <text x="{x + 20}" y="146" class="sans h">{esc(title)}</text>
  {sub}
</g>"""
        )
    connectors = "".join(
        f'<path class="wire" d="M{x + 210} 176 H{x + 238}"/><path class="flow f{i}" d="M{x + 210} 176 H{x + 238}"/>'
        for i, (x, _, _) in enumerate(stages[:-1])
    )
    style = f"""
.sans {{ font-family: {SANS}; }}
.mono {{ font-family: {MONO}; font-size: 13px; }}
.h {{ fill: {PAPER}; font-size: 17px; font-weight: 700; }}
.dim {{ fill: {STEEL}; }}
.cap {{ font-family: {MONO}; font-size: 12px; fill: {STEEL}; letter-spacing: .18em; }}
.wire {{ stroke: {STEEL}; stroke-opacity: .35; stroke-width: 2; fill: none; }}
.flow {{ stroke: {EMBER}; stroke-width: 3; fill: none; stroke-linecap: round;
  stroke-dasharray: 10 300; animation: flow 4s linear infinite; }}
@keyframes flow {{ 0% {{ stroke-dashoffset: 310; }} 100% {{ stroke-dashoffset: 0; }} }}
.f1 {{ animation-delay: .6s; }} .f2 {{ animation-delay: 1.2s; }} .f3 {{ animation-delay: 1.8s; }} .f4 {{ animation-delay: 2.4s; }} .f5 {{ animation-delay: 2.4s; }}
.lit {{ stroke: {COPPER}; stroke-width: 2; opacity: 0; animation: lit 4s ease-in-out infinite; }}
@keyframes lit {{ 0%,100% {{ opacity: 0; }} 10% {{ opacity: 1; }} 30% {{ opacity: 0; }} }}
.s1 .lit {{ animation-delay: .6s; }} .s2 .lit {{ animation-delay: 1.2s; }} .s3 .lit {{ animation-delay: 1.8s; }}
.s4 .lit {{ animation-delay: 2.6s; }} .s5 .lit {{ animation-delay: 2.6s; }}
.stage {{ animation: in .9s cubic-bezier(.22,1,.36,1) both; }}
.s1 {{ animation-delay: .1s; }} .s2 {{ animation-delay: .2s; }} .s3 {{ animation-delay: .3s; }} .s4 {{ animation-delay: .4s; }} .s5 {{ animation-delay: .5s; }}
@keyframes in {{ from {{ opacity: 0; transform: translateY(10px); }} to {{ opacity: 1; transform: none; }} }}
"""
    body = f"""
<rect width="{width}" height="{height}" rx="20" fill="{VOID}"/>
{grid(width, height)}
<text x="40" y="66" class="cap">HOW A ctst RUN FLOWS — NO DAEMON, JUST A CLI AND FILES</text>
{"".join(boxes)}
{connectors}
<path class="wire" d="M964 176 C 990 176, 984 106, 1010 106"/>
<path class="flow f4" d="M964 176 C 990 176, 984 106, 1010 106"/>
<path class="wire" d="M964 176 C 990 176, 984 246, 1010 246"/>
<path class="flow f5" d="M964 176 C 990 176, 984 246, 1010 246"/>
<g class="stage s4">
  <rect x="1010" y="62" width="230" height="88" rx="14" fill="{DOCK}" stroke="{LINE}"/>
  <rect x="1010" y="62" width="230" height="88" rx="14" fill="none" class="lit"/>
  <text x="1030" y="96" class="sans h">Linux native</text>
  <text x="1030" y="122" class="mono dim">user+PID ns · cgroups v2</text>
</g>
<g class="stage s5">
  <rect x="1010" y="202" width="230" height="88" rx="14" fill="{DOCK}" stroke="{LINE}"/>
  <rect x="1010" y="202" width="230" height="88" rx="14" fill="none" class="lit"/>
  <text x="1030" y="236" class="sans h">macOS · Windows</text>
  <text x="1030" y="262" class="mono dim">QEMU micro-VM + agent</text>
</g>
<rect x="40" y="314" width="1200" height="60" rx="14" fill="none" stroke="{LINE}" stroke-dasharray="6 6"/>
<text x="64" y="350" class="mono" style="font-size:14px" fill="{INK}"><tspan fill="{EMBER}">.containust/</tspan>  state/state.json · state/applied.json · images/ · layers/sha256 · logs/   <tspan fill="{STEEL}">← the whole runtime state, per project</tspan></text>
"""
    return svg(width, height, "Containust architecture: .ctst to plan to Linux or QEMU backend", body, style)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, render in (("hero.svg", hero), ("terminal.svg", terminal), ("architecture.svg", arch)):
        (OUT / name).write_text(render(), encoding="utf-8")
        print(f"wrote {OUT / name}")


if __name__ == "__main__":
    main()
