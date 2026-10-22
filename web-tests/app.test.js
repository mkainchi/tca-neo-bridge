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
    precision: { source: null, scale: 0.5 }, safe_start: true, combos: { pause: [], next_profile: [] },
    behaviours: { q3: 'pulse_change' }, timing: { pulse_ms: 80, long_ms: 500, double_ms: 250, repeat_ms: 120 },
    long_press: {}, double_tap: {}, shift: null, shifted: { buttons: {}, pov: {} },
    camera: { ramp_ms: 0, full_after_hold: false, lever: false }, yaw_source: 'twist',
    head_tracking: { enabled: false, port: 4242, yaw_range: 45, pitch_range: 30, invert_yaw: false, invert_pitch: false, timeout_ms: 500, tuning: { ...axis(), deadzone: 0.03 } },
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
      if (path === '/api/profiles') return { profiles: ['Attack', 'Cruise'], active: 'Cruise' };
      return { pressed: [], status: 'Idle' };
    }),
    post: vi.fn(async (path) => { calls.push(['POST', path]); return { profiles: ['Attack', 'Cruise'], active: 'Attack', config: { ...defaults(), camera_strength: 0.5 }, running: true }; }),
    put: vi.fn(async (path, value) => {
      calls.push(['PUT', path, structuredClone(value)]);
      if (path === '/api/labels') return { saved: value.length };
      if (path.startsWith('/api/profiles/')) return { profiles: ['Attack', 'Cruise', 'Dogfight'], active: 'Dogfight', config: defaults(), running: false };
      return path === '/api/preview' ? { preview: true, running: true } : { saved: true, running: false };
    }),
    del: vi.fn(async (path) => { calls.push(['DELETE', path]); return path.startsWith('/api/profiles/') ? { profiles: ['Attack'], active: null, config: defaults() } : { preview: false }; }),
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
    await expect(api.post('/api/profiles/A/activate')).rejects.toThrow();
    expect(fetchImpl.mock.calls[3][1].method).toBe('POST');
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
    $('nav-axes').click();
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
    // The rails follow the draft thresholds before they are saved.
    p.live({ pressed: [], levers: [0.6, 0.04], status: 'Emulating' });
    expect($('lever-state').textContent).toBe('Not split');
    $('split-forward').value = '50';
    fire($('split-forward'));
    expect($('lever-state').textContent).toBe('High-G active · NEO 5 held');
    expect($('lever-rails').style.getPropertyValue('--fw')).toBe('50%');
    p.config.split_high_g.enabled = false;
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
  it('shows one rail page at a time, with the save bar on the settings pages', async () => {
    const p = await page();
    expect($('foot-actions').hidden).toBe(true);
    for (const name of app.PAGES) {
      $(`nav-${name}`).click();
      expect(app.PAGES.filter((other) => !$(`page-${other}`).hidden)).toEqual([name]);
      expect($(`nav-${name}`).getAttribute('aria-current')).toBe('page');
      expect(document.querySelectorAll('[aria-current=page]')).toHaveLength(1);
      expect($('foot-actions').hidden).toBe(name === 'mapping' || name === 'live');
    }
    expect(p.page).toBe('live');
    // Typing outside the settings form is not a settings edit.
    fire($('profile-new'));
    expect(p.dirty).toBe(false);
  });

  it('switches the mapping side panel by click and arrow keys, and back to the selection on a pick', async () => {
    await page();
    $('side-neo').click();
    expect($('side-neo-panel').hidden).toBe(false);
    expect($('side-sel-panel').hidden).toBe(true);
    expect($('side-neo').tabIndex).toBe(0);
    $('side-neo').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    expect($('side-log-panel').hidden).toBe(false);
    expect(document.activeElement).toBe($('side-log'));
    $('side-log').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    expect(document.activeElement).toBe($('side-sel'));
    $('side-sel').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft' }));
    expect(document.activeElement).toBe($('side-log'));
    $('side-log').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    expect($('side-log-panel').hidden).toBe(false);
    tca('s2').click();
    expect($('side-sel-panel').hidden).toBe(false);
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
    $('nav-axes').click();
    expect($('test-input-wrap').hidden).toBe(false);
    expect($('rail-live-text').textContent).toBe('Idle');
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
    // Live input is the default: once the device reports, the strip and graph follow it and the test input hides.
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor() });
    expect($('read-input-label').textContent).toBe('Roll · live');
    expect($('read-input').textContent).toBe('+0.50');
    expect($('test-input-wrap').hidden).toBe(true);
    expect([$('rail-live').dataset.live, $('rail-live-text').textContent]).toEqual(['true', 'Live input']);
    expect([...document.querySelectorAll('[data-livechip]')].map((chip) => chip.textContent)).toEqual(['Live input', 'Live input', 'Live input']);
    expect(document.querySelector('[data-axis=pitch] [data-out]').textContent).toBe(app.signed(app.shape(-0.25, p.config.axes.pitch)));
    const [raw, out] = document.querySelectorAll('[data-axis=pitch] .minibar span');
    expect([raw.style.getPropertyValue('--size'), out.style.getPropertyValue('--from')]).toEqual(['12.5%', `${50 + app.shape(-0.25, p.config.axes.pitch) * 50}%`]);
    // Without device input it falls back to the test input.
    p.live({ pressed: [], status: 'Idle' });
    expect([$('read-input-label').textContent, $('test-input-wrap').hidden, $('curve-name').textContent]).toEqual(['Roll · test', false, 'Roll']);
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
    expect([$('throttle-source').textContent, $('test-lever-wrap').hidden]).toEqual(['Left lever · live', true]);
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
    $('nav-axes').click();
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
    // Moving between settings pages keeps a preview; leaving them ends it but keeps the draft.
    await p.setPreview(true);
    $('nav-camera').click();
    expect(p.previewing).toBe(true);
    $('nav-mapping').click();
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Unsaved changes · preview ended when you left the settings.'));
    expect(api.calls.at(-1)).toEqual(['DELETE', '/api/preview']);
    expect(p.config.axes.roll.deadzone).toBe(0.3);
    $('nav-mapping').click(); // no preview: nothing to end
    // Saving ends it too.
    $('nav-axes').click();
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

group('gestures and layers', () => {
  it('reads each action and layer of a source', () => {
    const config = defaults();
    config.long_press = { s2: { buttons: [4, 3], pov: 'left' } };
    config.shifted = { buttons: { s1: [9] }, pov: { s2: 'down' } };
    expect(app.current(config, 's2', 'long')).toEqual({ buttons: [3, 4], pov: 'left', mode: 'hold' });
    expect(app.current(config, 's2', 'double')).toEqual({ buttons: [], pov: null, mode: 'hold' });
    expect(app.current(config, 's1', 'press', 'shift')).toEqual({ buttons: [9], pov: null, mode: 'hold' });
    expect(app.current(config, 's2', 'press', 'shift')).toEqual({ buttons: [], pov: 'down', mode: 'hold' });
    delete config.shifted;
    delete config.long_press;
    expect(app.current(config, 's1', 'press', 'shift').buttons).toEqual([]);
    expect(app.current(config, 's2', 'long').buttons).toEqual([]);
    expect(app.MODES.repeat.pulses).toHaveLength(4);
  });

  it('maps long press and double tap, and clears them', async () => {
    const api = fakeApi();
    const p = await page(api);
    choose('slot', 'press'); // nothing selected yet: no proposal to rebuild
    expect(p.proposal).toBeNull();
    tca('s2').click();
    expect($('slots').hidden).toBe(false);
    expect(document.querySelector('[data-slot=press]').textContent).toBe('Nothing');
    choose('slot', 'long');
    expect([$('behaviour').hidden, $('gestures').hidden]).toEqual([true, false]);
    expect($('slot-help').textContent).toBe(app.SLOTS.long.help);
    expect($('gesture-help').textContent).toContain('(250 ms)');
    neo('b3').click();
    neo('pov-left').click();
    expect($('neo-context-text').textContent).toBe('Mapping of Sidestick button 2 · long press → NEO 3 (Weapon select), NEO POV left');
    await p.record();
    expect(p.saved.long_press.s2).toEqual({ buttons: [3], pov: 'left' });
    expect($('link-status').textContent).toBe('Recorded Sidestick button 2 · long press: NEO 3 (Weapon select), NEO POV left.');
    expect(document.querySelector('[data-slot=long]').textContent).toBe('NEO 3 (Weapon select), NEO POV left');
    choose('slot', 'double');
    neo('b4').click();
    await p.record();
    expect(p.saved.double_tap.s2).toEqual({ buttons: [4], pov: null });
    // Removing every target removes the action.
    choose('slot', 'long');
    neo('b3').click();
    neo('pov-left').click();
    $('record').click();
    await vi.waitFor(() => expect('s2' in p.saved.long_press).toBe(false));
    expect(p.config.double_tap.s2).toEqual({ buttons: [4], pov: null });
    choose('slot', 'press');
    expect($('behaviour').hidden).toBe(false);
    choose('behaviour', 'repeat');
    expect($('timeline-out').children).toHaveLength(4);
  });

  it('edits the shift layer once a shift control is set', async () => {
    const api = fakeApi();
    const p = await page(api);
    tca('s1').click();
    expect(radio('layer', 'shift').disabled).toBe(true);
    expect(radio('layer', 'shift').parentElement.title).toBe('Choose a shift control on the Buttons & helpers page');
    expect($('layer-note').hidden).toBe(true);
    p.saved.shift = 's16';
    choose('layer', 'shift');
    expect($('layer-shift').textContent).toBe('Shift · S16');
    expect([$('layer-note').hidden, $('layer-note').textContent]).toEqual([false, 'Shift layer: these mappings apply while Sidestick button 16 is held. Controls without one keep their normal mapping.']);
    expect($('mapped-now').textContent).toBe('Normal: NEO 1 (Gun)');
    expect(document.querySelector('[data-slot=long]').textContent).toBe('Nothing');
    expect(radio('slot', 'long').disabled).toBe(true);
    expect($('slot-help').textContent).toBe('The shift layer changes the press only; long press and double tap are the same in both layers.');
    neo('b9').click();
    expect($('mapped-new').textContent).toBe('NEO 9 (Radar)');
    neo('pov-up').click();
    await p.record();
    expect(p.saved.shifted).toEqual({ buttons: { s1: [9] }, pov: { s1: 'up' } });
    expect(p.saved.buttons.s1).toEqual([1]); // the normal layer is untouched
    expect($('link-status').textContent).toBe('Recorded Sidestick button 1 (shift layer): NEO 9 (Radar), NEO POV up.');
    neo('b9').click();
    neo('pov-up').click();
    await p.record();
    expect(p.saved.shifted).toEqual({ buttons: {}, pov: {} });
    // Choosing the shift layer while on a long press goes back to the press.
    choose('layer', 'normal');
    choose('slot', 'long');
    choose('layer', 'shift');
    expect(p.slot).toBe('press');
    // Without a shift control any more, the normal layer comes back.
    p.saved.shift = null;
    $('deselect').click();
    expect([p.layer, radio('layer', 'normal').checked]).toEqual(['normal', true]);
    // Profiles without a shifted layer yet.
    delete p.saved.shifted;
    p.saved.shift = 's16';
    tca('s3').click();
    choose('layer', 'shift');
    neo('b2').click();
    await p.record();
    expect(p.saved.shifted).toEqual({ buttons: { s3: [2] }, pov: {} });
  });
});

group('camera, yaw and timing', () => {
  it('previews the camera ramp and explains the camera and yaw sources', async () => {
    const p = await page();
    $('nav-camera').click();
    expect($('camera-source-help').textContent).toBe('Only the hat moves the camera.');
    expect($('ramp').getAttribute('aria-label')).toBe('Camera output while the hat is held: 100% at once.');
    expect($('ramp-long').style.getPropertyValue('display')).toBe('none');
    $('camera-ramp-range').value = '300';
    fire($('camera-ramp-range'));
    expect([$('camera-ramp').value, p.config.camera.ramp_ms]).toEqual(['300', 300]);
    $('camera-full').checked = true;
    fire($('camera-full'));
    expect(p.config.camera.full_after_hold).toBe(true);
    expect($('ramp').getAttribute('aria-label')).toBe('Camera output while the hat is held: ramps to 100% over 300 ms, full after 500 ms.');
    expect($('ramp-long').getAttribute('x1')).toBe(String((10 + (500 / 1500) * 280).toFixed(1)));
    expect($('ramp-path').getAttribute('d')).toBe(app.rampPath(p.config.camera, 1, 500));
    expect(app.rampPath({ ramp_ms: 0, full_after_hold: false }, 0.5, 500).startsWith('M10.0 50.0')).toBe(true);
    expect(app.rampPath({ ramp_ms: 600, full_after_hold: true }, 0.5, 300).endsWith('L290.0 12.0')).toBe(true);
    for (const [source, help] of [['off', 'No yaw input'], ['twist', 'The sidestick twist']]) {
      choose('yaw-source', source, 'input');
      expect($('yaw-help').textContent.startsWith(help)).toBe(true);
    }
    expect($('dirty').textContent).toBe('Unsaved changes.');
  });

  it('draws the hat hold on the ramp as it happens, with the camera output', async () => {
    const p = await page();
    let now = 1000;
    p.clock = () => now;
    expect([$('hat-state').textContent, $('ramp-now').style.getPropertyValue('display'), $('cam-x-text').textContent]).toEqual(['Hat released', 'none', ' 0.00']);
    const axes = [32768, 32768, 32768, 65535, 0, 32768, 32768];
    p.live({ pressed: ['hat-right'], status: 'Emulating', links: LINKS, monitor: monitor({ axes }) });
    now = 1750;
    p.live({ pressed: ['hat-right', 'hat-up'], status: 'Emulating', links: LINKS, monitor: monitor({ axes }) });
    expect([$('hat-state').textContent, $('hat-state').dataset.tone]).toEqual(['Hat held · 750 ms', 'ok']);
    expect([$('ramp-now').getAttribute('x1'), $('ramp-hold').getAttribute('width'), $('ramp-now').style.getPropertyValue('display')]).toEqual(['150.0', '140.0', '']);
    expect([$('cam-x-text').textContent, $('cam-y-text').textContent, $('cam-x').style.getPropertyValue('--size')]).toEqual(['+1.00', '−1.00', '50%']);
    now = 9000; // past the graph: pinned to its end
    p.live({ pressed: ['hat-up'], status: 'Emulating', links: LINKS, monitor: monitor({ axes }) });
    expect($('ramp-now').getAttribute('x1')).toBe('290.0');
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor() });
    expect([$('hat-state').textContent, $('ramp-hold').getAttribute('width')]).toEqual(['Hat released', '0.0']);
    // A mapped hat does not drive the camera.
    p.saved.hat = 'mapped';
    p.live({ pressed: ['hat-up'], status: 'Emulating', links: LINKS, monitor: monitor() });
    expect([p.hatSince, $('hat-state').textContent, $('hat-state').dataset.tone]).toEqual([null, 'Hat mapped as buttons', '']);
  });

  it('turns the other lever features off when one needs the levers', async () => {
    const p = await page(); // split-lever High-G is on
    choose('camera-source', 'lever', 'input');
    expect([p.config.camera.lever, p.config.split_high_g.enabled, $('split-enabled').checked]).toEqual([true, false, false]);
    expect($('dirty').textContent).toBe('Unsaved changes: split-lever High-G turned off, since it uses the quadrant levers too.');
    expect($('camera-source-help').textContent).toContain('The right quadrant lever');
    choose('lever', 'right', 'input');
    expect($('camera-source-help').textContent).toContain('The left quadrant lever');
    choose('yaw-source', 'lever_differential', 'input');
    expect([p.config.yaw_source, p.config.camera.lever, radio('camera-source', 'hat').checked]).toEqual(['lever_differential', false, true]);
    expect($('dirty').textContent).toBe('Unsaved changes: the camera lever turned off, since it uses the quadrant levers too.');
    expect($('yaw-help').textContent).toContain('average of both levers');
    $('split-enabled').checked = true;
    fire($('split-enabled'));
    expect([p.config.split_high_g.enabled, p.config.yaw_source, radio('yaw-source', 'twist').checked]).toEqual([true, 'twist', true]);
    expect($('dirty').textContent).toBe('Unsaved changes: lever yaw turned off, since it uses the quadrant levers too.');
    choose('camera-source', 'hat', 'input');
    expect($('dirty').textContent).toBe('Unsaved changes.');
    // With nothing else on, turning one on says nothing extra.
    $('split-enabled').checked = false;
    fire($('split-enabled'));
    choose('yaw-source', 'lever_differential', 'input');
    expect($('dirty').textContent).toBe('Unsaved changes.');
    // The mapping tab's axes table follows the saved sources.
    p.saved.yaw_source = 'lever_differential';
    p.saved.camera.lever = true;
    p.render();
    expect($('axes').textContent).toContain('RzQuadrant lever differenceYaw');
    expect($('axes').textContent).toContain('Average of both quadrant levers or sidestick sliderThrottle');
    expect($('axes').textContent).toContain('Sidestick hat + spare leverCamera');
    p.saved.yaw_source = 'off';
    p.render();
    expect($('axes').textContent).toContain('No source (yaw off)Yaw');
  });

  it('shows the smoothing delay and validates the timings', async () => {
    const api = fakeApi();
    const p = await page(api);
    expect($('tune-lag').textContent).toBe('No added delay');
    $('tune-smoothing').value = '50';
    fire($('tune-smoothing'));
    expect([$('tune-lag').textContent, app.lag(0.5), app.lag(0.9)]).toEqual(['≈ 10 ms added delay', 10, 90]);
    $('timing-repeat').value = '80';
    await p.save();
    expect($('timing-error').textContent).toBe('Repeat every must be longer than the pulse length.');
    $('timing-repeat').value = '150';
    $('timing-long').value = '100';
    $('timing-double').value = '300';
    await p.save();
    expect($('timing-error').textContent).toBe('Enter a number from 200 to 2000.'); // not cleared by the valid double-tap field
    $('timing-long').value = '600';
    await p.save();
    expect(api.calls.at(-1)[2].timing).toEqual({ pulse_ms: 80, long_ms: 600, double_ms: 300, repeat_ms: 150 });
    expect(api.calls.at(-1)[2].axes.roll.smoothing).toBe(0.5);
  });

  it('picks and clears the shift control by pressing it', async () => {
    const p = await page();
    $('shift-pick').click();
    const well = document.querySelector('[data-for=shift]');
    p.live({ pressed: [], status: 'Emulating', links: LINKS });
    expect([well.hidden, well.textContent]).toEqual([false, 'Press the TCA control to hold for the shift layer. Its own mapping is not sent while it is the shift control.']);
    p.live({ pressed: ['s16'], status: 'Emulating', links: LINKS });
    expect([p.config.shift, $('shift-source').textContent, $('dirty').textContent]).toEqual(['s16', 'S16 · Sidestick button 16', 'Unsaved: hold Sidestick button 16 for the shift layer.']);
    $('shift-clear').click();
    expect([p.config.shift, $('dirty').textContent]).toEqual([null, 'Unsaved: shift layer off.']);
  });
});

group('profiles', () => {
  it('lists profiles, opens and closes the menu, and switches profile', async () => {
    const api = fakeApi();
    const p = await page(api);
    expect($('profile-name').textContent).toBe('Cruise');
    expect([...document.querySelectorAll('#profile-list button')].map((b) => [b.textContent, b.getAttribute('aria-current')])).toEqual([['Attack', 'false'], ['Cruiseactive', 'true']]);
    expect([$('profiles-empty').hidden, $('profile-delete').disabled]).toEqual([true, false]);
    $('profile-picker').click();
    expect([$('profile-menu').hidden, $('profile-picker').getAttribute('aria-expanded'), document.activeElement]).toEqual([false, 'true', $('profile-new')]);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect([$('profile-menu').hidden, document.activeElement]).toEqual([true, $('profile-picker')]);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); // already closed: nothing to do
    $('profile-picker').click();
    $('profile-list').click(); // inside the menu: stays open
    expect($('profile-menu').hidden).toBe(false);
    document.body.click(); // outside: closes
    expect($('profile-menu').hidden).toBe(true);
    document.body.click();
    // Switching with unsaved edits is refused; otherwise the daemon's profile becomes the page's state.
    p.mark(true, 'Unsaved changes.');
    document.querySelector('#profile-list button').click();
    expect([$('profile-status').textContent, $('profile-status').dataset.state, api.post.mock.calls.length]).toEqual(['Save or discard unsaved settings first.', 'error', 0]);
    p.mark(false, '');
    await p.activate('Attack');
    expect(api.calls.at(-1)).toEqual(['POST', '/api/profiles/Attack/activate']);
    expect([p.saved.camera_strength, $('camera').value, $('profile-name').textContent, $('dirty').textContent]).toEqual([0.5, '50', 'Attack', 'Switched to Attack. Applied to the running emulation.']);
    api.post = vi.fn(async () => ({ profiles: ['Attack'], active: 'Attack', config: defaults(), running: false }));
    await p.activate('Attack');
    expect($('profile-status').textContent).toBe('Switched to Attack.');
    api.post = vi.fn(async () => { throw new Error('There is no profile named Gone.'); });
    await p.activate('Gone');
    expect($('profile-status').textContent).toBe('Not switched: There is no profile named Gone.');
  });

  it('saves the saved settings under a name, and deletes the active profile in two clicks', async () => {
    const api = fakeApi();
    const p = await page(api);
    $('profile-new').value = ' bad/name ';
    $('save-as').dispatchEvent(new Event('submit', { cancelable: true }));
    expect([$('profile-new').getAttribute('aria-invalid'), $('profile-status').textContent]).toEqual(['true', 'Use 1 to 40 letters, digits, spaces, _ or -.']);
    $('profile-new').value = 'Ground attack';
    p.mark(true, 'Unsaved changes.');
    await p.saveAs();
    expect($('profile-status').textContent).toBe('Save or discard unsaved settings first.');
    p.mark(false, '');
    await p.saveAs();
    expect(api.calls.at(-1)[1]).toBe('/api/profiles/Ground%20attack');
    expect([$('profile-new').value, $('profile-name').textContent, $('profile-status').textContent]).toEqual(['', 'Dogfight', 'Saved as Ground attack.']);
    api.put = vi.fn(async () => { throw new Error('The profiles folder could not be created.'); });
    $('profile-new').value = 'X';
    await p.saveAs();
    expect($('profile-status').textContent).toBe('Not saved: The profiles folder could not be created.');
    // Delete asks first, then deletes; the settings stay.
    $('profile-delete').click();
    expect($('profile-delete').textContent).toBe('Delete Dogfight? Click again');
    $('profile-delete').click();
    await vi.waitFor(() => expect($('profile-status').textContent).toBe('Deleted Dogfight. The current settings are kept.'));
    expect([$('profile-name').textContent, $('profile-delete').disabled, $('profile-delete').textContent]).toEqual(['Unsaved profile', true, 'Delete']);
    p.renderProfiles({ profiles: ['Attack'], active: 'Attack' });
    api.del = vi.fn(async () => { throw new Error('offline'); });
    await p.deleteProfile();
    await p.deleteProfile();
    expect([$('profile-status').textContent, $('profile-name').textContent]).toEqual(['Not deleted: offline', 'Attack']);
    p.renderProfiles({ profiles: [], active: null });
    expect($('profiles-empty').hidden).toBe(false);
  });

  it('reports a profile list that cannot be read', async () => {
    const api = fakeApi();
    const original = api.get;
    api.get = vi.fn(async (path) => { if (path === '/api/profiles') throw new Error('offline'); return original(path); });
    await page(api);
    expect([$('profile-status').textContent, $('profile-status').dataset.state]).toEqual(['Saved profiles could not be listed: offline', 'error']);
  });

  it('exports the saved settings and imports a checked profile file', async () => {
    const api = fakeApi();
    const p = await page(api);
    const clicks = [];
    vi.stubGlobal('URL', { createObjectURL: vi.fn(() => 'blob:x'), revokeObjectURL: vi.fn() });
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function record() { clicks.push(this.download); });
    $('profile-export').click();
    expect([clicks, $('profile-status').textContent, URL.revokeObjectURL.mock.calls[0][0]]).toEqual([['Cruise.json'], 'Exported Cruise.json.', 'blob:x']);
    p.active = null;
    p.exportProfile();
    expect(clicks.at(-1)).toBe('tca-neo-bridge.json');
    click.mockRestore();
    vi.unstubAllGlobals();
    // Import: the daemon validates and saves it; the page then shows it.
    const file = (text) => ({ name: 'shared.json', text: async () => text });
    const imported = { ...defaults(), camera_strength: 0.25 };
    api.get = vi.fn(async (path) => (path === '/api/config' ? { config: imported } : {}));
    await p.importFile(file(JSON.stringify(imported)));
    expect(api.calls.at(-1)).toEqual(['PUT', '/api/config', imported]);
    expect([p.saved.camera_strength, $('profile-status').textContent]).toEqual([0.25, 'Imported shared.json. Save it under a name to keep it as a profile.']);
    await p.importFile(file('{nope'));
    expect($('profile-status').textContent.startsWith('Not imported:')).toBe(true);
    await p.importFile(undefined);
    p.mark(true, 'Unsaved changes.');
    await p.importFile(file('{}'));
    expect($('profile-status').textContent).toBe('Save or discard unsaved settings first.');
    // The file input hands its file over.
    const spy = vi.spyOn(p, 'importFile').mockResolvedValue();
    $('profile-import').dispatchEvent(new Event('change'));
    expect(spy).toHaveBeenCalledWith(undefined);
  });
});

group('next profile combo', () => {
  it('records and clears the next-profile combo by pressing it', async () => {
    const p = await page();
    $('next-record').click();
    p.live({ pressed: [], status: 'Emulating', links: LINKS });
    expect(document.querySelector('[data-for=next]').textContent).toBe('Press and release the controls for the combo (up to 4), then choose Done.');
    p.live({ pressed: ['s16', 'q6'], status: 'Emulating', links: LINKS });
    $('next-record').click();
    expect([p.config.combos.next_profile, $('dirty').textContent, $('next-combo').textContent]).toEqual([['s16', 'q6'], 'Unsaved: new next-profile combo.', 'S16+Q6']);
    $('next-clear').click();
    expect([p.config.combos.next_profile, $('dirty').textContent]).toEqual([[], 'Unsaved: next-profile combo off.']);
  });
});

group('live tab', () => {
  it('shows raw input next to what the NEO receives, the state, NEO buttons and POV', async () => {
    const p = await page();
    $('nav-live').click();
    expect($('page-live').hidden).toBe(false);
    expect([$('live-empty').hidden, $('live-axes').children.length, $('live-pov-text').textContent]).toEqual([false, 0, 'POV centred']);
    expect([...$('live-states').children].map((s) => [s.textContent, s.dataset.tone])).toEqual([
      ['Throttle armed', 'ok'], ['Precision off', ''], ['Shift off', ''], ['Input live', 'ok'], ['Preview off', ''],
    ]);
    const axes = [16384, 49151, 0, 65535, 0, 32768, 32768];
    p.live({ pressed: ['s1', 'hat-down'], status: 'Emulating', links: LINKS, paused: true, preview: true, monitor: monitor({ axes, armed: false, precision: true, shift: true, buttons: 0b101, pov: 2 }) });
    const rows = [...$('live-axes').children].map((row) => [row.querySelector('.name').firstChild.textContent, row.querySelector('.name span').textContent, row.querySelector('.value').textContent]);
    expect(rows).toEqual([['Roll', 'Stick X', '−0.50'], ['Pitch', 'Stick Y', '+0.50'], ['Yaw', 'Twist', ' 0.00'], ['Throttle', 'Left lever', '+1.00'], ['Camera X', 'Hat', '+1.00'], ['Camera Y', 'Hat', '−1.00']]);
    expect([...$('live-states').children].map((s) => s.textContent)).toEqual(['Throttle waiting for centre', 'Precision on', 'Shift held', 'Input paused', 'Previewing unsaved']);
    expect($('live-pressed').textContent).toBe('S1Hat down');
    expect([...$('live-buttons').children].map((b) => b.dataset.on).slice(0, 3)).toEqual(['true', 'false', 'true']);
    expect([$('live-pov').getAttribute('aria-label'), [...$('live-pov').children].findIndex((c) => c.dataset.on === 'true')]).toEqual(['POV right', 5]);
    // Not rendered while another page is shown; shown from the last data when the page opens.
    $('nav-axes').click();
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor({ pov: 8 }) });
    expect($('live-pov-text').textContent).toBe('POV right');
    $('nav-live').click();
    expect($('live-pov-text').textContent).toBe('POV centred');
  });

  it('describes each source the way the profile reads them', () => {
    const config = defaults();
    const m = monitor({ levers: [0.8, 0.2], yaw: 0.3 });
    const rows = (overrides, pressed = []) => app.liveRows(m, pressed, { ...config, ...overrides }).map(([name, source, raw]) => [name, source, Math.round(raw * 100) / 100]);
    expect(rows({ throttle_lever: 'right' })[3]).toEqual(['Throttle', 'Right lever', -0.6]);
    expect(rows({ yaw_source: 'lever_differential' }).slice(2, 4)).toEqual([['Yaw', 'Lever difference', 0.6], ['Throttle', 'Both levers', 0]]);
    expect(rows({ yaw_source: 'off' })[2]).toEqual(['Yaw', 'Off', 0]);
    expect(rows({ camera: { ...config.camera, lever: true } })[4]).toEqual(['Camera X', 'Hat or spare lever', -0.6]);
    expect(rows({ camera: { ...config.camera, lever: true } }, ['hat-left'])[4]).toEqual(['Camera X', 'Hat or spare lever', -1]);
    expect(rows({ camera: { ...config.camera, lever: true }, throttle_lever: 'right' })[4][2]).toBe(0.6);
    expect(app.axisValue(32768)).toBeCloseTo(0, 4);
  });
});

group('cheat sheet', () => {
  it('prints the saved profile as a light page', async () => {
    const p = await page();
    const win = { print: vi.fn() };
    p.print(win);
    expect(win.print).toHaveBeenCalled();
    const sheet = $('cheatsheet').textContent;
    expect(sheet).toContain('Profile: Cruise');
    expect(sheet).toContain('S6Flares——');
    expect(sheet).toContain('Q3Autopilotpulse on change');
    expect(sheet).toContain('HelpersLevers splitHigh-G');
    expect(sheet).not.toContain('Shift (hold');
    $('print').click(); // the button prints too (jsdom's print is a no-op)
  });

  it('covers every action, layer, helper and axis setting', () => {
    const config = defaults();
    Object.assign(config, {
      hat: 'mapped', shift: 's16', yaw_source: 'lever_differential', split_high_g: { ...config.split_high_g, enabled: false },
      long_press: { s2: { buttons: [3], pov: null } }, double_tap: { s3: { buttons: [], pov: 'left' } },
      shifted: { buttons: { s1: [9] }, pov: {} }, precision: { source: 's4', scale: 0.5 },
      combos: { pause: ['s5', 'q6'], next_profile: ['s15'] }, throttle: { ...config.throttle, mode: 'detents' },
      camera: { ...config.camera, full_after_hold: true },
    });
    config.buttons['hat-up'] = [10];
    config.buttons.s16 = [4]; // the shift control's own mapping is not printed
    config.buttons.s13 = [13];
    config.axes.roll.curve = 0.2;
    config.axes.yaw.curve = -0.3;
    const html = app.cheatSheet(config, null, '21 Oct 2026');
    document.body.innerHTML = html;
    const text = document.body.textContent;
    expect(text).toContain('Profile: Unsaved profile');
    expect(text).toContain('Shift (hold S16)');
    expect(text).toContain('S1Gun——Radar');
    expect(text).toContain('S2—Weapon select—same');
    expect(text).toContain('S3——POV leftsame');
    expect(text).toContain('Hat upGear');
    expect(text).toContain('S13NEO 13');
    expect(text).not.toContain('S16Target select');
    expect(text).toContain('StickRoll / pitch · curve 20%');
    expect(text).toContain('Lever differenceYaw · curve -30%');
    expect(text).toContain('Both leversThrottle · detents');
    expect(text).toContain('HatMapped as buttons (see above)');
    expect(text).toContain('Hold S4Precision aim · 50%Hold S16Shift layerS5 + Q6Pause / resume inputS15Next profile');
    const plain = defaults();
    plain.split_high_g.enabled = false;
    plain.yaw_source = 'off';
    plain.camera.full_after_hold = true;
    plain.throttle_lever = 'right';
    document.body.innerHTML = app.cheatSheet(plain, 'A & B', '1 Jan');
    expect(document.body.textContent).toContain('None set.');
    expect(document.body.textContent).toContain('TwistYaw off');
    expect(document.body.textContent).toContain('Right leverThrottle');
    expect(document.body.textContent).toContain('HatCamera · full after 500 ms hold');
    expect(app.cheatSheet(plain, '<x>', 'd')).toContain('&lt;x&gt;');
  });
});

group('reliability', () => {
  it('shows launch warnings once each and the reconnecting TCA pill', async () => {
    const api = fakeApi();
    const original = api.get;
    api.get = vi.fn(async (path) => (path === '/api/config' ? { ...(await original(path)), warnings: ['Steam was already running.'] } : original(path)));
    const p = await page(api);
    expect([$('warnings').hidden, $('warnings').querySelector('.warning').textContent]).toEqual([false, 'Steam was already running.']);
    expect($('warnings').querySelector('[role=alert] svg')).not.toBeNull();
    const first = $('warnings').firstChild;
    p.live({ pressed: [], status: 'Emulating', links: LINKS, warnings: ['Steam was already running.'] });
    expect($('warnings').firstChild).toBe(first); // unchanged: not re-announced
    p.live({ pressed: [], status: 'Emulating', links: { ...LINKS, tca: false } });
    expect([$('warnings').hidden, $('pill-tca').textContent, $('pill-tca').dataset.state]).toEqual([true, 'TCA: reconnecting…', 'busy']);
    expect(app.pills('Idle', null).tca).toEqual(['off', 'TCA: not open']);
    // Dismissed warnings stay hidden until they change.
    p.live({ pressed: [], status: 'Emulating', links: LINKS, warnings: ['Steam was already running.'] });
    $('warnings').querySelector('.x').click();
    p.live({ pressed: [], status: 'Emulating', links: LINKS, warnings: ['Steam was already running.'] });
    expect($('warnings').hidden).toBe(true);
    p.live({ pressed: [], status: 'Emulating', links: LINKS, warnings: ['Restart the game.'] });
    expect($('warnings').hidden).toBe(false);
  });
});

group('helper states and press timeline', () => {
  it('shows precision, shift, safe start and combo progress from the device', async () => {
    const p = await page();
    const states = () => ['precision-state', 'shift-state', 'safe-state', 'combo-state'].map((id) => [$(id).textContent, $(id).dataset.tone]);
    expect(states()).toEqual([['Precision off', ''], ['Shift off', ''], ['Starts with emulation', ''], ['No combo held', '']]);
    Object.assign(p.config, { precision: { source: 's4', scale: 0.4 }, shift: 's16', combos: { pause: ['s5', 'q6'], next_profile: ['s1'] } });
    p.live({ pressed: ['s5'], status: 'Emulating', links: LINKS, monitor: monitor({ armed: false }) });
    expect(states()).toEqual([['Ready', ''], ['Ready', ''], ['Waiting for centre', 'warn'], ['pause 1/2', 'warn']]);
    expect([...$('pause-combo').querySelectorAll('.held')].map((chip) => chip.textContent)).toEqual(['S5']);
    p.live({ pressed: ['s5', 'q6', 's4', 's16'], status: 'Emulating', links: LINKS, monitor: monitor({ precision: true, shift: true }) });
    expect(states()).toEqual([['Precision held · 40%', 'accent'], ['Shift held', 'warn'], ['Throttle armed', 'ok'], ['Pause combo held', 'ok']]);
    p.live({ pressed: ['s1'], status: 'Emulating', links: LINKS, monitor: monitor() });
    expect($('combo-state').textContent).toBe('Next-profile combo held');
    p.config.safe_start = false;
    p.renderSettings();
    expect($('safe-state').textContent).toBe('Off');
  });

  it('reads presses as taps, long presses and double taps', () => {
    const timing = { long_ms: 500, double_ms: 250, pulse_ms: 80 };
    const press = (start, end) => ({ start, end });
    expect(app.gestures([press(0, 100)], timing, 1000)).toEqual([{ kind: 'tap', from: 100, to: 180 }]);
    expect(app.gestures([press(0, 100), press(300, 380)], timing, 1000)).toEqual([{ kind: 'double', from: 300, to: 380 }]);
    expect(app.gestures([press(0, 100), press(400, 450)], timing, 1000).map((read) => read.kind)).toEqual(['tap', 'tap']);
    expect(app.gestures([press(0, 700), press(800, 850)], timing, 1000).map((read) => read.kind)).toEqual(['long', 'tap']);
    expect(app.gestures([press(0, null)], timing, 300)).toEqual([]);
    expect(app.gestures([press(0, null)], timing, 900)).toEqual([{ kind: 'long', from: 500, to: 900 }]);
  });

  it('draws the latest control’s presses against the timing draft', async () => {
    const p = await page();
    let now = 10000;
    p.clock = () => now;
    const step = (ms, pressed) => { now += ms; p.live({ pressed, status: 'Emulating', links: LINKS, monitor: monitor() }); };
    expect([$('press-key').textContent, $('press-in').children.length]).toEqual(['Pressed', 0]);
    step(0, ['s2']);
    step(200, ['s2']);
    expect($('press-text').textContent).toBe('S2 · holding 200 ms…');
    step(100, []);
    expect($('press-text').textContent).toBe('S2 · tap: 300 ms');
    expect([...$('press-out').children].map((read) => read.className)).toEqual(['out']);
    step(100, ['s2']);
    step(100, []);
    expect($('press-text').textContent).toBe('S2 · double tap: second tap within 250 ms');
    expect([...$('press-out').children].map((read) => read.className)).toEqual(['out alt']);
    step(500, ['s3']);
    step(700, ['s3']);
    expect($('press-text').textContent).toBe('S3 · long press: held 700 ms (long press from 500 ms)');
    expect([$('press-key').textContent, $('presses').getAttribute('aria-label')]).toEqual(['S3', $('press-text').textContent]);
    const [held, mark] = $('press-in').children;
    expect(mark.className).toBe('mk');
    expect([parseFloat(held.style.getPropertyValue('width')), parseFloat(mark.style.getPropertyValue('left'))].map((value) => Math.round(value * 100) / 100)).toEqual([23.33, 93.33]);
    // Old presses leave the timeline.
    step(100, []);
    step(4000, []);
    expect(p.presses).toEqual([]);
  });
});

group('head tracking', () => {
  it('reports its state and previews the camera through the draft settings', async () => {
    const api = fakeApi();
    const p = await page(api);
    expect([$('head-status').dataset.state, $('head-status-text').textContent, $('head-yaw-read').textContent]).toEqual(['off', 'Off', '—']);
    $('head-enabled').checked = true;
    fire($('head-enabled'));
    expect([p.config.head_tracking.enabled, $('head-status-text').textContent]).toEqual([true, 'Starts with emulation']);
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor() });
    expect($('head-status-text').textContent).toBe('Waiting for OpenTrack on UDP port 4242…');
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor({ head: [22.5, -15] }) });
    expect([$('head-status').dataset.state, $('head-status-text').textContent]).toEqual(['ok', 'Receiving from OpenTrack']);
    expect([$('head-yaw-read').textContent, $('head-pitch-read').textContent]).toEqual(['+23°', '−15°']);
    const [x, y] = app.headCamera([22.5, -15], p.config.head_tracking);
    expect($('head-camera-read').textContent).toBe(`${app.signed(x)} / ${app.signed(y)}`);
    expect($('head-dot').getAttribute('cx')).toBe((90 + x * 80).toFixed(1));
    expect($('head-view').getAttribute('aria-label')).toBe(`Camera at ${app.signed(x)} across, ${app.signed(y)} down`);
    p.live({ pressed: [], status: 'Emulating', links: LINKS, monitor: monitor({ head: [0, 0] }) });
    expect($('head-yaw-read').textContent).toBe('0°');
    // Inverts and settings change the preview straight away.
    for (const id of ['head-invert-yaw', 'head-invert-pitch']) {
      $(id).checked = true;
      fire($(id));
    }
    expect([p.config.head_tracking.invert_yaw, p.config.head_tracking.invert_pitch]).toEqual([true, true]);
    expect(app.headCamera([22.5, -15], p.config.head_tracking)).toEqual([-x, -y]);
    $('head-deadzone').value = '10';
    fire($('head-deadzone'));
    expect([p.config.head_tracking.tuning.deadzone, $('head-band').getAttribute('width')]).toEqual([0.1, '16.0']);
    $('head-port').value = '5000';
    fire($('head-port'));
    expect([p.config.head_tracking.port, $('head-port-step').textContent]).toEqual([5000, '5000']);
    // Validation and saving.
    $('head-yaw').value = '500';
    await p.save();
    expect($('head-error').textContent).toBe('Enter a number from 5 to 180.');
    $('head-yaw').value = '60';
    $('head-curve').value = '25';
    await p.save();
    expect(api.calls.at(-1)[2].head_tracking).toMatchObject({ enabled: true, port: 5000, yaw_range: 60, invert_yaw: true, tuning: { deadzone: 0.1, curve: 0.25 } });
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
