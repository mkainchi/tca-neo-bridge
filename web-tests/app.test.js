import { readFileSync } from 'node:fs';
import { beforeEach, describe as group, expect, it, vi } from 'vitest';
import * as app from '../daemon/web/app.js';

// Run from the repository root (see vitest.config.js).
const html = readFileSync('daemon/web/index.html', 'utf8');

const axes = (...list) => list.map(([name, max]) => ({ name, max }));
const STICK = { id: 'tca-stick', name: 'TCA Sidestick Airbus', vid: 0x044f, pid: 0x0405, layout: { buttons: 17, axes: axes(['x', 16383], ['y', 16383], ['rz', 255], ['slider', 255]), hat: true } };
const QUAD = { id: 'tca-quadrant', name: 'TCA Quadrant Eng 1&2', vid: 0x044f, pid: 0x0407, layout: { buttons: 31, axes: axes(['left', 65535], ['right', 65535]), hat: false } };
const T16 = { id: 't16000m', name: 'T.16000M', vid: 0x044f, pid: 0xb10a, layout: { buttons: 16, axes: axes(['x', 1023], ['y', 1023], ['rz', 255], ['slider', 255]), hat: true } };
const PAD = { id: '1234-0001', name: 'Pad', vid: 0x1234, pid: 1, layout: { buttons: 4, axes: axes(['z', 255]), hat: true } };
const TCA = 'tca-quadrant+tca-stick';
const s = (n) => `tca-stick/b${n}`;
const q = (n) => `tca-quadrant/b${n}`;
const bind = (device, axis, invert = false) => ({ device, axis, invert });

function tcaProfile() {
  return {
    hat: 'camera', camera: 'tca-stick',
    axes: { roll: bind('tca-stick', 'x'), pitch: bind('tca-stick', 'y'), yaw: bind('tca-stick', 'rz'), throttle: [bind('tca-stick', 'slider'), bind('tca-quadrant', 'left')] },
    split_high_g: { enabled: true, forward: 0.95, back: 0.05, levers: [bind('tca-quadrant', 'left'), bind('tca-quadrant', 'right')], reverse: false },
    buttons: { [s(1)]: [1], [s(5)]: [5], [s(6)]: [7], [q(2)]: [7] }, pov: { [s(11)]: 'up' },
  };
}

function config() {
  return { version: 2, deadzone: 0.04, camera_strength: 1, devices: [], setups: {} };
}

function devices(overrides = {}) {
  return { devices: [STICK, QUAD, PAD], active: ['tca-stick', 'tca-quadrant'], setup: TCA, profile: tcaProfile(), default: tcaProfile(), ...overrides };
}

function fakeApi(found = devices(), overrides = {}) {
  const calls = [];
  return {
    calls,
    found,
    get: vi.fn(async (path) => {
      calls.push(['GET', path]);
      if (path === '/api/config') return { config: config(), defaults: config(), status: 'Idle', links: null, profile: 'C:\\neo\\data\\config.json' };
      if (path === '/api/devices') return structuredClone(calls.found ?? found);
      if (path === '/api/labels') return { labels: [{ label: 'Old', sources: [s(3)], mapping: 'Nothing → NEO 3 (Weapon select)' }] };
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
const src = (key) => document.querySelector(`[data-source="${key}"]`);
const neo = (id) => document.querySelector(`[data-neo="${id}"]`);
const pressed = (element) => element.getAttribute('aria-pressed');
const fire = (element) => element.dispatchEvent(new Event('input', { bubbles: true }));
const set = (element, value) => { element.value = value; fire(element); };
const tick = (element, checked) => { element.checked = checked; fire(element); };
const radio = (name, value) => document.querySelector(`input[name=${name}][value=${value}]`);
const choose = (name, value, event = 'change') => { radio(name, value).checked = true; radio(name, value).dispatchEvent(new Event(event, { bubbles: true })); };
const row = (id) => document.querySelector(`[data-row="${id}"]`);
const lastPut = (api) => api.calls.filter((c) => c[0] === 'PUT' && c[1] === '/api/config').at(-1)[2];

beforeEach(() => {
  document.documentElement.innerHTML = html.replace(/^<!doctype html>/i, '');
});

group('helpers', () => {
  it('lists and names every source', () => {
    expect(app.sources([STICK, QUAD])).toHaveLength(52);
    expect(app.sources([QUAD])).toHaveLength(31);
    expect(app.parts('t16000m#2/hat-up')).toEqual(['t16000m#2', 'hat-up']);
    expect([s(5), s(17), q(3), q(12), q(20), 'tca-stick/hat-down'].map((key) => app.label(key))).toEqual([
      'Sidestick button 5', 'Sidestick input 17', 'Quadrant button 3', 'Quadrant detent state 12', 'Quadrant add-on input 20', 'Sidestick hat down',
    ]);
    const names = { t16000m: 'T.16000M' };
    expect(['t16000m/b3', 't16000m/hat-left', 'twcs/b2', 'twcs/hat-up'].map((key) => app.label(key, names))).toEqual(['T.16000M button 3', 'T.16000M hat left', 'twcs button 2', 'twcs hat up']);
    expect([s(6), q(12), 'tca-stick/hat-up', 'twcs/b3', 'twcs/hat-up'].map(app.short)).toEqual(['S6', 'Q12', 'Hat up', 'twcs 3', 'twcs hat up']);
  });

  it('names NEO controls and describes mappings', () => {
    expect(app.neoName('b5')).toBe('NEO 5 (High-G)');
    expect(app.neoName('b13')).toBe('NEO 13');
    expect(app.neoName('pov-left')).toBe('NEO POV left');
    expect(app.describe({ buttons: [], pov: null })).toBe('Nothing');
    expect(app.describe({ buttons: [1, 13], pov: 'up' })).toBe('NEO 1 (Gun), NEO 13, NEO POV up');
  });

  it('reads current mappings, compares them and finds sources by NEO control', () => {
    const profile = tcaProfile();
    profile.buttons[s(9)] = [4, 2];
    const keys = app.sources([STICK, QUAD]);
    expect(app.current(profile, s(9))).toEqual({ buttons: [2, 4], pov: null });
    expect(app.current(profile, s(11))).toEqual({ buttons: [], pov: 'up' });
    expect(app.same({ buttons: [1], pov: null }, { buttons: [1], pov: null })).toBe(true);
    expect(app.same({ buttons: [1], pov: null }, { buttons: [1], pov: 'up' })).toBe(false);
    expect(app.same({ buttons: [1], pov: null }, { buttons: [2], pov: null })).toBe(false);
    expect(app.sourcesFor(profile, 'b7', keys)).toEqual([s(6), q(2)]);
    expect(app.sourcesFor(profile, 'pov-up', keys)).toEqual([s(11)]);
  });

  it('lists changes, finds the camera hat and names bindings and levers', () => {
    expect(app.diff({ buttons: [1, 2], pov: 'up' }, { buttons: [2, 3], pov: 'down' })).toEqual([
      ['add', 'Adds NEO 3 (Weapon select)'], ['remove', 'Removes NEO 1 (Gun)'], ['add', 'Adds NEO POV down'], ['remove', 'Removes NEO POV up'],
    ]);
    expect(app.diff({ buttons: [], pov: null }, { buttons: [], pov: null })).toEqual([]);
    expect(app.diff({ buttons: [], pov: 'up' }, { buttons: [], pov: null })).toEqual([['remove', 'Removes NEO POV up']]);
    expect(app.cameraDevice({ camera: 't16000m' }, [STICK, T16])).toBe('t16000m');
    expect(app.cameraDevice({ camera: 'tca-quadrant' }, [QUAD, T16, STICK])).toBe('t16000m');
    expect(app.cameraDevice({ camera: null }, [QUAD])).toBeNull();
    expect(app.bindingName(bind('t16000m', 'x', true), { t16000m: 'T.16000M' })).toBe('T.16000M x (inverted)');
    expect(app.bindingName(bind('twcs', 'z'))).toBe('twcs z');
    expect([bind('tca-quadrant', 'left', true), bind('tca-quadrant', 'right'), bind('tca-quadrant', 'z'), bind('twcs', 'rz', true)].map((b) => app.leverName(b))).toEqual(['Left lever', 'Right lever', 'tca-quadrant z', 'twcs rz']);
    expect(app.splitActive([0.8, 0.1], { enabled: true, forward: 0.75, back: 0.25 })).toBe(true);
    expect(app.splitActive([0.8, 0.3], { enabled: true, forward: 0.75, back: 0.25 })).toBe(false);
  });

  it('finds the axis that moved past half travel', () => {
    const baseline = { 'tca-stick': [0, 0, 0, 0] };
    expect(app.moved(baseline, { 'tca-stick': [0, 0.4, 0, 0] }, [STICK, QUAD])).toBeNull();
    expect(app.moved(baseline, { 'tca-stick': [0, 0, 0.9, 0], 'tca-quadrant': [1, 1] }, [STICK, QUAD])).toEqual(bind('tca-stick', 'rz'));
    expect(app.moved({}, { 'tca-quadrant': [1, 1] }, [STICK, QUAD])).toBeNull(); // no baseline for that device yet
  });

  it('turns the daemon status and session links into header pills', () => {
    const full = { source: 'hotas', hidden: true, inputs: 2, virtual_neo: true };
    expect(app.pills('Emulating', full)).toEqual({ status: 'ok', inputs: ['ok', 'Controllers: 2 open'], hidhide: ['ok', 'HidHide: controllers hidden'], neo: ['ok', 'Virtual NEO present'] });
    expect(app.pills('Emulating', { source: 'validation', hidden: false, inputs: 0, virtual_neo: true })).toEqual({ status: 'ok', inputs: ['busy', 'Source: validation stream'], hidhide: ['off', 'HidHide: not needed'], neo: ['ok', 'Virtual NEO present'] });
    expect(app.pills('Idle', null)).toEqual({ status: 'off', inputs: ['off', 'Controllers: not open'], hidhide: ['off', 'HidHide: off'], neo: ['off', 'Virtual NEO: absent'] });
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
  });
});

group('mapping tab', () => {
  it('builds TCA diagrams, chips, NEO hotspots, the NEO table and the axes', async () => {
    await page();
    expect(document.querySelectorAll('[data-source]')).toHaveLength(53);
    expect(document.querySelectorAll(`[data-source="${q(3)}"]`)).toHaveLength(2);
    expect(src(q(12)).getAttribute('aria-label')).toBe('Quadrant detent state 12');
    expect(src(s(5)).style.getPropertyValue('--x')).toBe('7.179%');
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
    expect($('axes').textContent).toContain('XTCA Sidestick Airbus xRoll');
    expect($('axes').textContent).toContain('ZTCA Sidestick Airbus slider or TCA Quadrant Eng 1&2 leftThrottle');
    expect($('axes').textContent).toContain('Rx / RyTCA Sidestick Airbus hatCamera');
    expect($('profile-path').textContent).toBe('C:\\neo\\data\\config.json');
    expect($('setup-name').textContent).toBe('TCA Sidestick Airbus + TCA Quadrant Eng 1&2');
    expect(document.querySelector('[data-device="tca-quadrant"] .device-name').textContent).toBe('TCA Quadrant Eng 1&2');
    expect($('quadrant-rows').hidden).toBe(false);
    expect($('sources-empty').hidden).toBe(true);
    expect($('camera-device').disabled).toBe(true); // a single hat
    expect($('selected-name').textContent).toBe('Nothing selected');
    expect($('compare').hidden).toBe(true);
    expect($('deselect').disabled).toBe(true);
    expect($('recorded-count').textContent).toBe('1');
    expect($('recorded-empty').hidden).toBe(true);
    expect($('recorded').textContent).toContain('Old→ NEO 3 (Weapon select)was Nothing');
    expect($('pill-inputs').textContent).toBe('Controllers: not open');
    choose('hand', 'left');
    expect($('stick-img').getAttribute('src')).toBe('device/sidestick-left.svg');
    expect($('hand-caption').textContent).toBe('Left-hand setup');
    expect(src(s(5)).style.getPropertyValue('--x')).toBe('92.308%');
    expect(document.querySelectorAll('#stick-spots [data-source]')).toHaveLength(17);
    choose('hand', 'right');
    expect($('stick-img').alt).toBe('TCA Sidestick, top view, right-hand setup, numbered controls 1 to 17');
  });

  it('builds generated panels for other controllers and handles none at all', async () => {
    const profile = { hat: 'camera', camera: null, axes: { roll: null, pitch: null, yaw: null, throttle: [] }, split_high_g: { enabled: false, forward: 0.75, back: 0.25, levers: [], reverse: false }, buttons: { 't16000m/b2': [2], '1234-0001/hat-up': [9] }, pov: {} };
    const api = fakeApi(devices({ devices: [T16, PAD], active: ['t16000m', '1234-0001'], setup: '1234-0001+t16000m', profile, default: profile }));
    await page(api);
    expect(document.querySelector('figure.generic .device-name').textContent).toBe('T.16000M');
    expect(document.querySelector('figure.generic figcaption').textContent).toBe('T.16000M16 buttons · 4 axes');
    expect(src('t16000m/b16').textContent).toBe('16');
    expect(src('t16000m/b2').getAttribute('aria-label')).toBe('T.16000M button 2');
    expect(document.querySelectorAll('#hat-rows .chip-row')).toHaveLength(2);
    expect($('hat-rows').textContent).toContain('Pad hat');
    expect($('quadrant-rows').hidden).toBe(true);
    expect($('stick-spots')).toBeNull();
    // Two hats: the camera follows the first unless chosen; the other hat is mappable meanwhile.
    expect($('camera-device').disabled).toBe(false);
    expect($('camera-device').value).toBe('t16000m');
    expect(src('t16000m/hat-up').disabled).toBe(true);
    expect(src('1234-0001/hat-up').disabled).toBe(false);
    expect(row('b9').querySelector('.fed').textContent).toBe('1234-0001 hat up');
    expect(row('b2').querySelector('.fed').textContent).toBe('t16000m 2');
    expect($('axes').textContent).toContain('XNo sourceRoll');
    expect($('axes').textContent).toContain('ZNo sourceThrottle');
    expect($('axes').textContent).toContain('Rx / RyT.16000M hatCamera');
    $('camera-device').value = '1234-0001';
    $('camera-device').dispatchEvent(new Event('change'));
    await vi.waitFor(() => expect($('link-status').textContent).toBe('Pad hat drives the camera; its mappings are kept but inactive.'));
    expect(lastPut(api).setups['1234-0001+t16000m'].camera).toBe('1234-0001');
    expect(src('t16000m/hat-up').disabled).toBe(false);
    // No controllers.
    document.documentElement.innerHTML = html.replace(/^<!doctype html>/i, '');
    const empty = { ...profile, buttons: {} };
    await page(fakeApi(devices({ devices: [], active: [], setup: '', profile: empty, default: empty })));
    expect($('sources-empty').hidden).toBe(false);
    expect($('setup-name').textContent).toBe('no controllers');
    expect($('hat-hint').textContent).toBe('No controller with a hat: the camera mini-stick rests.');
    expect($('axes').textContent).toContain('Rx / RyNo hatCamera');
  });

  it('loads without a profile path and keeps the placeholder', async () => {
    const api = fakeApi();
    const original = api.get;
    api.get = vi.fn(async (path) => (path === '/api/config' ? { config: config(), defaults: config(), status: 'Idle' } : original(path)));
    await page(api);
    expect($('profile-path').textContent).toBe('config.json');
  });

  it('selecting a control lights its NEO mapping; NEO clicks build a new one', async () => {
    const p = await page();
    src(s(6)).click();
    expect(pressed(src(s(6)))).toBe('true');
    expect($('selected-name').textContent).toBe('Sidestick button 6');
    expect($('mapped-now').textContent).toBe('NEO 7 (Flares)');
    expect($('mapped-new').textContent).toBe('Unchanged');
    expect($('selected-kind').textContent).toBe('Your control');
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

  it('with nothing selected, a NEO click shows which controls feed it', async () => {
    await page();
    neo('b7').click();
    expect($('selected-name').textContent).toBe('NEO 7 (Flares)');
    expect($('link-heading').textContent).toBe('Selected NEO control');
    expect($('mapped-now-label').textContent).toBe('Fed by');
    expect($('mapped-new').hidden).toBe(true);
    expect($('mapped-now').textContent).toBe('Sidestick button 6, Quadrant button 2');
    expect($('neo-context-text').textContent).toBe('Inspecting NEO 7 (Flares) · fed by S6 · Q2');
    expect(src(s(6)).classList.contains('is-feed')).toBe(true);
    expect(pressed(neo('b7'))).toBe('true');
    expect(row('b7').classList.contains('is-sel')).toBe(true);
    expect($('deselect').disabled).toBe(false);
    row('b14').click();
    expect($('mapped-now').textContent).toBe('No control');
    expect($('neo-context-text').textContent).toBe('Inspecting NEO 14 · fed by nothing');
    neo('pov-up').click();
    expect(row('pov').classList.contains('is-sel')).toBe(true);
    neo('b14').click();
    neo('b14').click();
    expect($('selected-name').textContent).toBe('Nothing selected');
  });

  it('records corrections to the active set and the log', async () => {
    const api = fakeApi();
    const p = await page(api);
    src(s(6)).click();
    neo('b7').click();
    neo('b6').click();
    $('record').click();
    await vi.waitFor(() => expect($('link-status').textContent).toBe('Recorded Sidestick button 6: NEO 6 (Autopilot).'));
    const saved = lastPut(api);
    expect(saved.setups[TCA].buttons[s(6)]).toEqual([6]);
    expect(saved.deadzone).toBe(0.04);
    expect(api.calls.at(-1)[2].at(-1)).toEqual({ label: 'Sidestick button 6', sources: [s(6)], mapping: 'NEO 7 (Flares) → NEO 6 (Autopilot)' });
    expect(p.full.setups[TCA].buttons[s(6)]).toEqual([6]);
    expect($('mapped-now').textContent).toBe('NEO 6 (Autopilot)');
    expect($('recorded-count').textContent).toBe('2');
    // Clearing a POV removes it; choosing one stores it.
    src(s(11)).click();
    neo('pov-up').click();
    api.put = vi.fn(async (path) => (path === '/api/labels' ? { saved: 1 } : { saved: true, running: true }));
    await p.record();
    expect(s(11) in p.saved.pov).toBe(false);
    expect($('link-status').textContent).toBe('Recorded Sidestick button 11: Nothing. Applied to the running emulation.');
    src(s(12)).click();
    neo('pov-right').click();
    await p.record();
    expect(p.saved.pov[s(12)]).toBe('right');
  });

  it('keeps the proposal when saving fails and reports a failed log update', async () => {
    const api = fakeApi(devices(), { put: vi.fn(async () => { throw new Error('The profile could not be saved.'); }) });
    const p = await page(api);
    src(s(5)).click();
    neo('b9').click();
    await p.record();
    expect($('link-status').textContent).toBe('Not saved: The profile could not be saved.');
    expect(p.proposal.buttons).toEqual([5, 9]);
    expect($('record').disabled).toBe(false);
    api.put = vi.fn(async (path) => { if (path === '/api/labels') throw new Error('The recordings could not be saved.'); return { saved: true, running: false }; });
    await p.record();
    expect($('link-status').textContent).toBe('Mapping saved, but the correction log was not updated: The recordings could not be saved..');
    expect(p.saved.buttons[s(5)]).toEqual([5, 9]);
  });

  it('removes log entries and reports failures', async () => {
    const api = fakeApi();
    const p = await page(api);
    $('recorded').querySelector('button').click();
    await vi.waitFor(() => expect($('link-status').textContent).toBe('Removed from the correction log. The mapping itself is unchanged.'));
    expect(p.labels).toEqual([]);
    p.labels = [{ label: 'X', sources: [s(1)], mapping: 'a' }];
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

group('camera hat', () => {
  it('is disabled while it drives the camera and mappable in Mapped mode', async () => {
    const api = fakeApi();
    const p = await page(api);
    const up = 'tca-stick/hat-up';
    expect(src(up).textContent).toBe('Up');
    expect(src(up).disabled).toBe(true);
    expect(src(up).title).toBe('Set the camera hat to Mapped controls to map it');
    expect(radio('hat', 'camera').checked).toBe(true);
    p.live({ pressed: [up], status: 'Emulating' });
    expect(p.selected).toBeNull();
    choose('hat', 'mapped');
    await vi.waitFor(() => expect($('link-status').textContent).toBe('TCA Sidestick Airbus hat now sends its mapped controls; the camera mini-stick gets no input.'));
    expect(lastPut(api).setups[TCA].hat).toBe('mapped');
    expect(src(up).disabled).toBe(false);
    expect(src(up).title).toBe('Sidestick hat up');
    expect($('hat-hint').textContent).toBe('Each direction maps like a button; the camera gets no input.');
    expect($('axes').textContent).toContain('No source (hat is mapped)Camera');
    p.live({ pressed: [], status: 'Emulating' });
    p.live({ pressed: [up], status: 'Emulating' });
    expect(p.selected).toBe(up);
    neo('pov-up').click();
    await p.record();
    expect(p.saved.pov[up]).toBe('up');
    neo('pov-up').click();
    expect($('mapped-now').textContent).toBe('NEO POV up');
    // Reverse lookup includes hat sources.
    $('deselect').click();
    neo('pov-up').click();
    expect($('mapped-now').textContent).toBe('Sidestick button 11, Sidestick hat up');
    expect(row('pov').querySelector('.fed').textContent).toBe('U: S11 · Hat up');
    // Back to camera with a hat direction selected: the selection is cleared.
    src(up).click();
    api.put = vi.fn(async () => ({ saved: true, running: true }));
    await p.commitProfile({ hat: 'camera' });
    expect(p.selected).toBeNull();
    expect($('link-status').textContent).toBe('TCA Sidestick Airbus hat drives the camera; its mappings are kept but inactive. Applied to the running emulation.');
    expect(p.saved.pov[up]).toBe('up');
    // Switching with a button selected keeps it.
    src(s(2)).click();
    await p.commitProfile({ hat: 'mapped' });
    expect(p.selected).toBe(s(2));
  });

  it('names no hat when the set has none, and reports a failed change', async () => {
    const profile = { ...tcaProfile(), camera: null };
    const p = await page(fakeApi(devices({ devices: [QUAD], active: ['tca-quadrant'], setup: 'tca-quadrant', profile, default: profile })));
    await p.commitProfile({ hat: 'mapped' });
    expect($('link-status').textContent).toBe('The hat now sends its mapped controls; the camera mini-stick gets no input.');
    document.documentElement.innerHTML = html.replace(/^<!doctype html>/i, '');
    const failing = await page(fakeApi(devices(), { put: vi.fn(async () => { throw new Error('The profile could not be saved.'); }) }));
    await failing.commitProfile({ hat: 'mapped' });
    expect($('link-status').textContent).toBe('Not saved: The profile could not be saved.');
    expect(radio('hat', 'camera').checked).toBe(true);
    expect(failing.busy).toBe(false);
  });
});

group('live input', () => {
  it('lights held controls and the hat, and selects new presses on the mapping tab', async () => {
    const p = await page();
    p.live({ pressed: [], status: 'Idle' });
    expect($('live-input').textContent).toBe('Start emulation to see presses.');
    const links = { source: 'hotas', hidden: true, inputs: 2, virtual_neo: true };
    p.live({ pressed: [], status: 'Emulating', links });
    expect($('live-input').textContent).toBe('Nothing pressed');
    expect($('live-input').dataset.active).toBe('false');
    expect($('daemon-status').textContent).toBe('Daemon: Emulating');
    expect($('pill-status').dataset.state).toBe('ok');
    expect($('pill-hidhide').textContent).toBe('HidHide: controllers hidden');
    p.live({ pressed: [s(5), 'tca-stick/hat-up', 'tca-stick/hat-right'], status: 'Emulating', links });
    expect($('live-input').textContent).toBe('Now sending: Sidestick button 5, Sidestick hat up, Sidestick hat right');
    expect($('live-input').dataset.active).toBe('true');
    expect(src(s(5)).classList.contains('held')).toBe(true);
    expect(src('tca-stick/hat-up').classList.contains('held')).toBe(true);
    expect(src('tca-stick/hat-down').classList.contains('held')).toBe(false);
    expect(p.selected).toBe(s(5));
    p.live({ pressed: [s(5), 'tca-stick/hat-up'], status: 'Emulating' }); // held: no reselection
    src(q(1)).click();
    p.live({ pressed: [s(5), 'other/b1'], status: 'Emulating' }); // sources of absent devices are never selected
    expect(p.selected).toBe(q(1));
    p.busy = true;
    p.live({ pressed: [s(5), s(2)], status: 'Emulating' });
    expect(p.selected).toBe(q(1));
    p.busy = false;
    $('tab-settings').click();
    p.live({ pressed: [s(3)], status: 'Emulating' });
    expect(p.selected).toBe(q(1));
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
    expect(app.leverText([0.97, 0.02], rule)).toBe('First lever 97% · Second lever 2% · High-G active');
    expect(app.leverText([0.02, 0.97], rule, ['Left lever', 'Right lever'])).toBe('Left lever 2% · Right lever 97% · High-G active');
    expect(app.leverText([0.5, 0.97], rule)).toBe('First lever 50% · Second lever 97%');
    expect(app.leverText([0.97, 0.02], { ...rule, enabled: false })).toBe('First lever 97% · Second lever 2%');
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
    expect($('lever-name-0').textContent).toBe('Left lever');
    expect($('lever-rails').hidden).toBe(false);
    expect($('lever-rails').style.getPropertyValue('--fw')).toBe('95%');
    expect($('needle-0').style.getPropertyValue('--pos')).toBe('100%');
    expect([$('value-0').dataset.zone, $('value-1').dataset.zone]).toEqual(['forward', 'back']);
    expect($('lever-state').textContent).toBe('High-G active · NEO 5 held');
    expect($('lever-state').dataset.active).toBe('true');
    p.live({ pressed: [], levers: [0.5, 0.5], status: 'Emulating' });
    expect($('value-0').dataset.zone).toBe('middle');
    expect($('lever-state').textContent).toBe('Not split');
    p.saved.split_high_g = { ...p.saved.split_high_g, enabled: false, levers: [] };
    p.live({ pressed: [], levers: null, status: 'Idle' });
    expect($('lever-live').textContent).toBe('Lever positions appear here during emulation.');
    expect($('lever-state').textContent).toBe('Rule off');
    expect($('lever-name-1').textContent).toBe('Second lever');
  });

  it('validates and saves the thresholds, levers and the switch', async () => {
    const api = fakeApi();
    const p = await page(api);
    expect([$('lever-0').value, $('lever-1').value]).toEqual(['tca-quadrant|left', 'tca-quadrant|right']);
    tick($('split-enabled'), false);
    tick($('split-reverse'), true);
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
    // Lever inversion follows the matching throttle axis (none for the right lever).
    const throttleInverted = structuredClone(p.config.axes);
    expect(lastPut(api).setups[TCA].split_high_g).toEqual({ enabled: false, forward: 0.8, back: 0.1, reverse: true, levers: [bind('tca-quadrant', 'left'), bind('tca-quadrant', 'right')] });
    expect(throttleInverted.throttle).toHaveLength(2);
    expect(p.saved.split_high_g.forward).toBe(0.8);
    expect($('lever-state').textContent).toBe('Rule off');
    // Enabled with a lever missing: refused; unset levers save as none while the rule is off.
    tick($('split-enabled'), true);
    set($('lever-1'), '');
    await p.save();
    expect($('dirty').textContent).toBe('Pick both levers for split High-G, or turn the rule off.');
    tick($('split-enabled'), false);
    await p.save();
    expect(lastPut(api).setups[TCA].split_high_g.levers).toEqual([]);
    set($('lever-1'), 'tca-stick|slider');
    tick($('invert-yaw'), true);
    p.config.axes.throttle[0].invert = true;
    tick($('split-enabled'), true);
    await p.save();
    expect(lastPut(api).setups[TCA].split_high_g.levers).toEqual([bind('tca-quadrant', 'left'), bind('tca-stick', 'slider', true)]);
  });
});

group('axis sources', () => {
  it('binds, inverts, adds and removes axes', async () => {
    const api = fakeApi();
    const p = await page(api);
    expect($('bind-roll').value).toBe('tca-stick|x');
    expect($('bind-roll').options).toHaveLength(7); // none + 4 stick + 2 quadrant axes
    expect($('bind-roll').options[0].textContent).toBe('No source (NEO X rests)');
    expect($('throttle-list').textContent).toBe('TCA Sidestick Airbus sliderInvertRemoveTCA Quadrant Eng 1&2 leftInvertRemove');
    set($('bind-pitch'), '');
    expect($('invert-pitch').disabled).toBe(true);
    set($('bind-yaw'), 'tca-quadrant|right');
    tick($('invert-yaw'), true);
    tick($('invert-roll'), true);
    set($('throttle-add'), 'tca-quadrant|right');
    set($('throttle-add'), 'tca-quadrant|right'); // already there
    set($('throttle-add'), '');
    expect($('throttle-add').value).toBe('');
    tick(document.querySelector('[data-throttle="2"]'), true);
    document.querySelector('[data-remove="0"]').click();
    expect($('dirty').textContent).toBe('Unsaved changes.');
    await p.save();
    const axesSaved = lastPut(api).setups[TCA].axes;
    expect(axesSaved).toEqual({ roll: bind('tca-stick', 'x', true), pitch: null, yaw: bind('tca-quadrant', 'right', true), throttle: [bind('tca-quadrant', 'left'), bind('tca-quadrant', 'right', true)] });
    expect($('axes').textContent).toContain('YNo sourcePitch');
    expect($('axes').textContent).toContain('ZTCA Quadrant Eng 1&2 left or TCA Quadrant Eng 1&2 right (inverted)Throttle');
  });

  it('detects the axis moved during emulation', async () => {
    const p = await page();
    document.querySelector('[data-detect="roll"]').click();
    expect(document.querySelector('[data-detect="roll"]').getAttribute('aria-busy')).toBe('true');
    expect($('detect-hint').textContent).toBe('Move the axis you want, all the way, during emulation…');
    p.live({ pressed: [], status: 'Idle' });
    expect($('detect-hint').textContent).toBe('Start emulation from the tray icon, then move the axis.');
    const rest = { 'tca-stick': [0, 0, 0, -1], 'tca-quadrant': [1, 1] };
    p.live({ pressed: [], status: 'Emulating', axes: rest });
    p.live({ pressed: [], status: 'Emulating', axes: rest });
    expect(p.detecting).toBe('roll');
    p.live({ pressed: [], status: 'Emulating', axes: { ...rest, 'tca-stick': [0, 0, 0.9, -1] } });
    expect($('bind-roll').value).toBe('tca-stick|rz');
    expect($('detect-hint').textContent).toBe('Detected TCA Sidestick Airbus rz. Save settings to apply.');
    expect(p.detecting).toBeNull();
    document.querySelector('[data-detect="throttle"]').click();
    p.live({ pressed: [], status: 'Emulating', axes: rest });
    p.live({ pressed: [], status: 'Emulating', axes: { ...rest, 'tca-quadrant': [-1, 1] } });
    expect(p.config.axes.throttle).toHaveLength(2); // the quadrant's left lever is already a throttle
    document.querySelector('[data-detect="lever-1"]').click();
    p.live({ pressed: [], status: 'Emulating', axes: rest });
    p.live({ pressed: [], status: 'Emulating', axes: { ...rest, 'tca-stick': [0, 0, 0, 0] } });
    expect($('lever-1').value).toBe('tca-stick|slider');
    expect($('dirty').dataset.state).toBe('dirty');
    // Clicks elsewhere in the form do nothing.
    $('split-heading').click();
    expect(p.detecting).toBeNull();
  });
});

group('controllers', () => {
  it('switches between automatic and explicit selection and reloads the set', async () => {
    const api = fakeApi();
    const p = await page(api);
    const box = (id) => document.querySelector(`#device-list [data-device="${id}"]`);
    expect($('devices-auto').checked).toBe(true);
    expect([box('tca-stick').checked, box('tca-quadrant').checked, box('1234-0001').checked]).toEqual([true, true, false]);
    expect(box('1234-0001').parentElement.textContent).toBe('Pad (1234-0001)');
    tick(box('1234-0001'), true);
    tick(box('tca-quadrant'), false);
    expect(p.config.devices).toEqual(['tca-stick', '1234-0001']);
    expect($('devices-auto').checked).toBe(false);
    tick($('devices-auto'), true);
    expect(p.config.devices).toEqual([]);
    tick($('devices-auto'), false);
    expect(p.config.devices).toEqual(['tca-stick', 'tca-quadrant']);
    tick(box('tca-quadrant'), false);
    // Saving a new list reloads the controllers: the page shows the new set's mapping.
    const profile = { ...tcaProfile(), buttons: {}, pov: {} };
    api.found = devices({ devices: [STICK, QUAD, PAD], active: ['tca-stick'], setup: 'tca-stick', profile, default: profile });
    api.calls.found = api.found;
    await p.save();
    expect(lastPut(api).devices).toEqual(['tca-stick']);
    expect(lastPut(api).setups[TCA]).toBeDefined();
    expect(p.setup).toBe('tca-stick');
    expect($('quadrant-rows').hidden).toBe(true);
    expect(row('b7').querySelector('.fed').textContent).toBe('—');
    // Saving the same list does not reload.
    const before = api.calls.filter((c) => c[1] === '/api/devices').length;
    await p.save();
    expect(api.calls.filter((c) => c[1] === '/api/devices').length).toBe(before);
  });

  it('saves global settings without a controller set', async () => {
    const profile = { ...tcaProfile(), buttons: {}, pov: {}, split_high_g: { enabled: false, forward: 0.75, back: 0.25, levers: [], reverse: false } };
    const api = fakeApi(devices({ devices: [], active: [], setup: '', profile, default: profile }));
    const p = await page(api);
    await p.save();
    expect(lastPut(api).setups).toEqual({});
    expect(p.saved.split_high_g.levers).toEqual([]);
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
    expect($('deadzone-range').value).toBe('4');
    expect($('deadzone-text').textContent).toBe('Ignored band ±4%');
    $('deadzone-range').value = '12';
    fire($('deadzone-range'));
    expect($('deadzone').value).toBe('0.12');
    expect($('deadzone-band').style.getPropertyValue('--band')).toBe('12%');
    $('camera').value = '0.5';
    fire($('camera'));
    expect($('camera-range').value).toBe('50');
    $('split-forward-range').value = '70';
    fire($('split-forward-range'));
    expect($('split-forward').value).toBe('70');
    $('split-back').value = '20';
    fire($('split-back'));
    expect($('split-back-range').value).toBe('20');
    $('deadzone').value = 'x';
    fire($('deadzone'));
    expect($('deadzone-text').textContent).toBe('Ignored band ±0%');
    expect($('dirty').textContent).toBe('Unsaved changes.');
    $('deadzone').value = '1';
    $('camera').value = '';
    await p.save();
    expect($('deadzone-error').textContent).toBe('Enter a number from 0 to below 1.');
    expect($('camera-error').textContent).toBe('Enter a number from 0 to 1.');
    expect($('dirty').textContent).toBe('Fix the highlighted fields, then save.');
    expect($('dirty').dataset.state).toBe('error');
    $('deadzone').value = '-1';
    $('camera').value = '0.5';
    await p.save();
    $('deadzone').value = '0.1';
    $('camera').value = '2';
    await p.save();
    expect(api.put).not.toHaveBeenCalled();
    $('camera').value = '1';
    await p.save();
    expect(lastPut(api)).toMatchObject({ deadzone: 0.1, camera_strength: 1, devices: [] });
    expect($('dirty').textContent).toBe('Saved.');
    expect($('dirty').dataset.state).toBe('saved');
    api.put = vi.fn(async () => ({ saved: true, running: true }));
    $('profile').dispatchEvent(new Event('submit', { cancelable: true }));
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Saved. Applied to the running emulation.'));
    api.put = vi.fn(async () => { throw new Error('Deadzone must be at least 0 and below 1.'); });
    await p.save();
    expect($('dirty').textContent).toBe('Not saved: Deadzone must be at least 0 and below 1.');
    expect($('save').disabled).toBe(false);
  });

  it('resets every setting and this set’s mapping to the defaults', async () => {
    const api = fakeApi(devices({ default: { ...tcaProfile(), buttons: { [s(2)]: [2] }, pov: {}, hat: 'mapped' } }));
    const p = await page(api);
    set($('bind-roll'), '');
    $('reset').click();
    expect($('bind-roll').value).toBe('tca-stick|x');
    expect($('devices-auto').checked).toBe(true);
    expect($('dirty').dataset.state).toBe('dirty');
    expect($('dirty').textContent).toBe('Unsaved: defaults, including every mapping of these controllers. Save settings to apply.');
    await p.save();
    const saved = lastPut(api).setups[TCA];
    expect([saved.buttons, saved.hat]).toEqual([{ [s(2)]: [2] }, 'mapped']);
    expect(p.resetMapping).toBe(false);
  });
});

group('start', () => {
  function windowLike(fetchImpl) {
    return { document, fetch: fetchImpl, location: { hash: '#tok' }, setInterval: vi.fn((fn) => { fn(); return 7; }) };
  }

  it('loads with the fragment token and starts polling', async () => {
    const fetchImpl = vi.fn(async (path) => ({ ok: true, json: async () => {
      if (path === '/api/config') return { config: config(), defaults: config(), status: 'Idle' };
      if (path === '/api/devices') return devices();
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
