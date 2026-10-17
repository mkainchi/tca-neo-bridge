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
      if (path === '/api/config') return { config: defaults(), defaults: defaults(), status: 'Idle', links: null, profile: 'C:\\neo\\data\\config.json' };
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
    expect(app.current(config, 's9')).toEqual({ buttons: [2, 4], pov: null });
    expect(app.current(config, 's11')).toEqual({ buttons: [], pov: 'up' });
    expect(app.same({ buttons: [1], pov: null }, { buttons: [1], pov: null })).toBe(true);
    expect(app.same({ buttons: [1], pov: null }, { buttons: [1], pov: 'up' })).toBe(false);
    expect(app.same({ buttons: [1], pov: null }, { buttons: [2], pov: null })).toBe(false);
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
    expect(api.calls.at(-1)[2]).toMatchObject({ deadzone: 0.1, camera_strength: 1, throttle_lever: 'right', invert: { yaw: true } });
    expect($('dirty').textContent).toBe('Saved.');
    expect($('dirty').dataset.state).toBe('saved');
    expect(p.saved.throttle_lever).toBe('right');
    api.put = vi.fn(async () => ({ saved: true, running: true }));
    $('profile').dispatchEvent(new Event('submit', { cancelable: true }));
    await vi.waitFor(() => expect($('dirty').textContent).toBe('Saved. Applied to the running emulation.'));
    api.put = vi.fn(async () => { throw new Error('Deadzone must be at least 0 and below 1.'); });
    await p.save();
    expect($('dirty').textContent).toBe('Not saved: Deadzone must be at least 0 and below 1.');
    expect($('save').disabled).toBe(false);
    $('reset').click();
    expect(radio('lever', 'left').checked).toBe(true);
    expect($('dirty').dataset.state).toBe('dirty');
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
