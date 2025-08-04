// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightThemeRapidePlugin from 'starlight-theme-rapide';
import starlightSidebarTopicsPlugin from 'starlight-sidebar-topics';
import starlightHeadingBadgesPlugin from 'starlight-heading-badges';

import tailwindcss from '@tailwindcss/vite';

// https://astro.build/config
export default defineConfig({
  site: 'https://fractal-tess.github.io',
  base: '/faber/',

  vite: {
    server: {
      allowedHosts: ['localhost'],
    },

    plugins: [tailwindcss()],
  },

  integrations: [
    starlight({
      title: 'Faber Docs',
      customCss: ['./src/styles/custom.css'],

      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/fractal-tess/faber',
        },
      ],
      sidebar: [
        {
          label: 'Introduction',
          link: '/',
        },
        {
          label: 'Getting Started',
          items: [
            { slug: 'guides/installation' },
            { slug: 'guides/configuration' },
            { slug: 'guides/cli-usage' },
            { slug: 'guides/api-reference' },
            { slug: 'guides/security' },
          ],
        },
        {
          label: 'Core Components',
          items: [
            { slug: 'core' },
            { slug: 'configuration' },
            { slug: 'sandbox' },
            { slug: 'executor' },
            { slug: 'api' },
            { slug: 'cli' },
          ],
        },
      ],
      plugins: [
        starlightThemeRapidePlugin(),
        starlightHeadingBadgesPlugin(),
        // starlightSidebarTopicsPlugin(),
      ],
    }),
  ],
});
