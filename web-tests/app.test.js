import { readFileSync } from 'node:fs';
import { beforeEach, describe as group, expect, it, vi } from 'vitest';
import * as app from '../daemon/web/app.js';

// Run from the repository root (see vitest.config.js).
const html = readFileSync('daemon/web/index.html', 'utf8');

const axis = () => ({ deadzone: 0.04, saturation: 1, anti_deadzone: 0, curve: 0, smoothing: 0 });

function defaults() {
  return {
    version: 2, axes: { roll: axis(), pitch: axis(), yaw: axis() }, camera_strength: 1, throttle_lever: 'left',
    invert: { roll: false, pitch: false, yaw: false, throttle: false },
    split_high_g: { enabled: true, forward: 0.95, back: 0.05 },
    throttle: { mode: 'linear', idle: 0.1, cruise: 0.5, boost: 0.9, band: 0.03 },
    precision: { source: null, scale: 0.5 }, safe_start: true, combos: { pause: [] },
    behaviours: { q3: 'pulse_change' }, timing: { pulse_ms: 80 },
    buttons: { s1: [1], s5: [5], s6: [7], q2: [7], q3: [6] }, pov: { s11: 'up' },
  };
}

const LINKS = { source: 'tca', hidden: true, tca: true, virtual_neo: true };

/** Raw inputs as the daemon publishes them while emulating. */
function monitor(overrides = {}) {
  return { roll: 0.5, pitch: -0.25, yaw: 0, slider: 0, levers: [0.8, 0.3], axes: [32768, 32768, 32768, 32768, 32768, 32768, 32768], armed: true, precision: false, ...overrides };
}

function fakeApi(overrides = {}) {
  const calls = [];
  return {
    calls,
    get: vi.fn(async (path) => {
      calls.push(['GET', path]);
      if (path === '/api/config') return { config: defaults(), defaults: defaults(), status: 'Idle', links: null, profile: 'C:\\neo\\data\\config.json', preview: null };
      if (path === '/api/labels') return { labels: [{ label: 'Old', sources: ['s3'], mapping: 'Nothing → NEO 3 (Weapon select)' }] };
      return { pressed: [], status: 'Idle' };
    }),
    put: vi.fn(async (path, value) => {
      calls.push(['PUT', path, structuredClone(value)]);
      if (path === '/api/labels') return { saved: value.length };
      return path === '/api/preview' ? { preview: true, running: true } : { saved: true, running: false };
    }),
    del: vi.fn(async (path) => { calls.push(['DELETE', path]); return { preview: false }; }),
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
const radio = (name, value) => document.querySelector(`input[name=${name}][value=${value}]`);
const choose = (name, value, event = 'change') => { radio(name, value).checked = true; radio(name, value).dispatchEvent(new Event(event, { bubbles: true })); };
const row = (id) => document.querySelector(`[data-row="${id}"]`);

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
    expect(app.current(config, 's9')).toEqual({ buttons: [2, 4], pov: null, mode: 'hold' });
    expect(app.current(config, 's11')).toEqual({ buttons: [], pov: 'up', mode: 'hold' });
    expect(app.current(config, 'q3')).toEqual({ buttons: [6], pov: null, mode: 'pulse_change' });
    delete config.behaviours;
    expect(app.current(config, 'q3').mode).toBe('hold'); // profiles without behaviours
    expect(app.same({ buttons: [1], pov: null, mode: 'hold' }, { buttons: [1], pov: null, mode: 'hold' })).toBe(true);
    expect(app.same({ buttons: [1], pov: null, mode: 'hold' }, { buttons: [1], pov: 'up', mode: 'hold' })).toBe(false);
    expect(app.same({ buttons: [1], pov: null, mode: 'hold' }, { buttons: [2], pov: null, mode: 'hold' })).toBe(false);
    expect(app.same({ buttons: [1], pov: null, mode: 'hold' }, { buttons: [1], pov: null, mode: 'toggle' })).toBe(false);
    expect(app.describe({ buttons: [6], pov: null, mode: 'pulse_change' })).toBe('NEO 6 (Autopilot) · pulse on change');
    expect(app.describe({ buttons: [6], pov: null, mode: 'hold' })).toBe('NEO 6 (Autopilot)');
    expect(app.diff({ buttons: [6], pov: null, mode: 'hold' }, { buttons: [6], pov: null, mode: 'toggle' })).toEqual([['add', 'Behaviour: Toggle']]);
    expect(app.sourcesFor(config, 'b7')).toEqual(['s6', 'q2']);
    expect(app.sourcesFor(config, 'pov-up')).toEqual(['s11']);
  });

  it('shortens sources, lists changes and reports the split rule', () => {
    expect(['s6', 'q12', 'hat-up'].map(app.short)).toEqual(['S6', 'Q12', 'Hat up']);
    expect(app.diff({ buttons: [1, 2], pov: 'up' }, { buttons: [2, 3], pov: 'down' })).toEqual([
      ['add', 'Adds NEO 3 (Weapon select)'], ['remove', 'Removes NEO 1 (Gun)'], ['add', 'Adds NEO POV down'], ['remove', 'Removes NEO POV up'],
    ]);
    expect(app.diff({ buttons: [], pov: null }, { buttons: [], pov: null })).toEqual([]);
    expect(app.diff({ buttons: [], pov: 'up' }, { buttons: [], pov: null })).toEqual([['remove', 'Removes NEO POV up']]);
    expect(app.splitActive([0.8, 0.1], { enabled: true, forward: 0.75, back: 0.25 })).toBe(true);
    expect(app.splitActive([0.8, 0.3], { enabled: true, forward: 0.75, back: 0.25 })).toBe(false);
  });

  it('turns the daemon status and session links into header pills', () => {
    const full = { source: 'tca', hidden: true, tca: true, virtual_neo: true };
    expect(app.pills('Emulating', full)).toEqual({ status: 'ok', tca: ['ok', 'TCA: sidestick + quadrant open'], hidhide: ['ok', 'HidHide: TCA hidden'], neo: ['ok', 'Virtual NEO present'] });
    expect(app.pills('Emulating', { source: 'validation', hidden: false, tca: false, virtual_neo: true })).toEqual({ status: 'ok', tca: ['busy', 'Source: validation stream'], hidhide: ['off', 'HidHide: not needed'], neo: ['ok', 'Virtual NEO present'] });
    expect(app.pills('Idle', null)).toEqual({ status: 'off', tca: ['off', 'TCA: not open'], hidhide: ['off', 'HidHide: off'], neo: ['off', 'Virtual NEO: absent'] });
    expect(['Starting', 'Stopping', 'Error: x', 'unreachable. Reopen', 'unavailable (x)'].map((t) => app.pills(t, null).status)).toEqual(['busy', 'busy', 'fault', 'fault', 'fault']);
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
    await expect(api.del('/api/preview')).rejects.toThrow();
    expect(fetchImpl.mock.calls[2][1].method).toBe('DELETE');
  });
});

group('mapping tab', () => {
  it('builds diagrams, chips, NEO hotspots, the NEO table and the axes', async () => {
    await page();
    expect(document.querySelectorAll('[data-source]')).toHaveLength(53);
    expect(document.querySelectorAll('[data-source="q3"]')).toHaveLength(2);
    expect(tca('q12').getAttribute('aria-label')).toBe('Quadrant detent state 12');
    expect(tca('s5').style.getPropertyValue('--x')).toBe('7.179%');
    expect(document.querySelectorAll('[data-neo]')).toHaveLength(20);
    expect(neo('b5').getAttribute('aria-label')).toBe('NEO 5 (High-G)');
    expect(neo('b16').disabled).toBe(true);
    expect(neo('pov-up').querySelector('svg')).not.toBeNull();
    expect(neo('b5').style.getPropertyValue('--x')).toBe('43.234%');
    expect(neo('b5').style.getPropertyValue('--w')).toBe('5.611%');
    expect(document.querySelectorAll('[data-row]')).toHaveLength(16);
    expect(row('b7').textContent).toBe('7FlaresS6 · Q2');
    expect(row('b13').querySelector('.action').classList.contains('none')).toBe(true);
    expect(row('pov').querySelector('.fed').textContent).toBe('U: S11');
    expect($('axes').textContent).toContain('ZLeft quadrant lever or sidestick sliderThrottle');
    expect($('axes').textContent).toContain('Rx / RySidestick hatCamera');
    expect($('profile-path').textContent).toBe('C:\\neo\\data\\config.json');
    expect($('selected-name').textContent).toBe('Nothing selected');
    expect($('compare').hidden).toBe(true);
    expect($('deselect').disabled).toBe(true);
    expect($('recorded-count').textContent).toBe('1');
    expect($('recorded-empty').hidden).toBe(true);
    expect($('recorded').textContent).toContain('Old→ NEO 3 (Weapon select)was Nothing');
    expect($('pill-tca').textContent).toBe('TCA: not open');
    choose('hand', 'left');
    expect($('stick-img').getAttribute('src')).toBe('device/sidestick-left.svg');
    expect($('hand-caption').textContent).toBe('Left-hand setup');
    expect(tca('s5').style.getPropertyValue('--x')).toBe('92.308%');
    expect(document.querySelectorAll('#stick-spots [data-source]')).toHaveLength(17);
    choose('hand', 'right');
    expect($('stick-img').alt).toBe('TCA Sidestick, top view, right-hand setup, numbered controls 1 to 17');
  });

  it('loads without a profile path and keeps the placeholder', async () => {
    const api = fakeApi();
    const original = api.get;
    api.get = vi.fn(async (path) => (path === '/api/config' ? { config: defaults(), defaults: defaults(), status: 'Idle' } : original(path)));
    await page(api);
    expect($('profile-path').textContent).toBe('config.json');
  });

  it('selecting a TCA control lights its NEO mapping; NEO clicks build a new one', async () => {
    const p = await page();
    tca('s6').click();
    expect(pressed(tca('s6'))).toBe('true');
    expect($('selected-name').textContent).toBe('Sidestick button 6');
    expect($('mapped-now').textContent).toBe('NEO 7 (Flares)');
    expect($('mapped-new').textContent).toBe('Unchanged');
    expect($('selected-kind').textContent).toBe('TCA source');
    expect($('compare').hidden).toBe(false);
    expect(neo('b7').classList.contains('kept')).toBe(true);
    expect(pressed(neo('b7'))).toBe('true');
    expect($('record').disabled).toBe(true);
    expect($('neo-context').dataset.mode).toBe('sel');
    neo('b6').click();
    expect(neo('b6').classList.contains('added')).toBe(true);
    expect(row('b6').classList.contains('is-new')).toBe(true);
    neo('b7').click();
    expect(pressed(neo('b7'))).toBe('false');
    expect(neo('b7').classList.contains('removed')).toBe(true);
    expect($('diff').textContent).toBe('Adds NEO 6 (Autopilot)Removes NEO 7 (Flares)');
    expect($('neo-context-text').textContent).toBe('Mapping of Sidestick button 6 → NEO 6 (Autopilot)');
    neo('pov-down').click();
    expect($('mapped-new').textContent).toBe('NEO 6 (Autopilot), NEO POV down');
    neo('pov-down').click();
    expect(p.proposal.pov).toBeNull();
    neo('pov-left').click();
    expect(row('pov').classList.contains('is-new')).toBe(true);
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
    expect($('link-heading').textContent).toBe('Selected NEO control');
    expect($('mapped-now-label').textContent).toBe('Fed by');
    expect($('mapped-new').hidden).toBe(true);
    expect($('mapped-now').textContent).toBe('Sidestick button 6, Quadrant button 2');
    expect($('neo-context-text').textContent).toBe('Inspecting NEO 7 (Flares) · fed by S6 · Q2');
    expect(tca('s6').classList.contains('is-feed')).toBe(true);
    expect(pressed(neo('b7'))).toBe('true');
    expect(row('b7').classList.contains('is-sel')).toBe(true);
    expect($('deselect').disabled).toBe(false);
    row('b14').click();
    expect($('mapped-now').textContent).toBe('No TCA control');
    expect($('neo-context-text').textContent).toBe('Inspecting NEO 14 · fed by nothing');
    neo('pov-up').click();
    expect(row('pov').classList.contains('is-sel')).toBe(true);
    neo('b14').click();
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
    expect(radio('hat', 'camera').checked).toBe(true);
    expect($('axes').textContent).toContain('Sidestick hatCamera');
    p.live({ pressed: ['hat-up'], status: 'Emulating' });
    expect(p.selected).toBeNull();
    choose('hat', 'mapped');
    await vi.waitFor(() => expect($('link-status').textContent).toBe('The hat now sends its mapped controls; the camera mini-stick gets no input.'));
    expect(api.calls.at(-1)[2].hat).toBe('mapped');
    expect(tca('hat-up').disabled).toBe(false);
    expect(tca('hat-up').title).toBe('Sidestick hat up');
    expect($('hat-hint').textContent).toBe('Each direction maps like a button; the camera gets no input.');
    expect($('axes').textContent).toContain('No source (hat is mapped)Camera');
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
    expect($('mapped-now').textContent).toBe('Sidestick button 11, Sidestick hat up');
    expect(row('pov').querySelector('.fed').textContent).toBe('U: S11 · Hat up');
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
    expect(radio('hat', 'camera').checked).toBe(true);
    expect(p.busy).toBe(false);
  });
});

group('live input', () => {
  it('lights held controls and the hat, and selects new presses on the mapping tab', async () => {
    const p = await page();
    p.live({ pressed: [], status: 'Idle' });
    expect($('live-input').textContent).toBe('Start emulation to see presses.');
    const links = { source: 'tca', hidden: true, tca: true, virtual_neo: true };
    p.live({ pressed: [], status: 'Emulating', links });
    expect($('live-input').textContent).toBe('Nothing pressed');
    expect($('live-input').dataset.active).toBe('false');
    expect($('daemon-status').textContent).toBe('Daemon: Emulating');
    expect($('pill-status').dataset.state).toBe('ok');
    expect($('pill-hidhide').textContent).toBe('HidHide: TCA hidden');
    p.live({ pressed: ['s5', 'hat-up', 'hat-right'], status: 'Emulating', links });
    expect($('live-input').textContent).toBe('Now sending: Sidestick button 5, Sidestick hat up, Sidestick hat right');
    expect($('live-input').dataset.active).toBe('true');
    expect(tca('s5').classList.contains('held')).toBe(true);
    expect(tca('hat-up').classList.contains('held')).toBe(true);
    expect(tca('hat-down').classList.contains('held')).toBe(false);
    expect(p.selected).toBe('s5');
    p.live({ pressed: ['s5', 'hat-up', 'hat-right'], status: 'Emulating' }); // held: no reselection
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
    expect($('lever-rails').hidden).toBe(true);
    expect($('lever-state').textContent).toBe('Not split');
    p.live({ pressed: [], levers: [1, 0], status: 'Emulating' });
    expect($('lever-live').textContent).toBe('Left lever 100% · Right lever 0% · High-G active');
    expect($('lever-rails').hidden).toBe(false);
    expect($('lever-rails').style.getPropertyValue('--fw')).toBe('95%');
    expect($('needle-left').style.getPropertyValue('--pos')).toBe('100%');
    expect([$('value-left').dataset.zone, $('value-right').dataset.zone]).toEqual(['forward', 'back']);
    expect($('lever-state').textContent).toBe('High-G active · NEO 5 held');
    expect($('lever-state').dataset.active).toBe('true');
    p.live({ pressed: [], levers: [0.5, 0.5], status: 'Emulating' });
    expect($('value-left').dataset.zone).toBe('middle');
    expect($('lever-state').textContent).toBe('Not split');
    p.saved.split_high_g.enabled = false;
    p.live({ pressed: [], levers: null, status: 'Idle' });
    expect($('lever-live').textContent).toBe('Lever positions appear here during emulation.');
    expect($('lever-state').textContent).toBe('Rule off');
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
    expect($('lever-state').textContent).toBe('Rule off');
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
    choose('lever', 'right', 'input');
    expect($('tune-deadzone').value).toBe('4');
    expect($('tune-deadzone-range').value).toBe('4');
    $('tune-deadzone-range').value = '12';
    fire($('tune-deadzone-range'));
    expect($('tune-deadzone').value).toBe('12');
    expect(p.config.axes.roll.deadzone).toBe(0.12);
    $('camera').value = '50';
    fire($('camera'));
    expect($('camera-range').value).toBe('50');
    $('split-forward-range').value = '70';
    fire($('split-forward-range'));
    expect($('split-forward').value).toBe('70');
    $('split-back').value = '20';
    fire($('split-back'));
    expect($('split-back-range').value).toBe('20');
    $('detent-band').value = '5';
    fire($('detent-band')); // no slider partner
    expect(p.config.throttle.band).toBe(0.05);
    $('tune-curve').value = 'x';
    fire($('tune-curve')); // not a number: the draft keeps the last value
    expect(p.config.axes.roll.curve).toBe(0);
    $('tune-curve').value = '';
    fire($('tune-curve'));
    for (const id of ['invert-yaw', 'safe-start', 'split-enabled']) {
      $(id).checked = !$(id).checked;
      fire($(id));
    }
    expect([p.config.invert.yaw, p.config.safe_start, p.config.split_high_g.enabled]).toEqual([true, false, false]);
    expect($('dirty').textContent).toBe('Unsaved changes.');
    $('tune-deadzone').value = '60';
    await p.save();
    expect($('tune-deadzone-error').textContent).toBe('Enter a number from 0 to 50.');
    expect($('tune-curve-error').textContent).toBe('Enter a number from -100 to 100.');
    expect($('dirty').textContent).toBe('Fix the highlighted fields, then save.');
    expect($('dirty').dataset.state).toBe('error');
    $('tune-curve').value = '30';
    $('tune-deadzone').value = '50';
    $('tune-saturation').value = '50';
    await p.save();
    expect($('tune-saturation-error').textContent).toBe('Must be beyond the deadzone.');
    $('tune-deadzone').value = '40';
    $('tune-saturation').value = '90';
    $('detent-idle').value = '48';
    await p.save();
    expect($('detent-error').textContent).toBe('Needs idle < cruise − band and cruise + band < boost.');
    $('detent-idle').value = '10';
    $('detent-boost').value = '52';
    await p.save();
    expect($('detent-cruise').getAttribute('aria-invalid')).toBe('true');
    expect(api.put).not.toHaveBeenCalled();
    $('detent-boost').value = '90';
    await p.save();
    expect(api.calls.at(-1)[2]).toMatchObject({
      axes: { roll: { deadzone: 0.4, saturation: 0.9, curve: 0.3 } }, camera_strength: 0.5, throttle_lever: 'right', invert: { yaw: true },
      throttle: { band: 0.05 }, split_high_g: { forward: 0.7, back: 0.2 },
    });
    expect($('dirty').textContent).toBe('Saved.');
    expect($('dirty').dataset.state).toBe('saved');
    expect(p.saved.throttle_lever).toBe('right');
    api.put = vi.fn(async () => ({ saved: true, running: true }));
    $('profile').dispatchEvent(new Event('submit', { cancelable: true }));
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Saved. Applied to the running emulation.'));
    api.put = vi.fn(async () => { throw new Error('Yaw deadzone must be between 0 and 50%.'); });
    await p.save();
    expect($('dirty').textContent).toBe('Not saved: Yaw deadzone must be between 0 and 50%.');
    expect($('save').disabled).toBe(false);
    $('reset').click();
    expect(radio('lever', 'left').checked).toBe(true);
    expect($('dirty').dataset.state).toBe('dirty');
    expect($('dirty').textContent).toBe('Unsaved: defaults, including every mapping. Save settings to apply.');
    // Discard returns to the saved profile.
    $('discard').click();
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Changes discarded.'));
    expect(radio('lever', 'right').checked).toBe(true);
    expect(p.config).toEqual(p.saved);
  });
});

group('shared maths', () => {
  it('shapes axes exactly like the daemon (shared vectors)', () => {
    const vectors = JSON.parse(readFileSync('daemon/tests/data/axis-shape-vectors.json', 'utf8'));
    for (const { name, tuning, points } of vectors) {
      for (const [input, expected] of points) expect(app.shape(input, tuning), `${name} at ${input}`).toBeCloseTo(expected, 12);
    }
  });

  it('maps detents, throttle zones, curves and signed readouts', () => {
    const bands = { mode: 'detents', idle: 0.1, cruise: 0.5, boost: 0.9, band: 0.03 };
    expect([0.05, 0.1, 0.285, 0.5, 0.53, 0.9, 1].map((t) => app.detent(t, bands))).toEqual([-1, -1, -0.5, 0, 0, 1, 1]);
    expect(app.detent(0.71, bands)).toBeCloseTo(0.18 / 0.37, 12);
    expect(app.throttleOut(0.25, bands)).toBe(app.detent(0.25, bands));
    expect(app.throttleOut(0.25, { ...bands, mode: 'linear' })).toBe(-0.5);
    expect([-1, -0.4, 0, 0.4, 1].map((out) => app.zone(out, bands))).toEqual([['brake', 'Full brake'], ['brake', 'Braking'], ['cruise', 'Cruise'], ['boost', 'Boosting'], ['boost', 'Full boost']]);
    expect(app.zone(0.2, { mode: 'linear' })).toEqual([null, '60%']);
    expect([0.4, -0.4, 0, 0.001, -0.001].map(app.signed)).toEqual(['+0.40', '−0.40', ' 0.00', ' 0.00', ' 0.00']);
    const path = app.curvePath(axis());
    expect(path.startsWith('M10.0 230.0 L')).toBe(true);
    expect(path.endsWith('L230.0 10.0')).toBe(true);
    expect(path.split(' L')).toHaveLength(121);
  });
});

group('flight axes preview', () => {
  it('draws the selected axis from the draft and previews test or live input', async () => {
    const p = await page();
    $('tab-settings').click();
    expect($('curve-path').getAttribute('d')).toBe(app.curvePath(p.config.axes.roll));
    expect($('curve-band').getAttribute('width')).toBe('8.8');
    expect($('read-input-label').textContent).toBe('Roll · test');
    expect([$('read-input').textContent, $('read-output').textContent]).toEqual(['+0.40', `+${app.shape(0.4, axis()).toFixed(2)}`]);
    expect(document.querySelector('[data-axis=roll]').dataset.selected).toBe('true');
    $('test-input').value = '-50';
    fire($('test-input'));
    expect($('read-input').textContent).toBe('−0.50');
    expect($('curve-dot').getAttribute('cx')).toBe('65.0');
    // Another axis: its own values, curve and labels; the strip shows every axis.
    choose('axis', 'yaw', 'input');
    expect($('curve').getAttribute('aria-label')).toBe('Yaw response curve: stick input against NEO output');
    expect($('copy-axis').textContent).toBe('Copy yaw to roll and pitch');
    $('tune-curve').value = '60';
    fire($('tune-curve'));
    expect(p.config.axes.yaw.curve).toBe(0.6);
    expect(p.config.axes.roll.curve).toBe(0);
    $('copy-axis').click();
    expect([p.config.axes.roll.curve, p.config.axes.pitch.curve]).toEqual([0.6, 0.6]);
    expect($('dirty').textContent).toBe('Unsaved: Yaw settings copied to roll and pitch.');
    choose('axis', 'pitch', 'input');
    expect([$('tune-curve').value, $('copy-axis').textContent, $('reset-axis').textContent]).toEqual(['60', 'Copy pitch to roll', 'Reset pitch']);
    $('reset-axis').click();
    expect([$('tune-curve').value, p.config.axes.pitch.curve]).toEqual(['0', 0]);
    $('copy-axis').click();
    expect(p.config.axes.roll.curve).toBe(0);
    choose('axis', 'roll', 'input');
    $('tune-curve').value = '25';
    fire($('tune-curve'));
    $('copy-axis').click();
    expect(p.config.axes.pitch.curve).toBe(0.25);
    // Live input: the strip and graph follow the raw stick through the draft.
    choose('preview-source', 'live', 'input');
    expect($('read-input-label').textContent).toBe('Roll · no input yet');
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor() });
    expect($('read-input-label').textContent).toBe('Roll · live');
    expect($('read-input').textContent).toBe('+0.50');
    expect(document.querySelector('[data-axis=pitch] [data-out]').textContent).toBe(app.signed(app.shape(-0.25, p.config.axes.pitch)));
    const bar = document.querySelector('[data-axis=pitch] .minibar span');
    expect(bar.style.getPropertyValue('--from')).not.toBe('50%');
    // Dragging the test input switches back to it.
    fire($('test-input'));
    expect(radio('preview-source', 'test').checked).toBe(true);
    expect($('read-input-label').textContent).toBe('Roll · test');
  });
});

group('throttle', () => {
  it('previews linear and detent responses, and captures detents from the live lever', async () => {
    const p = await page();
    expect($('throttle-help').textContent).toBe('Lever travel maps straight to the NEO throttle. Mid-travel is cruise.');
    expect([$('throttle-zone').textContent, $('throttle-zone').dataset.zone]).toEqual(['50%', undefined]);
    expect($('detents').hidden).toBe(true);
    expect($('throttle-rail').dataset.mode).toBe('linear');
    choose('throttle-mode', 'detents', 'input');
    expect(p.config.throttle.mode).toBe('detents');
    expect($('detents').hidden).toBe(false);
    expect([$('throttle-zone').textContent, $('throttle-zone').dataset.zone]).toEqual(['Cruise', 'cruise']);
    expect($('throttle-out').textContent).toBe(' 0.00 · cruise');
    expect($('throttle-rail').style.getPropertyValue('--low')).toBe('47%');
    expect(document.querySelector('[data-mark=boost]').style.getPropertyValue('--at')).toBe('90%');
    $('test-lever').value = '5';
    fire($('test-lever'));
    expect([$('throttle-zone').textContent, $('throttle-out').textContent, $('throttle-bar').dataset.side]).toEqual(['Full brake', '−1.00 · brake', 'brake']);
    $('test-lever').value = '70';
    fire($('test-lever'));
    expect([$('throttle-zone').textContent, $('throttle-bar').dataset.side]).toEqual(['Boosting', 'boost']);
    // Capture needs the running emulation.
    expect(document.querySelector('[data-capture=cruise]').disabled).toBe(true);
    expect($('capture-hint').textContent).toBe('Start emulation to capture the lever in its detents.');
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor() });
    expect($('capture-hint').textContent).toBe('Put the left lever in a detent, then Capture.');
    expect($('throttle-source').textContent).toBe('Left lever · test');
    choose('preview-source', 'live', 'input');
    expect($('throttle-source').textContent).toBe('Left lever · live');
    expect($('throttle-needle').style.getPropertyValue('left')).toBe('80%');
    document.querySelector('[data-capture=boost]').click();
    expect([p.config.throttle.boost, $('detent-boost').value]).toEqual([0.8, '80']);
    expect($('dirty').textContent).toBe('Unsaved: Boost captured at 80%.');
    choose('lever', 'right', 'input');
    document.querySelector('[data-capture=idle]').click();
    expect(p.config.throttle.idle).toBe(0.3);
    expect($('dirty').textContent).toBe('Unsaved: Idle captured at 30%.');
    document.querySelector('[data-capture=cruise]').click();
    expect($('dirty').textContent).toBe('Unsaved: CL (cruise) captured at 30%.');
  });
});

group('control helpers', () => {
  it('picks the precision control and records the pause combo by pressing them', async () => {
    const p = await page();
    expect($('precision-source').textContent).toBe('Off');
    expect($('precision-clear').disabled).toBe(true);
    expect($('pause-combo').textContent).toBe('Off');
    $('precision-pick').click();
    const well = document.querySelector('[data-for=precision]');
    expect([well.hidden, well.textContent]).toEqual([false, 'Start emulation, then press the controls on the TCA.']);
    expect($('precision-pick').textContent).toBe('Cancel');
    p.live({ pressed: [], status: 'Emulating', links: LINKS });
    expect(well.textContent).toBe('Press the TCA control to hold for precision aiming.');
    p.live({ pressed: ['hat-up'], status: 'Emulating', links: LINKS }); // the hat cannot be picked
    expect(p.config.precision.source).toBeNull();
    p.live({ pressed: ['hat-up', 's4'], status: 'Emulating', links: LINKS });
    expect(p.config.precision.source).toBe('s4');
    expect([$('precision-source').textContent, well.hidden]).toEqual(['S4 · Sidestick button 4', true]);
    expect($('dirty').textContent).toBe('Unsaved: hold Sidestick button 4 for precision aiming.');
    $('precision-pick').click();
    $('precision-pick').click(); // cancel
    expect(p.capture).toBeNull();
    $('precision-clear').click();
    expect(p.config.precision.source).toBeNull();
    // Pause combo: presses accumulate (up to 4) until Done.
    $('pause-record').click();
    expect(document.querySelector('[data-for=pause]').textContent).toBe('Press and release the controls for the combo (up to 4), then choose Done.');
    expect($('pause-record').textContent).toBe('Done');
    for (const keys of [['s5'], [], ['q6'], ['s1', 's2', 's3']]) p.live({ pressed: keys, status: 'Emulating', links: LINKS });
    expect($('pause-combo').textContent).toBe('S5+Q6+S1+S2');
    p.renderSettings(); // unchanged combo: chips are kept
    $('pause-record').click();
    expect(p.config.combos.pause).toEqual(['s5', 'q6', 's1', 's2']);
    expect($('dirty').textContent).toBe('Unsaved: new pause combo.');
    expect($('pause-clear').disabled).toBe(false);
    $('pause-clear').click();
    expect([p.config.combos.pause, $('dirty').textContent]).toEqual([[], 'Unsaved: pause combo off.']);
    $('pause-record').click();
    $('pause-record').click(); // Done with nothing pressed
    expect($('dirty').textContent).toBe('Unsaved: pause combo off.');
    $('precision-scale').value = '30';
    fire($('precision-scale'));
    expect([$('precision-scale-range').value, p.config.precision.scale]).toEqual(['30', 0.3]);
  });

  it('shows the paused pill while input is paused', async () => {
    const p = await page();
    expect($('pill-paused').hidden).toBe(true);
    p.live({ pressed: [], status: 'Emulating', links: LINKS, paused: true });
    expect($('pill-paused').hidden).toBe(false);
  });
});

group('try in game', () => {
  it('previews the draft, coalesces drags, and ends with save, discard or leaving the tab', async () => {
    const api = fakeApi();
    const p = await page(api);
    $('tab-settings').click();
    $('try-live').checked = true;
    fire($('try-live'));
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Unsaved changes · running in game now.'));
    expect(api.calls.at(-1)[1]).toBe('/api/preview');
    expect($('pill-preview').hidden).toBe(false);
    // Edits while a preview request is in flight: one more request with the latest draft.
    let release;
    api.put = vi.fn(async (path, value) => { api.calls.push(['PUT', path, structuredClone(value)]); await new Promise((resolve) => { release = resolve; }); return { preview: true }; });
    $('tune-deadzone').value = '10';
    fire($('tune-deadzone'));
    $('tune-deadzone').value = '20';
    fire($('tune-deadzone'));
    $('tune-deadzone').value = '30';
    fire($('tune-deadzone'));
    expect(api.put).toHaveBeenCalledTimes(1);
    release();
    await vi.waitFor(() => expect(api.put).toHaveBeenCalledTimes(2));
    release();
    await vi.waitFor(() => expect(p.sending).toBe(false));
    expect(api.calls.at(-1)[2].axes.roll.deadzone).toBe(0.3);
    expect($('dirty').textContent).toBe('Unsaved changes · running in game now.');
    // An invalid draft is not sent.
    $('tune-deadzone').value = '70';
    fire($('tune-deadzone'));
    expect(api.put).toHaveBeenCalledTimes(2);
    $('tune-deadzone').value = '30';
    // While emulating the preview stays; when the daemon reports it gone (emulation stopped), the page follows.
    api.put = fakeApi().put;
    p.live({ pressed: [], status: 'Emulating', links: LINKS, preview: true });
    expect(p.previewing).toBe(true);
    p.live({ pressed: [], status: 'Idle', preview: false });
    expect([p.previewing, $('dirty').textContent]).toEqual([false, 'Unsaved changes · the preview ended with emulation.']);
    // Leaving the tab ends a preview but keeps the draft.
    await p.setPreview(true);
    $('tab-mapping').click();
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Unsaved changes · preview ended when you left this tab.'));
    expect(api.calls.at(-1)).toEqual(['DELETE', '/api/preview']);
    expect(p.config.axes.roll.deadzone).toBe(0.3);
    $('tab-mapping').click(); // no preview: nothing to end
    // Saving ends it too.
    $('tab-settings').click();
    await p.setPreview(true);
    await p.save();
    expect([p.previewing, $('try-live').checked]).toEqual([false, false]);
    // Discard ends it and restores the saved profile.
    $('tune-deadzone').value = '12';
    fire($('tune-deadzone'));
    await p.setPreview(true);
    $('discard').click();
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Changes discarded.'));
    expect([p.previewing, $('tune-deadzone').value]).toEqual([false, '30']);
  });

  it('refuses an invalid draft and reports preview failures', async () => {
    const api = fakeApi({
      put: vi.fn(async () => { throw new Error('Throttle detents need …'); }),
      del: vi.fn(async () => { throw new Error('offline'); }),
    });
    const p = await page(api);
    $('tune-saturation').value = '20';
    $('try-live').checked = true;
    fire($('try-live'));
    expect([$('try-live').checked, p.previewing, api.put.mock.calls.length]).toEqual([false, false, 0]);
    $('tune-saturation').value = '100';
    await p.setPreview(true);
    expect([p.previewing, $('dirty').textContent, $('dirty').dataset.state]).toEqual([false, 'Preview stopped: Throttle detents need …', 'error']);
    p.previewing = true;
    await p.setPreview(false);
    expect($('dirty').textContent).toBe('Preview not ended: offline');
    p.previewing = true;
    await p.discard();
    expect($('dirty').textContent).toBe('Changes discarded.');
  });

  it('opens on a preview the daemon is already running', async () => {
    const api = fakeApi();
    const original = api.get;
    const draft = defaults();
    draft.axes.yaw.curve = 0.5;
    api.get = vi.fn(async (path) => (path === '/api/config' ? { config: defaults(), defaults: defaults(), status: 'Emulating', links: LINKS, preview: draft } : original(path)));
    const p = await page(api);
    expect([p.previewing, p.config.axes.yaw.curve, p.saved.axes.yaw.curve]).toEqual([true, 0.5, 0]);
    expect([$('try-live').checked, $('pill-preview').hidden, $('dirty').textContent]).toEqual([true, false, 'Unsaved changes · running in game now.']);
  });
});

group('behaviour', () => {
  it('shows and records how a source drives its NEO targets', async () => {
    const api = fakeApi();
    const p = await page(api);
    expect($('behaviour').hidden).toBe(true);
    tca('q3').click();
    expect($('behaviour').hidden).toBe(false);
    expect(radio('behaviour', 'pulse_change').checked).toBe(true);
    expect($('mapped-now').textContent).toBe('NEO 6 (Autopilot) · pulse on change');
    expect($('timeline-out').children).toHaveLength(2);
    expect($('timeline').getAttribute('aria-label')).toBe('What the game receives with Pulse on change');
    choose('behaviour', 'hold');
    expect($('timeline-out').children[0].style.width).toBe('55%');
    expect($('behaviour-help').textContent).toBe(app.MODES.hold.help);
    expect($('diff').textContent).toBe('Behaviour: Hold');
    expect($('record').disabled).toBe(false);
    await p.record();
    expect('q3' in api.calls.find((c) => c[0] === 'PUT' && c[1] === '/api/config')[2].behaviours).toBe(false);
    expect(p.config.behaviours).toEqual({});
    tca('s1').click();
    choose('behaviour', 'toggle');
    await p.record();
    expect(p.saved.behaviours.s1).toBe('toggle');
    expect($('link-status').textContent).toBe('Recorded Sidestick button 1: NEO 1 (Gun) · toggle.');
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
