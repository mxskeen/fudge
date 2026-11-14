import { defineConfig } from 'astro/config';

// Defaults target the GitHub Pages project site for mxskeen/fudge.
// For a custom domain at the root: SITE=https://example.com BASE=/ npm run build
const site = process.env.SITE || 'https://mxskeen.github.io';
const base = process.env.BASE || '/fudge';

export default defineConfig({
  site,
  base,
  output: 'static',
  trailingSlash: 'always',
  build: { format: 'directory', inlineStylesheets: 'auto' },
});
