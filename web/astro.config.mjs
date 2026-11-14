import { defineConfig } from 'astro/config';

// For GitHub Pages project sites set e.g.
//   SITE=https://fudge-launcher.github.io BASE=/fudge
const site = process.env.SITE || 'http://localhost:4321';
const base = process.env.BASE || '/';

export default defineConfig({
  site,
  base,
  output: 'static',
  trailingSlash: 'always',
  build: { format: 'directory', inlineStylesheets: 'auto' },
});
