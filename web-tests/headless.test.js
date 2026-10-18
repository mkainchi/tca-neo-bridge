// @vitest-environment node
import { expect, it } from 'vitest';

it('does not start without a document', async () => {
  const app = await import('../daemon/web/app.js');
  expect(app.sources([{ id: 'tca-stick', layout: { buttons: 17, axes: [], hat: true } }])).toHaveLength(21);
});
