# Containust landing page

Static marketing page for the project.

## Preview

```bash
# from the repo root
open site/index.html
# or
python3 -m http.server 4173 --directory site
```

Then open `http://localhost:4173`.

Relative links point at `../docs/` and `../README.md` when served from the
repository (or GitHub Pages with the whole repo as the site root).

## Deploy (optional)

GitHub Pages → source: `/site` on `main`, or serve this folder from any static host.
