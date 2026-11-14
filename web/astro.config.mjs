import { defineConfig } from 'astro/config';

// Hosting targets:
//   Vercel (VERCEL=1 is set automatically): served from the domain root.
//   GitHub Pages (default): https://mxskeen.github.io/fudge/
//   Anything else: SITE=https://example.com BASE=/ npm run build
const onVercel = !!process.env.VERCEL;
const vercelHost = process.env.VERCEL_PROJECT_PRODUCTION_URL;

const site =
  process.env.SITE ||
  (onVercel && vercelHost ? `https://${vercelHost}` : 'https://mxskeen.github.io');
const base = process.env.BASE ?? (onVercel ? '/' : '/fudge');

export default defineConfig({
  site,
  base,
  output: 'static',
  trailingSlash: 'always',
  build: { format: 'directory', inlineStylesheets: 'auto' },
});
