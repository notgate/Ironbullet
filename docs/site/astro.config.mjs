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
        { label: 'Get started', items: [
          { label: 'Overview', slug: 'index' },
          { label: 'Interface tour', slug: 'interface-tour' },
          { label: 'Install and build', slug: 'getting-started' },
          { label: 'Pipeline model', slug: 'pipeline-model' },
        ] },
        { label: 'Block reference', items: [
          { label: 'Reference overview', slug: 'blocks' },
          { label: 'Requests', slug: 'blocks/requests' },
          { label: 'Parsing', slug: 'blocks/parsing' },
          { label: 'Checks', slug: 'blocks/checks' },
          { label: 'Functions', slug: 'blocks/functions' },
          { label: 'Control flow', slug: 'blocks/control' },
          { label: 'Utilities', slug: 'blocks/utilities' },
          { label: 'Browser automation', slug: 'blocks/browser' },
          { label: 'Challenge helpers', slug: 'blocks/bypass' },
          { label: 'Sensors', slug: 'blocks/sensors' },
          { label: 'Security', slug: 'blocks/security' },
          { label: 'File system', slug: 'blocks/filesystem' },
        ] },
        { label: 'Execution notes', items: [
          { label: 'HTTP requests', slug: 'http-requests' },
          { label: 'Parsing and checks', slug: 'parsing-and-checks' },
          { label: 'Capability status', slug: 'capability-status' },
          { label: 'Contributing', slug: 'contributing' },
        ] },
      ],
    }),
  ],
});
