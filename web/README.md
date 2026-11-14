# Fudge Launcher website

Static site built with Astro, TypeScript and plain CSS. No UI framework, no third-party requests at runtime.

```sh
npm install
npm run dev       # http://localhost:4321
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

Set `SITE` and `BASE` at build time. For a project site at `https://fudge-launcher.github.io/fudge/`:

```sh
SITE=https://fudge-launcher.github.io BASE=/fudge npm run build
```

Upload `dist/` with `actions/upload-pages-artifact`. For a custom domain or root site leave `BASE` unset.
