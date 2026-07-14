import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://notgate.github.io',
  base: '/Ironbullet',
  integrations: [
    starlight({
      title: 'Ironbullet',
      description: 'Reference documentation for the Ironbullet pipeline desktop application.',
      customCss: ['./src/styles/ironbullet.css'],
      sidebar: [
        { label: 'Start here', items: [
          { label: 'Overview', slug: 'index' },
          { label: 'Install and build', slug: 'getting-started' },
        ] },
        { label: 'Pipeline authoring', items: [
          { label: 'Pipeline model', slug: 'pipeline-model' },
          { label: 'HTTP requests', slug: 'http-requests' },
          { label: 'Parsing and checks', slug: 'parsing-and-checks' },
        ] },
        { label: 'Reference', items: [
          { label: 'Capability status', slug: 'capability-status' },
          { label: 'Contributing', slug: 'contributing' },
        ] },
      ],
    }),
  ],
});
