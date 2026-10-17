import { readFileSync } from 'node:fs';
import { beforeEach, describe as group, expect, it, vi } from 'vitest';
import * as app from '../daemon/web/app.js';

// Run from the repository root (see vitest.config.js).
const html = readFileSync('daemon/web/index.html', 'utf8');

function defaults() {
  return {
    version: 1, deadzone: 0.04, camera_strength: 1, throttle_lever: 'left',
    invert: { roll: false, pitch: false, yaw: false, throttle: false },
    split_high_g: { enabled: true, forward: 0.95, back: 0.05 },
    buttons: { s1: [1], s5: [5], s6: [7], q2: [7] }, pov: { s11: 'up' },
  };
}

function fakeApi(overrides = {}) {
  const calls = [];
  return {
    calls,
    get: vi.fn(async (path) => {
      calls.push(['GET', path]);
      if (path === '/api/config') return { config: defaults(), defaults: defaults(), status: 'Idle' };
      if (path === '/api/labels') return { labels: [{ label: 'Old', sources: ['s3'], mapping: 'Nothing → NEO 3 (Weapon select)' }] };
      return { pressed: [], status: 'Idle' };
    }),
    put: vi.fn(async (path, value) => { calls.push(['PUT', path, structuredClone(value)]); return path === '/api/labels' ? { saved: value.length } : { saved: true, running: false }; }),
    ...overrides,
  };
}

async function page(api = fakeApi()) {
  const instance = new app.Page(document, api);
  await instance.load();
  return instance;
}

const $ = (id) => document.getElementById(id);
const tca = (key) => document.querySelector(`[data-source="${key}"]`);
const neo = (id) => document.querySelector(`[data-neo="${id}"]`);
const pressed = (element) => element.getAttribute('aria-pressed');
const fire = (element) => element.dispatchEvent(new Event('input', { bubbles: true }));

beforeEach(() => {
  document.documentElement.innerHTML = html.replace(/^<!doctype html>/i, '');
});

group('helpers', () => {
  it('names every TCA source', () => {
    expect(app.sources()).toHaveLength(48);
    expect(['s5', 's17', 'q3', 'q12', 'q20', 'hat-down-left'].map(app.label)).toEqual([
      'Sidestick button 5', 'Sidestick input 17', 'Quadrant button 3', 'Quadrant detent state 12', 'Quadrant add-on input 20', 'Sidestick hat down-left',
    ]);
  });

  it('names NEO controls and describes mappings', () => {
    expect(app.neoName('b5')).toBe('NEO 5 (High-G)');
    expect(app.neoName('b13')).toBe('NEO 13');
    expect(app.neoName('pov-left')).toBe('NEO POV left');
    expect(app.describe({ buttons: [], pov: null })).toBe('Nothing');
    expect(app.describe({ buttons: [1, 13], pov: 'up' })).toBe('NEO 1 (Gun), NEO 13, NEO POV up');
  });

  it('reads current mappings, compares them and finds sources by NEO control', () => {
    const config = defaults();
    config.buttons.s9 = [4, 2];
    expect(app.current(config, 's9')).toEqual({ buttons: [2, 4], pov: null });
    expect(app.current(config, 's11')).toEqual({ buttons: [], pov: 'up' });
    expect(app.same({ buttons: [1], pov: null }, { buttons: [1], pov: null })).toBe(true);
    expect(app.same({ buttons: [1], pov: null }, { buttons: [1], pov: 'up' })).toBe(false);
    expect(app.same({ buttons: [1], pov: null }, { buttons: [2], pov: null })).toBe(false);
    expect(app.sourcesFor(config, 'b7')).toEqual(['s6', 'q2']);
    expect(app.sourcesFor(config, 'pov-up')).toEqual(['s11']);
  });

  it('client sends the token and surfaces server errors', async () => {
    const fetchImpl = vi.fn(async (_path, options) => ({
      ok: options.method === 'GET',
      json: async () => (options.method === 'GET' ? { fine: true } : { error: 'Deadzone must be below 1.' }),
    }));
    const api = app.client(fetchImpl, 'tok');
    expect(await api.get('/api/config')).toEqual({ fine: true });
    expect(fetchImpl.mock.calls[0][1].headers['X-Neo-Token']).toBe('tok');
    await expect(api.put('/api/config', { a: 1 })).rejects.toThrow('Deadzone must be below 1.');
    expect(fetchImpl.mock.calls[1][1].body).toBe('{"a":1}');
  });
});

group('mapping tab', () => {
  it('builds diagrams, chips, NEO hotspots and the legend', async () => {
    await page();
    expect(document.querySelectorAll('[data-source]')).toHaveLength(52);
    expect(tca('q12').getAttribute('aria-label')).toBe('Quadrant detent state 12');
    expect(document.querySelectorAll('[data-neo]')).toHaveLength(20);
    expect(neo('b5').getAttribute('aria-label')).toBe('NEO 5 (High-G)');
    expect(neo('b16').disabled).toBe(true);
    expect(neo('pov-up').querySelector('svg')).not.toBeNull();
    expect(neo('b5').style.getPropertyValue('--x')).toBe('44.4%');
    expect($('neo-legend').textContent).toContain('5 High-G · 6 Autopilot');
    expect($('selected-name').textContent).toBe('Nothing selected');
    expect($('deselect').disabled).toBe(true);
    expect($('recorded-count').textContent).toBe('1');
    $('handedness').value = 'left';
    $('handedness').dispatchEvent(new Event('change'));
    expect($('sidestick-frame').dataset.hand).toBe('left');
  });

  it('selecting a TCA control lights its NEO mapping; NEO clicks build a new one', async () => {
    const p = await page();
    tca('s6').click();
    expect(pressed(tca('s6'))).toBe('true');
    expect($('selected-name').textContent).toBe('Sidestick button 6');
    expect($('mapped-now').textContent).toBe('NEO 7 (Flares)');
    expect($('mapped-new').textContent).toBe('Unchanged');
    expect(neo('b7').classList.contains('current')).toBe(true);
    expect(pressed(neo('b7'))).toBe('true');
    expect($('record').disabled).toBe(true);
    neo('b6').click();
    expect(neo('b6').classList.contains('added')).toBe(true);
    neo('b7').click();
    expect(pressed(neo('b7'))).toBe('false');
    neo('pov-down').click();
    expect($('mapped-new').textContent).toBe('NEO 6 (Autopilot), NEO POV down');
    neo('pov-down').click();
    expect(p.proposal.pov).toBeNull();
    neo('pov-left').click();
    expect($('record').disabled).toBe(false);
    $('revert').click();
    expect($('mapped-new').textContent).toBe('Unchanged');
    $('deselect').click();
    expect($('selected-name').textContent).toBe('Nothing selected');
  });

  it('with nothing selected, a NEO click shows which TCA controls feed it', async () => {
    await page();
    neo('b7').click();
    expect($('selected-name').textContent).toBe('NEO 7 (Flares)');
    expect($('mapped-now').textContent).toBe('From Sidestick button 6, Quadrant button 2');
    expect(tca('s6').classList.contains('linked')).toBe(true);
    expect(pressed(neo('b7'))).toBe('true');
    expect($('deselect').disabled).toBe(false);
    neo('b14').click();
    expect($('mapped-now').textContent).toBe('No TCA control');
    neo('b14').click();
    expect($('selected-name').textContent).toBe('Nothing selected');
  });

  it('records corrections to the profile and the log', async () => {
    const api = fakeApi();
    const p = await page(api);
    tca('s6').click();
    neo('b7').click();
    neo('b6').click();
    $('record').click();
    await vi.waitFor(() => expect($('link-status').textContent).toBe('Recorded Sidestick button 6: NEO 6 (Autopilot).'));
    const configPut = api.calls.find((c) => c[0] === 'PUT' && c[1] === '/api/config');
    expect(configPut[2].buttons.s6).toEqual([6]);
    expect(api.calls.at(-1)[2].at(-1)).toEqual({ label: 'Sidestick button 6', sources: ['s6'], mapping: 'NEO 7 (Flares) → NEO 6 (Autopilot)' });
    expect(p.config.buttons.s6).toEqual([6]);
    expect($('mapped-now').textContent).toBe('NEO 6 (Autopilot)');
    expect($('recorded-count').textContent).toBe('2');
    // Clearing a POV removes it; choosing one stores it.
    tca('s11').click();
    neo('pov-up').click();
    api.put = vi.fn(async (path) => (path === '/api/labels' ? { saved: 1 } : { saved: true, running: true }));
    await p.record();
    expect('s11' in p.saved.pov).toBe(false);
    expect($('link-status').textContent).toBe('Recorded Sidestick button 11: Nothing. Applied to the running emulation.');
    tca('s12').click();
    neo('pov-right').click();
    await p.record();
    expect(p.saved.pov.s12).toBe('right');
  });

  it('keeps the proposal when saving fails and reports a failed log update', async () => {
    const api = fakeApi({ put: vi.fn(async () => { throw new Error('The profile could not be saved.'); }) });
    const p = await page(api);
    tca('s5').click();
    neo('b9').click();
    await p.record();
    expect($('link-status').textContent).toBe('Not saved: The profile could not be saved.');
    expect(p.proposal.buttons).toEqual([5, 9]);
    expect($('record').disabled).toBe(false);
    api.put = vi.fn(async (path) => { if (path === '/api/labels') throw new Error('The recordings could not be saved.'); return { saved: true, running: false }; });
    await p.record();
    expect($('link-status').textContent).toBe('Mapping saved, but the correction log was not updated: The recordings could not be saved..');
    expect(p.saved.buttons.s5).toEqual([5, 9]);
  });

  it('removes log entries and reports failures', async () => {
    const api = fakeApi();
    const p = await page(api);
    $('recorded').querySelector('button').click();
    await vi.waitFor(() => expect($('link-status').textContent).toBe('Removed from the correction log. The mapping itself is unchanged.'));
    expect(p.labels).toEqual([]);
    p.labels = [{ label: 'X', sources: ['s1'], mapping: 'a' }];
    p.renderLabels();
    api.put = vi.fn(async () => { throw new Error('offline'); });
    await p.forget(0);
    expect($('link-status').textContent).toBe('Not removed: offline');
  });

  it('explains when earlier corrections cannot be loaded', async () => {
    const api = fakeApi();
    const original = api.get;
    api.get = vi.fn(async (path) => { if (path === '/api/labels') throw new Error('The recordings could not be read.'); return original(path); });
    await page(api);
    expect($('link-status').textContent).toBe('Earlier corrections could not be loaded: The recordings could not be read.');
  });
});

group('sidestick hat', () => {
  it('is disabled while it drives the camera and mappable in Mapped mode', async () => {
    const api = fakeApi();
    const p = await page(api);
    expect(tca('hat-up').textContent).toBe('Up');
    expect(tca('hat-up').disabled).toBe(true);
    expect(tca('hat-up').title).toBe('Set the hat to Mapped controls to map it');
    expect($('hat-mode').value).toBe('camera');
    p.live({ pressed: ['hat-up'], status: 'Emulating' });
    expect(p.selected).toBeNull();
    $('hat-mode').value = 'mapped';
    $('hat-mode').dispatchEvent(new Event('change'));
    await vi.waitFor(() => expect($('link-status').textContent).toBe('The hat now sends its mapped controls; the camera mini-stick gets no input.'));
    expect(api.calls.at(-1)[2].hat).toBe('mapped');
    expect(tca('hat-up').disabled).toBe(false);
    expect(tca('hat-up').title).toBe('Sidestick hat up');
    p.live({ pressed: [], status: 'Emulating' });
    p.live({ pressed: ['hat-up'], status: 'Emulating' });
    expect(p.selected).toBe('hat-up');
    neo('pov-up').click();
    await p.record();
    expect(p.saved.pov['hat-up']).toBe('up');
    neo('pov-up').click();
    expect($('mapped-now').textContent).toBe('NEO POV up');
    // Reverse lookup includes hat sources.
    $('deselect').click();
    neo('pov-up').click();
    expect($('mapped-now').textContent).toBe('From Sidestick button 11, Sidestick hat up');
    // Back to camera with a hat direction selected: the selection is cleared.
    tca('hat-up').click();
    api.put = vi.fn(async () => ({ saved: true, running: true }));
    await p.setHat('camera');
    expect(p.selected).toBeNull();
    expect($('link-status').textContent).toBe('The hat drives the camera again; hat mappings are kept but inactive. Applied to the running emulation.');
    expect(p.saved.pov['hat-up']).toBe('up');
    // Switching with a button selected keeps it; with nothing selected is fine too.
    await p.setHat('mapped');
    tca('s2').click();
    await p.setHat('camera');
    expect(p.selected).toBe('s2');
    $('deselect').click();
    await p.setHat('mapped');
    expect(p.selected).toBeNull();
  });

  it('reports a failed mode change and keeps the previous mode', async () => {
    const api = fakeApi({ put: vi.fn(async () => { throw new Error('The profile could not be saved.'); }) });
    const p = await page(api);
    await p.setHat('mapped');
    expect($('link-status').textContent).toBe('Not saved: The profile could not be saved.');
    expect($('hat-mode').value).toBe('camera');
    expect(p.busy).toBe(false);
  });
});

group('live input', () => {
  it('lights held controls and the hat, and selects new presses on the mapping tab', async () => {
    const p = await page();
    p.live({ pressed: [], status: 'Emulating' });
    expect($('live-input').textContent).toBe('Now sending: nothing');
    expect($('daemon-status').textContent).toBe('Daemon: Emulating');
    p.live({ pressed: ['s5', 'hat-up-right'], status: 'Emulating' });
    expect($('live-input').textContent).toBe('Now sending: Sidestick button 5, Sidestick hat up-right');
    expect(tca('s5').classList.contains('held')).toBe(true);
    expect(document.querySelector('.hat[data-hat=up]').classList.contains('held')).toBe(true);
    expect(document.querySelector('.hat[data-hat=right]').classList.contains('held')).toBe(true);
    expect(document.querySelector('.hat[data-hat=down]').classList.contains('held')).toBe(false);
    expect(p.selected).toBe('s5');
    p.live({ pressed: ['s5', 'hat-up-right'], status: 'Emulating' }); // held: no reselection
    tca('q1').click();
    p.live({ pressed: ['s5'], status: 'Emulating' });
    expect(p.selected).toBe('q1');
    p.busy = true;
    p.live({ pressed: ['s5', 's2'], status: 'Emulating' });
    expect(p.selected).toBe('q1');
    p.busy = false;
    $('tab-settings').click();
    p.live({ pressed: ['s3'], status: 'Emulating' });
    expect(p.selected).toBe('q1');
  });

  it('poll reports an unreachable daemon', async () => {
    const api = fakeApi();
    const p = await page(api);
    await p.poll();
    expect(api.calls.at(-1)).toEqual(['GET', '/api/live']);
    api.get = vi.fn(async () => { throw new Error('offline'); });
    await p.poll();
    expect($('daemon-status').textContent).toBe('Daemon: unreachable. Reopen Configure from the tray icon.');
  });
});

group('split-lever High-G', () => {
  it('describes live lever positions against the saved thresholds', () => {
    const rule = { enabled: true, forward: 0.95, back: 0.05 };
    expect(app.leverText([0.97, 0.02], rule)).toBe('Left lever 97% · Right lever 2% · High-G active');
    expect(app.leverText([0.02, 0.97], rule)).toBe('Left lever 2% · Right lever 97% · High-G active');
    expect(app.leverText([0.5, 0.97], rule)).toBe('Left lever 50% · Right lever 97%');
    expect(app.leverText([0.97, 0.5], rule)).toBe('Left lever 97% · Right lever 50%');
    expect(app.leverText([0.97, 0.02], { ...rule, enabled: false })).toBe('Left lever 97% · Right lever 2%');
  });

  it('shows the live readout during emulation', async () => {
    const p = await page();
    expect($('split-forward').value).toBe('95');
    expect($('split-back').value).toBe('5');
    expect($('split-enabled').checked).toBe(true);
    p.live({ pressed: [], levers: [1, 0], status: 'Emulating' });
    expect($('lever-live').textContent).toBe('Left lever 100% · Right lever 0% · High-G active');
    p.live({ pressed: [], levers: null, status: 'Idle' });
    expect($('lever-live').textContent).toBe('Lever positions appear here during emulation.');
  });

  it('validates and saves the thresholds and the switch', async () => {
    const api = fakeApi();
    const p = await page(api);
    $('split-enabled').checked = false;
    fire($('split-enabled'));
    $('split-forward').value = '';
    await p.save();
    expect($('split-forward-error').textContent).toBe('Enter a number from 0 to 100.');
    $('split-forward').value = '80';
    $('split-back').value = '80';
    await p.save();
    expect($('split-back-error').textContent).toBe('Must be below the forward lever threshold.');
    expect($('split-back').getAttribute('aria-invalid')).toBe('true');
    $('split-back').value = '';
    await p.save();
    expect(api.put).not.toHaveBeenCalled();
    $('split-back').value = '10';
    await p.save();
    expect(api.calls.at(-1)[2].split_high_g).toEqual({ enabled: false, forward: 0.8, back: 0.1 });
    expect(p.saved.split_high_g.forward).toBe(0.8);
  });
});

group('tabs and settings', () => {
  it('switches tabs by click and arrow keys', async () => {
    await page();
    $('tab-settings').click();
    expect($('panel-settings').hidden).toBe(false);
    expect($('panel-mapping').hidden).toBe(true);
    expect($('tab-settings').tabIndex).toBe(0);
    $('tab-settings').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft' }));
    expect($('panel-mapping').hidden).toBe(false);
    expect(document.activeElement).toBe($('tab-mapping'));
    $('tab-mapping').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    expect(document.activeElement).toBe($('tab-settings'));
    $('tab-settings').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    expect($('panel-settings').hidden).toBe(false);
  });

  it('edits, validates, saves and resets settings', async () => {
    const api = fakeApi();
    const p = await page(api);
    $('lever').value = 'right';
    fire($('lever'));
    $('invert-yaw').checked = true;
    fire($('invert-yaw'));
    fire($('deadzone'));
    expect($('dirty').textContent).toBe('Unsaved changes.');
    $('deadzone').value = '1';
    $('camera').value = '';
    await p.save();
    expect($('deadzone-error').textContent).toBe('Enter a number from 0 to below 1.');
    expect($('camera-error').textContent).toBe('Enter a number from 0 to 1.');
    expect($('dirty').textContent).toBe('Fix the highlighted fields, then save.');
    $('deadzone').value = '-1';
    $('camera').value = '0.5';
    await p.save();
    $('deadzone').value = '0.1';
    $('camera').value = '2';
    await p.save();
    expect(api.put).not.toHaveBeenCalled();
    $('camera').value = '1';
    await p.save();
    expect(api.calls.at(-1)[2]).toMatchObject({ deadzone: 0.1, camera_strength: 1, throttle_lever: 'right', invert: { yaw: true } });
    expect($('dirty').textContent).toBe('Saved.');
    expect(p.saved.throttle_lever).toBe('right');
    api.put = vi.fn(async () => ({ saved: true, running: true }));
    $('profile').dispatchEvent(new Event('submit', { cancelable: true }));
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Saved. Applied to the running emulation.'));
    api.put = vi.fn(async () => { throw new Error('Deadzone must be at least 0 and below 1.'); });
    await p.save();
    expect($('dirty').textContent).toBe('Not saved: Deadzone must be at least 0 and below 1.');
    expect($('save').disabled).toBe(false);
    $('reset').click();
    expect($('lever').value).toBe('left');
    expect($('dirty').textContent).toBe('Unsaved: defaults, including every mapping. Save settings to apply.');
  });
});

group('start', () => {
  function windowLike(fetchImpl) {
    return { document, fetch: fetchImpl, location: { hash: '#tok' }, setInterval: vi.fn((fn) => { fn(); return 7; }) };
  }

  it('loads with the fragment token and starts polling', async () => {
    const fetchImpl = vi.fn(async (path) => ({ ok: true, json: async () => {
      if (path === '/api/config') return { config: defaults(), defaults: defaults(), status: 'Idle' };
      if (path === '/api/labels') return { labels: [] };
      return { pressed: [], status: 'Idle' };
    } }));
    const win = windowLike(fetchImpl);
    const p = await app.start(win, 5);
    expect(p.timer).toBe(7);
    expect(win.setInterval.mock.calls[0][1]).toBe(5);
    expect(fetchImpl.mock.calls[0][1].headers['X-Neo-Token']).toBe('tok');
  });

  it('reports a failed first load without polling', async () => {
    const fetchImpl = vi.fn(async () => ({ ok: false, json: async () => ({ error: 'This page is out of date.' }) }));
    const win = windowLike(fetchImpl);
    const p = await app.start(win);
    expect(win.setInterval).not.toHaveBeenCalled();
    expect(p.timer).toBeUndefined();
    expect($('daemon-status').textContent).toBe('Daemon: unavailable (This page is out of date.)');
  });

  it('starts automatically when loaded into the configuration page', async () => {
    vi.resetModules();
    const fetchImpl = vi.fn(async () => ({ ok: false, json: async () => ({ error: 'stopped' }) }));
    vi.stubGlobal('fetch', fetchImpl);
    window.location.hash = '#auto';
    await import('../daemon/web/app.js');
    await vi.waitFor(() => expect($('daemon-status').textContent).toBe('Daemon: unavailable (stopped)'));
    expect(fetchImpl.mock.calls[0][1].headers['X-Neo-Token']).toBe('auto');
    vi.unstubAllGlobals();
  });
});
