# Fudge Launcher website

Static site built with Astro, TypeScript and plain CSS. No UI framework, no third-party requests at runtime.

```sh
npm install
npm run dev       # http://localhost:4321/fudge/
npm run build     # static output in dist/
npm run preview   # serve dist/
```

## Demo engine

The landing page demo calls `evaluate_json` from `crates/fudge-compute` compiled to WebAssembly (`--features wasm`). The built files are committed in `public/wasm/` so the site builds without Rust. To rebuild them:

```sh
cargo install wasm-bindgen-cli --version 0.2.130 --locked   # must match Cargo.lock
npm run wasm                                                # runs scripts/build-wasm.sh
```

Currency uses fixed sample rates; app/file/window/kill/search/shell rows are simulated in the browser.

## Deploying (GitHub Pages)

The defaults in `astro.config.mjs` target `https://mxskeen.github.io/fudge/`:

```sh
npm run build
```

Upload `dist/` with `actions/upload-pages-artifact`. For a custom domain or a root site, override both values:

```sh
SITE=https://example.com BASE=/ npm run build
```

## Branding

The logo lives in `../branding/`. The site uses copies in `public/` (favicons, touch icon, `og.png`). The in-page mark is `src/components/Logo.astro`.
