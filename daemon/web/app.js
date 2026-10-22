// Any-TM-HOTAS bridge configuration page. The token comes from the URL fragment, so it never appears in request lines.

/** AC8 actions for native NEO buttons (official AC8 PC mapping); unlisted buttons have no documented action. */
export const NEO_ACTIONS = { 1: 'Gun', 2: 'Missile', 3: 'Weapon select', 4: 'Target select', 5: 'High-G', 6: 'Autopilot', 7: 'Flares', 8: 'View', 9: 'Radar', 10: 'Gear', 12: 'Pause / menu' };

export const STICK = 'tca-stick';
export const QUADRANT = 'tca-quadrant';
export const HAT_DIRECTIONS = ['up', 'right', 'down', 'left'];
export const NEO_AXES = [['roll', 'X', 'Roll'], ['pitch', 'Y', 'Pitch'], ['yaw', 'Rz', 'Yaw']];

/**
 * Diagram hotspots: [key, x, y, diameter] in each SVG's viewBox units. They sit on the numbered callouts.
 * Sidestick: viewBox -76 -8 390 340 (left-hand setup mirrors 5–10 and 11–16). Quadrant: 533 × 364 (two
 * callouts read 3). NEO: 606 × 726; button 16 exists physically but not in the NEO report.
 */
export const DIAGRAMS = {
  right: { box: [-76, -8, 390, 340], spots: [[5, -48, 125], [6, -48, 153], [7, -48, 181], [10, -48, 209], [9, -48, 237], [8, -48, 265], [11, 284, 125], [12, 284, 153], [13, 284, 181], [16, 284, 209], [15, 284, 237], [14, 284, 265], [1, -48, 79], [2, -48, 43], [3, -13, 98], [4, 244, 99], [17, 260, 303]].map(([n, x, y]) => [`${STICK}/b${n}`, x, y, 27]) },
  left: { box: [-76, -8, 390, 340], spots: [[11, -48, 125], [12, -48, 153], [13, -48, 181], [16, -48, 209], [15, -48, 237], [14, -48, 265], [5, 284, 125], [6, 284, 153], [7, 284, 181], [10, 284, 209], [9, 284, 237], [8, 284, 265], [1, -48, 79], [2, -48, 43], [3, -13, 98], [4, 244, 99], [17, 260, 303]].map(([n, x, y]) => [`${STICK}/b${n}`, x, y, 27]) },
  quadrant: { box: [0, 0, 533, 364], spots: [[1, 79, 49, 30], [2, 447, 65, 30], [3, 473, 153, 30], [3, 67, 179, 30], [4, 470, 204, 30], [5, 58, 218, 30], [6, 274, 327, 30], [7, 64, 308, 22], [8, 86, 308, 22]].map(([n, x, y, d]) => [`${QUADRANT}/b${n}`, x, y, d]) },
  neo: { box: [0, 0, 606, 726], spots: [[1, 546, 159], [2, 278, 185], [3, 427, 126], [4, 320, 165], [5, 262, 321], [6, 262, 362], [7, 262, 402], [8, 262, 444], [9, 54, 218], [10, 54, 255], [11, 407, 598], [12, 471, 598], [13, 301, 636], [14, 366, 636], [15, 329, 663], [16, 436, 621]].map(([n, x, y]) => [`b${n}`, x, y, 34]).concat([['pov-up', 199, 104, 18], ['pov-right', 222, 126, 18], ['pov-down', 199, 147, 18], ['pov-left', 176, 126, 18]]) },
};

/** Every mappable source of these devices: buttons, then hat directions ("t16000m/b3", "t16000m/hat-up"). */
export function sources(devices) {
  return devices.flatMap((d) => [
    ...Array.from({ length: d.layout.buttons }, (_, i) => `${d.id}/b${i + 1}`),
    ...(d.layout.hat ? HAT_DIRECTIONS.map((h) => `${d.id}/hat-${h}`) : []),
  ]);
}

/** "device/control" → [device, control]. */
export function parts(key) {
  const at = key.lastIndexOf('/');
  return [key.slice(0, at), key.slice(at + 1)];
}

export function label(key, names = {}) {
  const [device, control] = parts(key);
  const name = names[device] ?? device;
  if (control.startsWith('hat-')) return `${device === STICK ? 'Sidestick' : name} hat ${control.slice(4)}`;
  const n = Number(control.slice(1));
  if (device === STICK) return n === 17 ? 'Sidestick input 17' : `Sidestick button ${n}`;
  if (device === QUADRANT) return n > 16 ? `Quadrant add-on input ${n}` : n > 8 ? `Quadrant detent state ${n}` : `Quadrant button ${n}`;
  return `${name} button ${n}`;
}

/** Compact source name for tables: "S6", "Q2", "Hat up" on the TCA; "twcs 3", "twcs hat up" elsewhere. */
export function short(key) {
  const [device, control] = parts(key);
  const hat = control.startsWith('hat-') ? control.slice(4) : null;
  if (device === STICK) return hat ? `Hat ${hat}` : `S${control.slice(1)}`;
  if (device === QUADRANT) return `Q${control.slice(1)}`;
  return hat ? `${device} hat ${hat}` : `${device} ${control.slice(1)}`;
}

export function neoName(id) {
  if (id.startsWith('pov-')) return `NEO POV ${id.slice(4)}`;
  const n = Number(id.slice(1));
  return NEO_ACTIONS[n] ? `NEO ${n} (${NEO_ACTIONS[n]})` : `NEO ${n}`;
}

/** {buttons, pov, mode} → readable text. */
export function describe(mapping) {
  const names = mapping.buttons.map((n) => neoName(`b${n}`));
  if (mapping.pov) names.push(neoName(`pov-${mapping.pov}`));
  const text = names.length ? names.join(', ') : 'Nothing';
  return mapping.mode && mapping.mode !== 'hold' ? `${text} · ${MODES[mapping.mode].label.toLowerCase()}` : text;
}

/** Source behaviours: name, what the game receives, and the NEO pulses drawn on the timeline ([start%, width%]). */
export const MODES = {
  hold: { label: 'Hold', help: 'The NEO button stays pressed while the control is on. A latched switch keeps it held.', pulses: [[15, 55]] },
  pulse: { label: 'Pulse', help: 'One short press when the control turns on. Good for a mode selector: one flare burst, not a stream.', pulses: [[15, 6]] },
  pulse_change: { label: 'Pulse on change', help: 'One short press each time the control moves, on and off. Turning a latching switch on and off toggles Autopilot on and off.', pulses: [[15, 6], [70, 6]] },
  toggle: { label: 'Toggle', help: 'Each press turns the NEO button on or off, like a latch for a momentary button.', pulses: [[15, 85]] },
  repeat: { label: 'Repeat', help: 'Short presses again and again while held, at the Timing interval: rapid flares or gun bursts.', pulses: [[15, 6], [30, 6], [45, 6], [60, 6]] },
};

/** Actions a source can have: its press (by behaviour), a long press and a double tap. */
export const SLOTS = {
  press: { label: 'Press', help: 'Tap or hold, by its behaviour.' },
  long: { label: 'Long press', help: 'Held for the long-press time: these targets are held until release, instead of the press.' },
  double: { label: 'Double tap', help: 'Two taps within the double-tap window: one short press of these targets.' },
};

/**
 * A source's mapping for one action and layer: {buttons, pov, mode}. Behaviours belong to the press; long press
 * and double tap are the same in both layers.
 */
export function current(config, key, slot = 'press', layer = 'normal') {
  if (slot !== 'press') {
    const target = (slot === 'long' ? config.long_press : config.double_tap)?.[key];
    return { buttons: [...(target?.buttons ?? [])].sort((a, b) => a - b), pov: target?.pov ?? null, mode: 'hold' };
  }
  const source = layer === 'shift' ? config.shifted ?? { buttons: {}, pov: {} } : config;
  return { buttons: [...(source.buttons[key] ?? [])].sort((a, b) => a - b), pov: source.pov[key] ?? null, mode: config.behaviours?.[key] ?? 'hold' };
}

export function same(a, b) {
  return a.pov === b.pov && a.buttons.join() === b.buttons.join() && a.mode === b.mode;
}

/** What a proposal adds and removes compared with the saved mapping. */
export function diff(before, after) {
  const changes = [];
  for (const n of after.buttons) if (!before.buttons.includes(n)) changes.push(['add', `Adds ${neoName(`b${n}`)}`]);
  for (const n of before.buttons) if (!after.buttons.includes(n)) changes.push(['remove', `Removes ${neoName(`b${n}`)}`]);
  if (after.pov !== before.pov) {
    if (after.pov) changes.push(['add', `Adds ${neoName(`pov-${after.pov}`)}`]);
    if (before.pov) changes.push(['remove', `Removes ${neoName(`pov-${before.pov}`)}`]);
  }
  if (after.mode !== before.mode) changes.push(['add', `Behaviour: ${MODES[after.mode].label}`]);
  return changes;
}

export const AXES = ['roll', 'pitch', 'yaw'];
/** Button combos: [page id prefix, profile field, name in messages]. */
const COMBOS = [['pause', 'pause', 'pause'], ['next', 'next_profile', 'next-profile']];
/** Rail pages; the settings pages edit the draft and show the save bar. */
export const PAGES = ['mapping', 'axes', 'throttle', 'camera', 'helpers', 'controllers', 'live'];
const SETTINGS_PAGES = ['axes', 'throttle', 'camera', 'helpers', 'controllers'];
const SIDE_TABS = ['side-sel', 'side-neo', 'side-log'];
/** The press timeline spans this many milliseconds. */
export const PRESS_SPAN = 3000;
const HEAD_TOGGLES = [['head-enabled', 'enabled'], ['head-invert-yaw', 'invert_yaw'], ['head-invert-pitch', 'invert_pitch']];
const WARNING_ICON = '<svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true"><path d="M10 3 18 17H2Z"/><path d="M10 8v4M10 14.5v.5"/></svg>';
/** Mapping fields kept per controller set; every other field of the profile file is shared. */
export const PROFILE_KEYS = ['hat', 'camera_hat', 'bindings', 'split_high_g', 'camera_lever', 'yaw_source', 'precision', 'combos', 'behaviours', 'long_press', 'double_tap', 'shift', 'shifted', 'buttons', 'pov'];

/** One flat settings object: the shared settings of `file` with the mapping of one controller set. */
export function flatten(file, profile) {
  const shared = Object.fromEntries(Object.entries(file).filter(([key]) => key !== 'setups'));
  return structuredClone({ ...shared, ...profile });
}

/** The profile file with this flat settings object: shared settings, and the mapping stored for `setup`. */
export function compose(file, flat, setup) {
  const profile = Object.fromEntries(PROFILE_KEYS.filter((key) => key in flat).map((key) => [key, flat[key]]));
  const shared = Object.fromEntries(Object.entries(flat).filter(([key]) => !PROFILE_KEYS.includes(key)));
  return structuredClone({ ...shared, setups: setup ? { ...file.setups, [setup]: profile } : file.setups });
}

/** Camera deflection (-1..1, NEO convention: up is negative Y) from a head sample, as the daemon computes it. */
export function headCamera([yaw, pitch], tracking) {
  const sign = (invert) => (invert ? -1 : 1);
  return [shape((sign(tracking.invert_yaw) * yaw) / tracking.yaw_range, tracking.tuning), shape((-sign(tracking.invert_pitch) * pitch) / tracking.pitch_range, tracking.tuning)];
}
const AXIS_NAMES = { roll: 'Roll', pitch: 'Pitch', yaw: 'Yaw' };

/** One axis value in -1..1 through deadzone, saturation, curve and anti-deadzone (same as neo::shape). */
export function shape(value, t) {
  const v = Math.min(1, Math.max(-1, value));
  if (Math.abs(v) <= t.deadzone) return 0;
  const travel = Math.min((Math.abs(v) - t.deadzone) / (t.saturation - t.deadzone), 1);
  const k = t.curve;
  const curved = k >= 0 ? (1 - k) * travel + k * travel ** 3 : (1 + k) * travel - k * Math.cbrt(travel);
  return Math.sign(v) * (t.anti_deadzone + (1 - t.anti_deadzone) * curved);
}

/** Detent throttle: travel (0 idle, 1 forward) to -1 full brake, 0 cruise, +1 full boost (same as neo::detent). */
export function detent(travel, t) {
  const [low, high] = [t.cruise - t.band, t.cruise + t.band];
  if (travel <= t.idle) return -1;
  if (travel < low) return (travel - t.idle) / (low - t.idle) - 1;
  if (travel <= high) return 0;
  return travel < t.boost ? (travel - high) / (t.boost - high) : 1;
}

/** What the game reads for a throttle travel: -1 brake … +1 boost. */
export function throttleOut(travel, throttle) {
  return throttle.mode === 'detents' ? detent(travel, throttle) : 2 * travel - 1;
}

/** Graph coordinates: -1..1 mapped onto the 240-unit plot, 10 units of margin. */
const plot = (v) => 120 + v * 110;

/** SVG path of an axis response from full left to full right. */
export function curvePath(t) {
  return Array.from({ length: 121 }, (_, i) => {
    const x = i / 60 - 1;
    return `${i ? 'L' : 'M'}${plot(x).toFixed(1)} ${(240 - plot(shape(x, t))).toFixed(1)}`;
  }).join(' ');
}

const POV_NAMES = ['up', 'up-right', 'right', 'down-right', 'down', 'down-left', 'left', 'up-left'];
/** POV 0..7 to its cell in a 3 × 3 grid read left to right, top to bottom. */
const POV_CELLS = [1, 2, 5, 8, 7, 6, 3, 0];

/** A NEO axis value (0..65535, centre 32768) as -1..1. */
export function axisValue(raw) {
  return (raw / 65535) * 2 - 1;
}

/** Axis binding as text: "T.16000M x", "TCA Quadrant left (inverted)". */
export function bindingName(binding, names = {}) {
  return `${names[binding.device] ?? binding.device} ${binding.axis}${binding.invert ? ' (inverted)' : ''}`;
}

/** Split-lever names: the TCA quadrant's levers keep their own names. */
export function leverName(binding, names = {}) {
  if (binding.device === QUADRANT && ['left', 'right'].includes(binding.axis)) return `${binding.axis === 'left' ? 'Left' : 'Right'} lever`;
  return bindingName({ ...binding, invert: false }, names);
}

/** The device whose hat drives the camera: the profile's choice when it has a hat, else the first with one. */
export function cameraDevice(profile, devices) {
  const hatted = devices.filter((d) => d.layout.hat);
  return (hatted.find((d) => d.id === profile.camera_hat) ?? hatted[0])?.id ?? null;
}

/**
 * Live page rows from the daemon's monitor: [name, source, raw -1..1, sent -1..1]. The throttle reads as the game
 * does (+1 boost), the camera from the camera hat (or the camera lever).
 */
export function liveRows(monitor, pressed, config, names = {}, camera = null) {
  const name = (binding) => (binding ? bindingName({ ...binding, invert: false }, names) : 'No source');
  const hat = (on, off) => (pressed.includes(`${camera}/hat-${on}`) ? 1 : 0) - (pressed.includes(`${camera}/hat-${off}`) ? 1 : 0);
  const hatX = hat('right', 'left');
  const cameraX = config.camera_lever && !hatX ? monitor.camera_lever ?? 0 : hatX;
  const yaw = { twist: [name(config.bindings.yaw), monitor.yaw], off: ['Off', 0], lever_differential: ['Lever difference', monitor.yaw] }[config.yaw_source];
  const throttle = config.bindings.throttle.length ? config.bindings.throttle.map(name).join(' or ') : 'No source';
  return [
    ['Roll', name(config.bindings.roll), monitor.roll, axisValue(monitor.axes[0])],
    ['Pitch', name(config.bindings.pitch), monitor.pitch, axisValue(monitor.axes[1])],
    ['Yaw', yaw[0], yaw[1], axisValue(monitor.axes[5])],
    ['Throttle', throttle, monitor.throttle === null ? 0 : 2 * monitor.throttle - 1, -axisValue(monitor.axes[2])],
    ['Camera X', config.camera_lever ? 'Hat or camera lever' : 'Hat', cameraX, axisValue(monitor.axes[3])],
    ['Camera Y', 'Hat', hat('down', 'up'), axisValue(monitor.axes[4])],
  ];
}

const escape = (text) => String(text).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const actionName = (mapping) => {
  const parts = mapping.buttons.map((n) => NEO_ACTIONS[n] ?? `NEO ${n}`);
  if (mapping.pov) parts.push(`POV ${mapping.pov}`);
  return parts.join(', ');
};

/** The printable cheat sheet of a saved profile and its controllers, as HTML (every value escaped). */
export function cheatSheet(config, profile, date, devices = [], names = {}) {
  const camera = cameraDevice(config, devices);
  const keys = sources(devices).filter((key) => !(config.hat === 'camera' && key.startsWith(`${camera}/hat-`)));
  const cell = (text) => (text ? `<td>${escape(text)}</td>` : '<td class="x">—</td>');
  const rows = keys.map((key) => {
    const press = actionName(current(config, key));
    const long = actionName(current(config, key, 'long'));
    const double = actionName(current(config, key, 'double'));
    const shifted = actionName(current(config, key, 'press', 'shift'));
    if (key === config.shift || !(press || long || double || shifted)) return '';
    const mode = config.behaviours?.[key];
    const tag = mode && mode !== 'hold' && press ? `<span class="tag">${escape(MODES[mode].label.toLowerCase())}</span>` : '';
    const shift = config.shift ? (shifted ? `<td>${escape(shifted)}</td>` : '<td class="x">same</td>') : '';
    return `<tr><td class="k">${escape(short(key))}</td>${press ? `<td>${escape(press)}${tag}</td>` : '<td class="x">—</td>'}${cell(long)}${cell(double)}${shift}</tr>`;
  }).join('');
  const bound = (binding) => (binding ? bindingName({ ...binding, invert: false }, names) : 'No source');
  const curve = (axis) => (config.axes[axis].curve ? ` · curve ${Math.round(config.axes[axis].curve * 100)}%` : '');
  const throttles = config.bindings.throttle.map(bound).join(' or ') || 'No source';
  const axes = [
    [`${bound(config.bindings.roll)} / ${bound(config.bindings.pitch)}`, `Roll / pitch${curve('roll')}`],
    [config.yaw_source === 'lever_differential' ? 'Lever difference' : bound(config.bindings.yaw), config.yaw_source === 'off' ? 'Yaw off' : `Yaw${curve('yaw')}`],
    [throttles, config.throttle.mode === 'detents' ? 'Throttle · detents: idle brakes, the cruise detent cruises, boost from the last detent' : 'Throttle'],
    [camera ? `${names[camera] ?? camera} hat` : 'No hat', config.hat === 'mapped' ? 'Mapped as buttons (see above)' : `Camera${config.camera.full_after_hold ? ` · full after ${config.timing.long_ms} ms hold` : ''}`],
  ];
  const combo = (list) => list.map(short).join(' + ');
  const helpers = [
    config.precision.source && [`Hold ${short(config.precision.source)}`, `Precision aim · ${Math.round(config.precision.scale * 100)}%`],
    config.shift && [`Hold ${short(config.shift)}`, 'Shift layer'],
    config.combos.pause.length && [combo(config.combos.pause), 'Pause / resume input'],
    config.combos.next_profile.length && [combo(config.combos.next_profile), 'Next profile'],
    config.split_high_g.enabled && ['Levers split', 'High-G'],
  ].filter(Boolean);
  const table = (pairs) => `<table><tbody>${pairs.map(([k, v]) => `<tr><td class="k">${escape(k)}</td><td>${escape(v)}</td></tr>`).join('')}</tbody></table>`;
  const title = devices.length ? devices.map((d) => d.name).join(' + ') : 'No controllers';
  return `<div class="sheet-head"><div><h1>${escape(title)} → ACE COMBAT 8</h1><p>Profile: ${escape(profile ?? 'Unsaved profile')}</p></div><p class="meta">Any-TM-HOTAS bridge<br>Printed ${escape(date)}</p></div>`
    + `<h2>Buttons</h2><table><thead><tr><th>Control</th><th>Press</th><th>Long press</th><th>Double tap</th>${config.shift ? `<th>Shift (hold ${escape(short(config.shift))})</th>` : ''}</tr></thead><tbody>${rows}</tbody></table>`
    + `<div class="cols"><div><h2>Axes</h2>${table(axes)}</div><div><h2>Helpers</h2>${helpers.length ? table(helpers) : '<p>None set.</p>'}</div></div>`
    + `<p class="foot">Long press: ${config.timing.long_ms} ms · double tap: ${config.timing.double_ms} ms · repeat every ${config.timing.repeat_ms} ms. Generated from the saved profile; reprint after changes.</p>`;
}

/** Delay that smoothing adds at the 10 ms report period, in whole milliseconds. */
export function lag(smoothing) {
  return Math.round((10 * smoothing) / (1 - smoothing));
}

/** The ramp graph spans this long a hat hold, in milliseconds, across x 10..290 of its 300-unit plot. */
const RAMP_SPAN = 1500;
const rampX = (ms) => 10 + (Math.min(ms, RAMP_SPAN) / RAMP_SPAN) * 280;

/** SVG path of the camera output while the hat is held (same rule as the daemon's camera ramp). */
export function rampPath(camera, strength, longMs) {
  const points = [];
  for (let t = 0; t <= RAMP_SPAN; t += 25) {
    const ramp = camera.ramp_ms ? Math.min(1, t / camera.ramp_ms) : 1;
    const value = camera.full_after_hold && t >= longMs ? 1 : strength * ramp;
    points.push(`${points.length ? 'L' : 'M'}${rampX(t).toFixed(1)} ${(88 - value * 76).toFixed(1)}`);
  }
  return points.join(' ');
}

/**
 * How the presses of one control read against the timing: [{kind, from, to}] in the presses' own clock, where kind
 * is 'tap' (a short press at release), 'long' (held from the long-press time) or 'double' (a short press on the
 * second tap). A press still held below the long-press time reads as nothing yet.
 */
export function gestures(presses, timing, now) {
  const out = [];
  let tap = null;
  for (const { start, end } of presses) {
    const until = end ?? now;
    if (until - start >= timing.long_ms) {
      out.push({ kind: 'long', from: start + timing.long_ms, to: until });
      tap = null;
    } else if (end !== null) {
      if (tap && start - tap.end <= timing.double_ms) {
        out.splice(out.indexOf(tap.read), 1, { kind: 'double', from: start, to: start + timing.pulse_ms });
        tap = null;
      } else {
        const read = { kind: 'tap', from: end, to: end + timing.pulse_ms };
        out.push(read);
        tap = { end, read };
      }
    }
  }
  return out;
}

/** "+0.40", "−0.40", " 0.00". */
export function signed(value) {
  const text = Math.abs(value).toFixed(2);
  return `${value > 0 && text !== '0.00' ? '+' : value < 0 && text !== '0.00' ? '−' : ' '}${text}`;
}

/** Throttle zone name for the readout. */
export function zone(out, throttle) {
  if (throttle.mode !== 'detents') return [null, `${Math.round((out + 1) * 50)}%`];
  if (out === -1) return ['brake', 'Full brake'];
  if (out < 0) return ['brake', 'Braking'];
  if (out === 0) return ['cruise', 'Cruise'];
  return ['boost', out === 1 ? 'Full boost' : 'Boosting'];
}

/** Sources (among `keys`) currently mapped to a NEO hotspot. */
export function sourcesFor(config, id, keys) {
  if (id.startsWith('pov-')) return keys.filter((key) => config.pov[key] === id.slice(4));
  const n = Number(id.slice(1));
  return keys.filter((key) => (config.buttons[key] ?? []).includes(n));
}

/** True while the split-lever rule holds NEO 5. */
export function splitActive([first, second], rule) {
  return rule.enabled && Math.max(first, second) >= rule.forward && Math.min(first, second) <= rule.back;
}

/** Live lever readout, flagging an active split against the thresholds. */
export function leverText(levers, rule, names = ['First lever', 'Second lever']) {
  const [first, second] = levers;
  return `${names[0]} ${Math.round(first * 100)}% · ${names[1]} ${Math.round(second * 100)}%${splitActive(levers, rule) ? ' · High-G active' : ''}`;
}

/** Header pills from the daemon status and what the running session holds. */
export function pills(status, links) {
  const state = /^Emulating/.test(status) ? 'ok' : /^(Starting|Stopping)/.test(status) ? 'busy' : /^(Error|unreachable|unavailable)/.test(status) ? 'fault' : 'off';
  const validation = links?.source === 'validation';
  return {
    status: state,
    inputs: validation ? ['busy', 'Source: validation stream'] : links?.inputs ? ['ok', `Controllers: ${links.inputs} open`] : links ? ['busy', 'Controllers: reconnecting…'] : ['off', 'Controllers: not open'],
    hidhide: links?.hidden ? ['ok', 'HidHide: controllers hidden'] : ['off', validation ? 'HidHide: not needed' : 'HidHide: off'],
    neo: links?.virtual_neo ? ['ok', 'Virtual NEO present'] : ['off', 'Virtual NEO: absent'],
  };
}

/** The axis a live sample moved past half travel since the baseline, as a binding; null when nothing did. */
export function moved(baseline, axes, devices) {
  for (const device of devices) {
    const now = axes[device.id] ?? [];
    const index = now.findIndex((value, i) => Math.abs(value - (baseline[device.id]?.[i] ?? value)) > 0.5);
    if (index >= 0) return { device: device.id, axis: device.layout.axes[index].name, invert: false };
  }
  return null;
}

export function client(fetchImpl, token) {
  const call = async (method, path, body) => {
    const response = await fetchImpl(path, { method, headers: { 'X-Neo-Token': token, 'Content-Type': 'application/json' }, body });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error);
    return data;
  };
  return {
    get: (path) => call('GET', path),
    put: (path, value) => call('PUT', path, JSON.stringify(value)),
    post: (path) => call('POST', path),
    del: (path) => call('DELETE', path),
  };
}

const ARROW = '<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1.5 L8.5 8 L1.5 8 Z"/></svg>';
const HELP = {
  sel: 'Click NEO buttons or a POV arrow to change this control’s mapping, then record it. Changes apply immediately, even while emulating.',
  rev: 'These controls feed it. Select one of them to change its mapping.',
  none: 'Select a control, then click NEO buttons or a POV arrow to build its mapping. With nothing selected, clicking a NEO button shows what feeds it.',
};
const error = (id) => Page.FIELDS[id]?.[4] ?? `${id}-error`;
const percent = (value) => `${Math.round(value * 1000) / 1000}%`;
/** A centred bar for a value in -1..1. */
const bars = (bar, value) => {
  bar.style.setProperty('--from', `${50 + Math.min(value, 0) * 50}%`);
  bar.style.setProperty('--size', `${Math.abs(value) * 50}%`);
};
/** A state chip's text and tone (ok, warn, accent or none). */
const tone = (chip, text, value = '') => {
  chip.textContent = text;
  chip.dataset.tone = value;
};
const encode = (binding) => (binding ? `${binding.device}|${binding.axis}` : '');
const decode = (value, invert = false) => {
  if (!value) return null;
  const [device, axis] = value.split('|');
  return { device, axis, invert };
};
const sameAxis = (a, b) => a.device === b.device && a.axis === b.axis;

export class Page {
  constructor(doc, api) {
    this.doc = doc;
    this.api = api;
    this.full = null;           // the whole saved profile file (every controller set)
    this.saved = null;          // saved settings with this controller set's mapping (mapping edits start from here)
    this.config = null;         // settings draft, in the same flat form
    this.defaults = null;
    this.devices = [];          // controllers present (or in the running session)
    this.active = [];           // the ones taking part
    this.names = {};
    this.keys = [];
    this.setup = '';
    this.selected = null;       // source key
    this.proposal = null;       // {buttons, pov} being built for the selection
    this.reverse = null;        // NEO hotspot inspected with nothing selected
    this.hand = 'right';
    this.held = new Set();
    this.labels = [];
    this.busy = false;
    this.dirty = false;
    this.slot = 'press';        // action being mapped: press, long or double
    this.layer = 'normal';      // mapping layer being edited: normal or shift
    this.axis = 'roll';         // flight axis shown on the Flight axes page
    this.page = 'mapping';      // rail page shown
    this.testInput = 0.4;       // test controls: the previews use them only without live input
    this.testLever = 0.5;
    this.monitor = null;        // raw inputs from the running emulation
    this.capture = null;        // {kind: 'precision' | 'pause', keys} while picking controls by pressing them
    this.detecting = null;      // binding waiting for an axis movement
    this.baseline = null;
    this.previewing = false;    // "Try in game": the draft runs in the emulation without being saved
    this.sending = false;
    this.queued = false;
    this.presses = [];          // recent presses for the timing timeline: {key, start, end}
    this.hatSince = null;       // when the camera hat was pressed, while it is held
    this.clock = () => Date.now();
    this.$ = (id) => doc.getElementById(id);
  }

  async load() {
    const data = await this.api.get('/api/config');
    this.full = data.config;
    this.globals = data.defaults;
    if (data.profile) this.$('profile-path').textContent = data.profile;
    this.build();
    await this.refresh(data.preview);
    if (data.preview) {
      this.previewing = true;
      this.mark(true, 'Unsaved changes · running in game now.');
      this.renderSettings();
    }
    this.status(data.status, data.links);
    this.warnings(data.warnings);
    try {
      this.labels = (await this.api.get('/api/labels')).labels;
      this.renderLabels();
    } catch (error) {
      this.note(`Earlier corrections could not be loaded: ${error.message}`);
    }
    try {
      this.renderProfiles(await this.api.get('/api/profiles'));
    } catch (error) {
      this.profileNote(`Saved profiles could not be listed: ${error.message}`, true);
    }
  }

  /** Controllers and the active set's mapping; rebuilds everything that depends on them. */
  async refresh(preview = null) {
    const found = await this.api.get('/api/devices');
    this.devices = found.devices;
    this.active = found.devices.filter((d) => found.active.includes(d.id));
    this.names = Object.fromEntries(found.devices.map((d) => [d.id, d.name]));
    this.keys = sources(this.active);
    this.setup = found.setup;
    this.saved = flatten(this.full, found.profile);
    this.defaults = flatten(this.globals, found.default);
    this.selected = null;
    this.proposal = null;
    this.reverse = null;
    this.$('setup-name').textContent = this.active.length ? this.active.map((d) => d.name).join(' + ') : 'No controllers';
    this.buildSources();
    this.buildBindings();
    this.showSettings(preview ? flatten(preview, preview.setups?.[this.setup] ?? found.profile) : structuredClone(this.saved));
    this.render();
  }

  label(key) {
    return label(key, this.names);
  }

  /** Open or close the profile menu. */
  menu(open) {
    this.$('profile-menu').hidden = !open;
    this.$('profile-picker').setAttribute('aria-expanded', String(open));
    if (open) this.$('profile-new').focus();
  }

  profileNote(text, error = false) {
    this.$('profile-status').textContent = text;
    this.$('profile-status').dataset.state = error ? 'error' : 'ok';
  }

  /** {profiles, active} from the daemon: the picker, the list (active marked) and the actions. */
  renderProfiles({ profiles, active }) {
    this.profiles = profiles;
    this.profileName = active;
    this.$('profile-name').textContent = active ?? 'Unsaved profile';
    this.$('profiles-empty').hidden = profiles.length > 0;
    this.$('profile-list').replaceChildren(...profiles.map((name) => {
      const item = this.doc.createElement('li');
      const button = this.doc.createElement('button');
      button.type = 'button';
      button.setAttribute('aria-current', String(name === active));
      button.innerHTML = '<span></span><span class="tag"></span>';
      button.firstChild.textContent = name;
      button.lastChild.textContent = name === active ? 'active' : '';
      button.addEventListener('click', () => this.activate(name));
      item.append(button);
      return item;
    }));
    const deleting = this.$('profile-delete');
    deleting.disabled = !active;
    deleting.textContent = this.confirmDelete ? `Delete ${active}? Click again` : 'Delete';
  }

  /** Profile switches replace the saved settings, so unsaved edits must be saved or discarded first. */
  unsaved() {
    if (!this.dirty) return false;
    this.profileNote('Save or discard unsaved settings first.', true);
    return true;
  }

  /** Make the daemon's answer the page's state: the saved profile, the settings shown, the profile list. */
  adopt(result, text) {
    this.full = structuredClone(result.config);
    this.saved = flatten(this.full, this.full.setups[this.setup] ?? this.defaults);
    this.previewing = false;
    this.showSettings(structuredClone(this.saved));
    this.mark(false, text);
    this.renderProfiles(result);
    this.render();
    this.profileNote(text);
  }

  async activate(name) {
    if (this.unsaved()) return;
    try {
      const result = await this.api.post(`/api/profiles/${encodeURIComponent(name)}/activate`);
      this.adopt(result, `Switched to ${name}.${result.running ? ' Applied to the running emulation.' : ''}`);
    } catch (error) {
      this.profileNote(`Not switched: ${error.message}`, true);
    }
  }

  /** Save the saved settings under a new name (or replace a profile of that name), and make it active. */
  async saveAs() {
    const field = this.$('profile-new');
    const name = field.value.trim();
    const valid = /^[A-Za-z0-9_-]([A-Za-z0-9 _-]{0,38}[A-Za-z0-9_-])?$/.test(name);
    field.setAttribute('aria-invalid', String(!valid));
    if (!valid) return this.profileNote('Use 1 to 40 letters, digits, spaces, _ or -.', true);
    if (this.unsaved()) return undefined;
    try {
      const result = await this.api.put(`/api/profiles/${encodeURIComponent(name)}`, {});
      field.value = '';
      this.renderProfiles(result);
      return this.profileNote(`Saved as ${name}.`);
    } catch (error) {
      return this.profileNote(`Not saved: ${error.message}`, true);
    }
  }

  /** Delete the active profile; the first click asks, the second deletes. The settings themselves stay. */
  async deleteProfile() {
    const name = this.profileName;
    if (!this.confirmDelete) {
      this.confirmDelete = true;
      return this.renderProfiles({ profiles: this.profiles, active: name });
    }
    this.confirmDelete = false;
    try {
      this.renderProfiles(await this.api.del(`/api/profiles/${encodeURIComponent(name)}`));
      return this.profileNote(`Deleted ${name}. The current settings are kept.`);
    } catch (error) {
      this.renderProfiles({ profiles: this.profiles, active: name });
      return this.profileNote(`Not deleted: ${error.message}`, true);
    }
  }

  /** Fill the cheat sheet from the saved profile, then print it (print styles show only the sheet). */
  print(win = globalThis.window) {
    const date = new Date().toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
    this.$('cheatsheet').innerHTML = cheatSheet(this.saved, this.profileName, date, this.active, this.names);
    win.print();
  }

  /** The Live page: raw input next to what the NEO receives, the bridge state, NEO buttons and POV. */
  renderLive(data) {
    const monitor = data.monitor;
    this.$('live-empty').hidden = Boolean(monitor);
    const rows = monitor ? liveRows(monitor, data.pressed, this.saved, this.names, this.camera()) : [];
    this.$('live-axes').replaceChildren(...rows.map(([name, source, raw, sent]) => {
      const row = this.doc.createElement('div');
      row.className = 'live-axis';
      row.innerHTML = '<div class="name"><span></span></div><div class="pair"><div class="track raw"><i></i><span></span></div><div class="track out"><i></i><span></span></div></div><div class="value"></div>';
      row.firstChild.prepend(name);
      row.firstChild.lastChild.textContent = source;
      row.querySelectorAll('.track span').forEach((bar, i) => bars(bar, [raw, sent][i]));
      row.lastChild.textContent = signed(sent);
      return row;
    }));
    const states = [
      [monitor?.armed ?? true, 'Throttle armed', 'Throttle waiting for centre', 'ok', 'warn'],
      [monitor?.precision, 'Precision on', 'Precision off', 'warn', ''],
      [monitor?.shift, 'Shift held', 'Shift off', 'warn', ''],
      [data.paused, 'Input paused', 'Input live', 'warn', 'ok'],
      [data.preview, 'Previewing unsaved', 'Preview off', 'warn', ''],
    ];
    this.$('live-states').replaceChildren(...states.map(([on, yes, no, toneOn, toneOff]) => {
      const item = this.doc.createElement('li');
      item.textContent = on ? yes : no;
      item.dataset.tone = on ? toneOn : toneOff;
      return item;
    }));
    this.$('live-pressed').replaceChildren(...data.pressed.map((key) => {
      const chip = this.doc.createElement('span');
      chip.className = 'chip static';
      chip.textContent = short(key);
      chip.title = this.label(key);
      return chip;
    }));
    const buttons = monitor?.buttons ?? 0;
    this.$('live-buttons').replaceChildren(...Array.from({ length: 15 }, (_, i) => {
      const item = this.doc.createElement('li');
      item.dataset.on = String(Boolean(buttons & (1 << i)));
      item.innerHTML = '<b></b><span></span>';
      item.firstChild.textContent = String(i + 1);
      item.lastChild.textContent = NEO_ACTIONS[i + 1] ?? '—';
      return item;
    }));
    const pov = monitor?.pov ?? 8;
    const name = pov < 8 ? `POV ${POV_NAMES[pov]}` : 'POV centred';
    this.$('live-pov').querySelectorAll('span').forEach((cell, index) => { cell.dataset.on = String(index === POV_CELLS[pov]); });
    this.$('live-pov').setAttribute('aria-label', name);
    this.$('live-pov-text').textContent = name;
  }

  /** The saved settings of every controller set as a JSON file to keep or share. */
  exportProfile() {
    const blob = new Blob([`${JSON.stringify(this.full, null, 2)}\n`], { type: 'application/json' });
    const link = this.doc.createElement('a');
    link.href = URL.createObjectURL(blob);
    link.download = `${this.profileName ?? 'any-tm-hotas-bridge'}.json`;
    link.click();
    URL.revokeObjectURL(link.href);
    this.profileNote(`Exported ${link.download}.`);
  }

  /** A shared JSON profile (any version): the daemon checks and upgrades it before it becomes the saved settings. */
  async importFile(file) {
    if (!file) return;
    if (this.unsaved()) return;
    try {
      const profile = JSON.parse(await file.text());
      await this.api.put('/api/config', profile);
      const data = await this.api.get('/api/config');
      this.adopt({ config: data.config, profiles: this.profiles, active: this.profileName }, `Imported ${file.name}. Save it under a name to keep it as a profile.`);
    } catch (error) {
      this.profileNote(`Not imported: ${error.message}`, true);
    } finally {
      this.$('profile-import').value = '';
    }
  }

  /** One transparent button per numbered callout, sized and placed in the diagram's own units. */
  spot(container, [key, x, y, size], box, attribute) {
    const [left, top, width, height] = box;
    const button = this.doc.createElement('button');
    button.type = 'button';
    button.className = key.startsWith('pov-') ? 'hs pov' : 'hs';
    button.dataset[attribute] = key;
    button.style.setProperty('--x', percent(((x - left) / width) * 100));
    button.style.setProperty('--y', percent(((y - top) / height) * 100));
    button.style.setProperty('--w', percent((size / width) * 100));
    container.append(button);
    return button;
  }

  control(element) {
    const name = this.label(element.dataset.source);
    element.setAttribute('aria-label', name);
    element.setAttribute('aria-pressed', 'false');
    element.title = name;
    element.addEventListener('click', () => this.select(element.dataset.source));
  }

  chip(key, text, extra = '') {
    const chip = this.doc.createElement('button');
    chip.type = 'button';
    chip.className = `chip${extra}`;
    chip.dataset.source = key;
    chip.textContent = text;
    this.control(chip);
    return chip;
  }

  chipRow(container, caption, chips) {
    const row = this.doc.createElement('div');
    row.className = 'chip-row';
    row.innerHTML = '<span class="cap"></span><div class="chips end"></div>';
    row.firstElementChild.textContent = caption;
    row.lastElementChild.append(...chips);
    container.append(row);
  }

  stickSpots() {
    const container = this.$('stick-spots');
    for (const old of container.querySelectorAll('[data-source]')) old.remove();
    const diagram = DIAGRAMS[this.hand];
    for (const spot of diagram.spots) this.control(this.spot(container, spot, diagram.box, 'source'));
    const setup = this.hand === 'right' ? 'Right-hand setup' : 'Left-hand setup';
    const image = this.$('stick-img');
    image.src = `device/sidestick-${this.hand}.svg`;
    image.alt = `TCA Sidestick, top view, ${setup.toLowerCase()}, numbered controls 1 to 17`;
    this.$('hand-caption').textContent = setup;
    for (const radio of this.doc.querySelectorAll('input[name=hand]')) radio.checked = radio.value === this.hand;
  }

  /** Diagrams for the TCA pair, a generated button panel for any other controller, hat and extra chips. */
  buildSources() {
    const container = this.$('sources');
    container.replaceChildren();
    this.$('sources-empty').hidden = this.active.length > 0;
    for (const id of ['hat-rows', 'extra-detents', 'extra-addons']) this.$(id).replaceChildren();
    this.$('quadrant-rows').hidden = true;
    for (const device of this.active) {
      if (device.id === STICK || device.id === QUADRANT) {
        container.append(this.$(`tpl-${device.id}`).content.cloneNode(true));
      } else {
        const figure = this.doc.createElement('figure');
        figure.className = 'fig generic';
        figure.dataset.device = device.id;
        figure.innerHTML = '<figcaption class="cap split"><span class="device-name"></span><span></span></figcaption><div class="chips"></div>';
        figure.querySelector('.device-name').textContent = device.name;
        figure.querySelector('figcaption').lastElementChild.textContent = `${device.layout.buttons} buttons · ${device.layout.axes.length} axes`;
        const keys = Array.from({ length: device.layout.buttons }, (_, i) => `${device.id}/b${i + 1}`);
        figure.lastElementChild.append(...keys.map((key) => this.chip(key, parts(key)[1].slice(1))));
        container.append(figure);
      }
      if (device.layout.hat) {
        const hats = HAT_DIRECTIONS.map((d) => this.chip(`${device.id}/hat-${d}`, d[0].toUpperCase() + d.slice(1), ' hat-chip'));
        this.chipRow(this.$('hat-rows'), `${device.id === STICK ? 'Sidestick' : device.name} hat`, hats);
      }
    }
    if (this.$('stick-spots')) {
      for (const radio of this.doc.querySelectorAll('input[name=hand]')) {
        radio.addEventListener('change', () => { this.hand = radio.value; this.stickSpots(); this.render(); });
      }
      this.stickSpots();
    }
    if (this.$('quad-spots')) {
      for (const spot of DIAGRAMS.quadrant.spots) this.control(this.spot(this.$('quad-spots'), spot, DIAGRAMS.quadrant.box, 'source'));
      const extra = (from, to) => Array.from({ length: to - from + 1 }, (_, i) => `${QUADRANT}/b${from + i}`);
      this.$('extra-detents').append(...extra(9, 16).map((key) => this.chip(key, parts(key)[1].slice(1))));
      this.$('extra-addons').append(...extra(17, 31).map((key) => this.chip(key, parts(key)[1].slice(1))));
      this.$('quadrant-rows').hidden = false;
    }
    const camera = this.$('camera-device');
    const hatted = this.active.filter((d) => d.layout.hat);
    camera.replaceChildren(...hatted.map((d) => this.option(`${d.name} hat`, d.id)));
    camera.disabled = hatted.length < 2;
  }

  option(text, value) {
    const option = this.doc.createElement('option');
    option.textContent = text;
    option.value = value;
    return option;
  }

  /** Axis selects for the present controllers, and the controller list. */
  buildBindings() {
    const options = (empty) => [this.option(empty, ''), ...this.active.flatMap((d) => d.layout.axes.map((a) => this.option(`${d.name} · ${a.name}`, `${d.id}|${a.name}`)))];
    this.$('bindings').replaceChildren(...NEO_AXES.map(([axis, neo, action]) => {
      const row = this.doc.createElement('div');
      row.className = 'binding';
      row.innerHTML = `<label class="label" for="bind-${axis}">${action}</label><select id="bind-${axis}" class="sel" data-axis="${axis}" form="profile"></select><label class="tog"><input type="checkbox" id="invert-${axis}" data-invert="${axis}" form="profile"><span>Invert</span></label><button class="btn quiet" type="button" data-detect="${axis}">Detect</button>`;
      row.querySelector('select').append(...options(`No source (NEO ${neo} rests)`));
      return row;
    }));
    this.$('throttle-add').replaceChildren(...options('Add an axis…'));
    for (const id of ['lever-0', 'lever-1']) this.$(id).replaceChildren(...options('Not set'));
    this.$('camera-lever').replaceChildren(...options('Not set'));
    this.$('device-list').replaceChildren(...this.devices.map((d) => {
      const item = this.doc.createElement('label');
      item.className = 'tog';
      item.innerHTML = '<input type="checkbox" data-device="" form="profile"><span></span>';
      item.firstElementChild.dataset.device = d.id;
      item.lastElementChild.textContent = `${d.name} (${d.id})`;
      return item;
    }));
    const info = [['Controllers', this.active.length ? this.active.map((d) => d.name).join(' + ') : 'None'], ['Set', this.setup || '—'], ['Mapping', this.full.setups[this.setup] ? 'Saved for this set' : 'Default for this set']];
    this.$('set-info').replaceChildren(...info.map(([term, value]) => {
      const row = this.doc.createElement('div');
      row.innerHTML = '<dt class="mono"></dt><dd></dd>';
      row.firstChild.textContent = term;
      row.lastChild.textContent = value;
      return row;
    }));
  }

  /** Static listeners and the NEO side, once. */
  build() {
    for (const spot of DIAGRAMS.neo.spots) {
      const id = spot[0];
      const button = this.spot(this.$('neo-spots'), spot, DIAGRAMS.neo.box, 'neo');
      button.disabled = id === 'b16';
      if (id.startsWith('pov-')) button.innerHTML = ARROW;
      button.setAttribute('aria-label', button.disabled ? 'NEO 16: on the hardware, not in the NEO report' : neoName(id));
      button.title = button.getAttribute('aria-label');
      button.setAttribute('aria-pressed', 'false');
      button.addEventListener('click', () => this.pick(id));
    }
    for (let n = 1; n <= 15; n++) {
      const row = this.doc.createElement('button');
      row.type = 'button';
      row.className = 'nrow';
      row.dataset.row = `b${n}`;
      row.setAttribute('aria-pressed', 'false');
      row.innerHTML = `<span class="nnum">${n}</span><span class="action${NEO_ACTIONS[n] ? '' : ' none'}">${NEO_ACTIONS[n] ?? 'No AC8 action'}</span><span class="fed mono"></span>`;
      row.addEventListener('click', () => this.pick(`b${n}`));
      this.$('neo-rows').append(row);
    }
    const pov = this.doc.createElement('div');
    pov.className = 'nrow';
    pov.dataset.row = 'pov';
    pov.innerHTML = '<span class="nnum">POV</span><span class="action none">Hat, 4 directions · pick an arrow on the diagram</span><span class="fed mono"></span>';
    this.$('neo-rows').append(pov);
    for (const radio of this.doc.querySelectorAll('input[name=hat]')) radio.addEventListener('change', () => this.commitProfile({ hat: radio.value }));
    this.$('camera-device').addEventListener('change', (event) => this.commitProfile({ camera_hat: event.target.value }));
    this.$('record').addEventListener('click', () => this.record());
    this.$('revert').addEventListener('click', () => { this.proposal = this.mapping(this.selected); this.render(); });
    for (const radio of this.doc.querySelectorAll('input[name=layer], input[name=slot]')) {
      radio.addEventListener('change', () => {
        this[radio.name] = radio.value;
        // Long press and double tap are the same in both layers: the shift layer edits the press only.
        if (this.layer === 'shift') this.slot = 'press';
        if (this.selected) this.proposal = this.mapping(this.selected);
        this.render();
      });
    }
    this.$('deselect').addEventListener('click', () => { this.selected = null; this.proposal = null; this.reverse = null; this.render(); });
    for (const button of this.doc.querySelectorAll('[data-page]')) button.addEventListener('click', () => this.show(button.dataset.page));
    for (const tab of this.doc.querySelectorAll('[role=tab]')) {
      tab.addEventListener('click', () => this.side(tab.id));
      tab.addEventListener('keydown', (event) => {
        const step = { ArrowRight: 1, ArrowLeft: SIDE_TABS.length - 1 }[event.key];
        if (step) this.side(SIDE_TABS[(SIDE_TABS.indexOf(tab.id) + step) % SIDE_TABS.length], true);
      });
    }
    for (const radio of this.doc.querySelectorAll('input[name=behaviour]')) {
      radio.addEventListener('change', () => { this.proposal.mode = radio.value; this.render(); });
    }
    // Settings fields belong to the profile form, including Try in game in the footer outside it.
    this.$('neo-app').addEventListener('input', (event) => { if (event.target.form === this.$('profile')) this.edit(event.target); });
    this.$('profile').addEventListener('submit', (event) => { event.preventDefault(); return this.save(); });
    this.$('profile').addEventListener('click', (event) => {
      const button = event.target.closest?.('button');
      if (button?.dataset.detect) this.detect(button.dataset.detect);
      if (button?.dataset.remove) this.removeThrottle(Number(button.dataset.remove));
    });
    this.$('reset').addEventListener('click', () => {
      this.showSettings(structuredClone(this.defaults));
      this.changed('Unsaved: defaults, including every mapping of these controllers. Save settings to apply.');
    });
    this.$('discard').addEventListener('click', () => this.discard());
    this.$('copy-axis').addEventListener('click', () => {
      const targets = this.axis === 'yaw' ? ['roll', 'pitch'] : [this.axis === 'roll' ? 'pitch' : 'roll'];
      for (const axis of targets) this.config.axes[axis] = { ...this.config.axes[this.axis] };
      this.changed(`Unsaved: ${AXIS_NAMES[this.axis]} settings copied to ${targets.join(' and ')}.`);
    });
    this.$('reset-axis').addEventListener('click', () => {
      this.config.axes[this.axis] = { ...this.defaults.axes[this.axis] };
      this.fill();
      this.changed(`Unsaved: ${AXIS_NAMES[this.axis]} back to its defaults.`);
    });
    for (const button of this.doc.querySelectorAll('[data-capture]')) {
      button.addEventListener('click', () => {
        const point = button.dataset.capture;
        this.config.throttle[point] = Math.round(this.lever() * 100) / 100;
        this.$(`detent-${point}`).value = Math.round(this.config.throttle[point] * 100);
        this.changed(`Unsaved: ${point === 'cruise' ? 'CL (cruise)' : point === 'idle' ? 'Idle' : 'Boost'} captured at ${Math.round(this.lever() * 100)}%.`);
      });
    }
    for (const [kind, text] of [['precision', 'precision aiming'], ['shift', 'shift layer']]) {
      this.$(`${kind}-pick`).addEventListener('click', () => this.startCapture(kind));
      this.$(`${kind}-clear`).addEventListener('click', () => {
        this.setSingle(kind, null);
        this.changed(`Unsaved: ${text} off.`);
      });
    }
    for (const [kind, field, text] of COMBOS) {
      this.$(`${kind}-record`).addEventListener('click', () => {
        if (this.capture?.kind !== kind) return this.startCapture(kind);
        this.config.combos[field] = this.capture.keys;
        this.capture = null;
        return this.changed(this.config.combos[field].length ? `Unsaved: new ${text} combo.` : `Unsaved: ${text} combo off.`);
      });
      this.$(`${kind}-clear`).addEventListener('click', () => {
        this.config.combos[field] = [];
        this.changed(`Unsaved: ${text} combo off.`);
      });
    }
    this.$('profile-picker').addEventListener('click', () => this.menu(this.$('profile-menu').hidden));
    this.doc.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && !this.$('profile-menu').hidden) {
        this.menu(false);
        this.$('profile-picker').focus();
      }
    });
    this.doc.addEventListener('click', (event) => {
      if (!this.$('profile-menu').hidden && !event.target.closest?.('.profile-bar')) this.menu(false);
    });
    this.$('save-as').addEventListener('submit', (event) => { event.preventDefault(); return this.saveAs(); });
    this.$('profile-import').addEventListener('change', (event) => this.importFile(event.target.files[0]));
    this.$('profile-export').addEventListener('click', () => this.exportProfile());
    this.$('profile-delete').addEventListener('click', () => this.deleteProfile());
    this.$('print').addEventListener('click', () => this.print());
  }

  /** Show one rail page. Settings pages share the draft and the save bar. */
  show(name) {
    this.page = name;
    for (const page of PAGES) {
      this.$(`page-${page}`).hidden = page !== name;
      if (page === name) this.$(`nav-${page}`).setAttribute('aria-current', 'page'); else this.$(`nav-${page}`).removeAttribute('aria-current');
    }
    const settings = SETTINGS_PAGES.includes(name);
    this.$('foot-actions').hidden = !settings;
    if (name === 'live') this.renderLive(this.lastLive ?? { pressed: [], monitor: null });
    // A preview belongs to the settings pages: leaving them ends it, keeping the draft.
    if (!settings && this.previewing) {
      this.setPreview(false).then(() => this.mark(true, 'Unsaved changes · preview ended when you left the settings.'));
    }
  }

  /** The mapping page's side panel: the selected control, the NEO buttons table or the correction log. */
  side(id, focus = false) {
    for (const tab of this.doc.querySelectorAll('[role=tab]')) {
      const active = tab.id === id;
      tab.setAttribute('aria-selected', String(active));
      tab.tabIndex = active ? 0 : -1;
      this.$(tab.getAttribute('aria-controls')).hidden = !active;
      if (active && focus) tab.focus();
    }
  }

  select(key) {
    this.selected = key;
    this.proposal = this.mapping(key);
    this.reverse = null;
    this.side('side-sel');
    this.render();
  }

  /** "Sidestick button 2 · long press", "Sidestick button 1 (shift layer)". */
  subject(key) {
    return `${this.label(key)}${this.layer === 'shift' ? ' (shift layer)' : ''}${this.slot === 'press' ? '' : ` · ${SLOTS[this.slot].label.toLowerCase()}`}`;
  }

  /** A mapping as text; an empty shift-layer press says that the normal mapping applies instead. */
  shown(mapping, slot = this.slot) {
    if (this.layer !== 'shift' || slot !== 'press' || mapping.buttons.length || mapping.pov) return describe(mapping);
    return `Normal: ${describe(current(this.saved, this.selected))}`;
  }

  /** The saved mapping of a source for the action and layer being edited. */
  mapping(key) {
    return current(this.saved, key, this.slot, this.layer);
  }

  camera() {
    return cameraDevice(this.saved, this.active);
  }

  /** Save the whole file with these saved settings (this set's mapping replaced). */
  async commit(next) {
    const file = compose(this.full, next, this.setup);
    const result = await this.api.put('/api/config', file);
    this.full = file;
    this.saved = next;
    return result;
  }

  /** Hat mode and camera hat are part of the mapping: save them at once, like a recorded mapping. */
  async commitProfile(change) {
    this.busy = true;
    try {
      const result = await this.commit({ ...structuredClone(this.saved), ...change });
      Object.assign(this.config, structuredClone(change));
      if (this.selected && !this.selectable(this.selected)) { this.selected = null; this.proposal = null; }
      const applied = result.running ? ' Applied to the running emulation.' : '';
      const camera = this.camera();
      const name = camera ? `${this.names[camera]} hat` : 'The hat';
      this.note(this.saved.hat === 'mapped'
        ? `${name} now sends its mapped controls; the camera mini-stick gets no input.${applied}`
        : `${name} drives the camera; its mappings are kept but inactive.${applied}`);
    } catch (error) {
      this.note(`Not saved: ${error.message}`);
    }
    this.busy = false;
    this.render();
  }

  /** The camera hat cannot be mapped while it drives the camera. */
  selectable(key) {
    const [device, control] = parts(key);
    return this.keys.includes(key) && !(control.startsWith('hat-') && this.saved.hat === 'camera' && device === this.camera());
  }

  pick(id) {
    if (!this.selected) {
      this.reverse = this.reverse === id ? null : id;
    } else if (id.startsWith('pov-')) {
      const dir = id.slice(4);
      this.proposal.pov = this.proposal.pov === dir ? null : dir;
    } else {
      const n = Number(id.slice(1));
      const buttons = this.proposal.buttons.includes(n) ? this.proposal.buttons.filter((b) => b !== n) : [...this.proposal.buttons, n];
      this.proposal.buttons = buttons.sort((a, b) => a - b);
    }
    this.render();
  }

  render() {
    const now = this.selected ? this.mapping(this.selected) : null;
    const feeds = this.reverse ? sourcesFor(this.saved, this.reverse, this.keys) : [];
    const mode = now ? 'sel' : this.reverse ? 'rev' : 'none';
    for (const control of this.doc.querySelectorAll('[data-source]')) {
      const key = control.dataset.source;
      control.setAttribute('aria-pressed', String(key === this.selected));
      control.classList.toggle('is-feed', feeds.includes(key));
      control.classList.toggle('held', this.held.has(key));
    }
    const hatMapped = this.saved.hat === 'mapped';
    for (const radio of this.doc.querySelectorAll('input[name=hat]')) radio.checked = radio.value === (hatMapped ? 'mapped' : 'camera');
    const camera = this.camera();
    this.$('camera-device').value = camera ?? '';
    this.$('hat-hint').textContent = !camera ? 'No controller with a hat: the camera mini-stick rests.' : hatMapped ? 'Each direction maps like a button; the camera gets no input.' : 'Pans the camera through the NEO mini-stick (Rx/Ry).';
    for (const chip of this.doc.querySelectorAll('.hat-chip')) {
      chip.disabled = !this.selectable(chip.dataset.source);
      chip.title = chip.disabled ? 'Set the camera hat to Mapped to map it' : this.label(chip.dataset.source);
    }
    const inMapping = (mapping, id) => (id.startsWith('pov-') ? mapping.pov === id.slice(4) : mapping.buttons.includes(Number(id.slice(1))));
    for (const spot of this.doc.querySelectorAll('[data-neo]')) {
      const id = spot.dataset.neo;
      const was = Boolean(now) && inMapping(now, id);
      const will = Boolean(now) && inMapping(this.proposal, id);
      spot.setAttribute('aria-pressed', String(now ? will : id === this.reverse));
      spot.classList.toggle('kept', was && will);
      spot.classList.toggle('added', will && !was);
      spot.classList.toggle('removed', was && !will);
    }
    for (const row of this.doc.querySelectorAll('[data-row]')) {
      const id = row.dataset.row;
      const fed = id === 'pov'
        ? HAT_DIRECTIONS.map((d) => [d, sourcesFor(this.saved, `pov-${d}`, this.keys)]).filter(([, keys]) => keys.length).map(([d, keys]) => `${d[0].toUpperCase()}: ${keys.map(short).join(' · ')}`).join('  ')
        : sourcesFor(this.saved, id, this.keys).map(short).join(' · ');
      row.querySelector('.fed').textContent = fed || '—';
      const active = id === 'pov' ? Boolean(this.reverse?.startsWith('pov-')) : this.reverse === id;
      row.classList.toggle('is-sel', active);
      row.classList.toggle('is-new', Boolean(now) && (id === 'pov' ? Boolean(this.proposal.pov) : this.proposal.buttons.includes(Number(id.slice(1)))));
      if (id !== 'pov') row.setAttribute('aria-pressed', String(active));
    }
    const changed = Boolean(now) && !same(now, this.proposal);
    const context = this.$('neo-context');
    context.dataset.mode = mode;
    this.$('link-heading').textContent = mode === 'rev' ? 'Selected NEO control' : 'Selected control';
    this.$('selected-kind').textContent = { sel: 'Your control', rev: 'NEO target', none: 'Selection' }[mode];
    this.$('link-help').textContent = HELP[mode];
    this.$('compare').hidden = mode === 'none';
    this.$('mapped-new-label').hidden = mode !== 'sel';
    this.$('mapped-new').hidden = mode !== 'sel';
    this.$('mapped-now-label').textContent = mode === 'rev' ? 'Fed by' : 'Mapped now';
    if (now) {
      this.$('selected-name').textContent = this.label(this.selected);
      this.$('mapped-now').textContent = this.shown(now);
      this.$('mapped-new').textContent = changed ? describe(this.proposal) : 'Unchanged';
      this.$('neo-context-text').textContent = `Mapping of ${this.subject(this.selected)} → ${describe(this.proposal)}`;
    } else if (this.reverse) {
      this.$('selected-name').textContent = neoName(this.reverse);
      this.$('mapped-now').textContent = feeds.length ? feeds.map((key) => this.label(key)).join(', ') : 'No control';
      this.$('neo-context-text').textContent = `Inspecting ${neoName(this.reverse)} · fed by ${feeds.length ? feeds.map(short).join(' · ') : 'nothing'}`;
    } else {
      this.$('selected-name').textContent = 'Nothing selected';
      this.$('mapped-now').textContent = '—';
      this.$('neo-context-text').textContent = 'Select a control to light up its NEO targets';
    }
    const list = this.$('diff');
    list.replaceChildren(...(now ? diff(now, this.proposal) : []).map(([kind, text]) => {
      const item = this.doc.createElement('li');
      item.className = kind;
      item.textContent = text;
      return item;
    }));
    this.renderLayer();
    const press = this.slot === 'press';
    this.$('slots').hidden = !now;
    this.$('behaviour').hidden = !now || !press;
    this.$('gestures').hidden = !now || press;
    if (now) {
      for (const radio of this.doc.querySelectorAll('input[name=slot]')) {
        radio.checked = radio.value === this.slot;
        radio.disabled = this.layer === 'shift' && radio.value !== 'press';
        this.doc.querySelector(`[data-slot=${radio.value}]`).textContent = this.shown(current(this.saved, this.selected, radio.value, this.layer), radio.value);
      }
      this.$('slot-help').textContent = this.layer === 'shift' ? 'The shift layer changes the press only; long press and double tap are the same in both layers.' : SLOTS[this.slot].help;
      this.$('gesture-help').textContent = `With a long press or double tap set, a tap is sent once the control is released and the double-tap window (${this.saved.timing.double_ms} ms) has passed, as a short press.`;
      const mode = MODES[this.proposal.mode];
      for (const radio of this.doc.querySelectorAll('input[name=behaviour]')) radio.checked = radio.value === this.proposal.mode;
      this.$('behaviour-help').textContent = mode.help;
      this.$('timeline').setAttribute('aria-label', `What the game receives with ${mode.label}`);
      this.$('timeline-out').replaceChildren(...mode.pulses.map(([start, width]) => {
        const pulse = this.doc.createElement('span');
        pulse.className = 'out';
        pulse.style.setProperty('left', `${start}%`);
        pulse.style.setProperty('width', `${width}%`);
        return pulse;
      }));
    }
    this.$('record').disabled = !changed || this.busy;
    this.$('revert').disabled = !changed || this.busy;
    this.$('deselect').disabled = mode === 'none';
    this.renderAxes();
  }

  /** The Normal / Shift switch: Shift needs a shift control, chosen on the Buttons & helpers page. */
  renderLayer() {
    const shift = this.saved.shift;
    if (!shift) this.layer = 'normal';
    for (const input of this.doc.querySelectorAll('input[name=layer]')) input.checked = input.value === this.layer;
    const option = this.doc.querySelector('input[name=layer][value=shift]');
    option.disabled = !shift;
    option.parentElement.title = shift ? '' : 'Choose a shift control on the Buttons & helpers page';
    this.$('layer-shift').textContent = shift ? `Shift · ${short(shift)}` : 'Shift';
    const note = this.$('layer-note');
    note.hidden = this.layer !== 'shift';
    note.textContent = shift ? `Shift layer: these mappings apply while ${this.label(shift)} is held. Controls without one keep their normal mapping.` : '';
  }

  renderAxes() {
    const bindings = this.saved.bindings;
    const camera = this.camera();
    const name = (binding) => (binding ? bindingName(binding, this.names) : 'No source');
    const yaw = { twist: name(bindings.yaw), off: 'No source (yaw off)', lever_differential: 'Split lever difference' }[this.saved.yaw_source];
    const rows = [
      ['X', name(bindings.roll), 'Roll'],
      ['Y', name(bindings.pitch), 'Pitch'],
      ['Rz', yaw, 'Yaw'],
      ['Z', bindings.throttle.length ? bindings.throttle.map((b) => bindingName(b, this.names)).join(' or ') : 'No source', 'Throttle'],
      ['Rx / Ry', `${this.saved.hat === 'mapped' ? 'No source (hat is mapped)' : camera ? `${this.names[camera]} hat` : 'No hat'}${this.saved.camera_lever ? ` + ${bindingName(this.saved.camera_lever, this.names)}` : ''}`, 'Camera'],
    ];
    this.$('axes').replaceChildren(...rows.map(([axis, source, action]) => {
      const row = this.doc.createElement('div');
      row.innerHTML = '<dt class="mono"></dt><dd></dd><dd class="mono act"></dd>';
      [axis, source, action].forEach((text, i) => { row.children[i].textContent = text; });
      return row;
    }));
  }

  async record() {
    const key = this.selected;
    const before = this.mapping(key);
    const after = structuredClone(this.proposal);
    const next = structuredClone(this.saved);
    if (this.slot === 'press') {
      // The press of the edited layer, and the behaviour (shared by both layers).
      const layer = this.layer === 'shift' ? (next.shifted = { buttons: { ...next.shifted?.buttons }, pov: { ...next.shifted?.pov } }) : next;
      if (after.buttons.length) layer.buttons[key] = after.buttons; else delete layer.buttons[key];
      if (after.pov) layer.pov[key] = after.pov; else delete layer.pov[key];
      next.behaviours = { ...next.behaviours };
      if (after.mode !== 'hold') next.behaviours[key] = after.mode; else delete next.behaviours[key];
    } else {
      const field = this.slot === 'long' ? 'long_press' : 'double_tap';
      next[field] = { ...next[field] };
      if (after.buttons.length || after.pov) next[field][key] = { buttons: after.buttons, pov: after.pov }; else delete next[field][key];
    }
    const what = this.subject(key);
    this.busy = true;
    this.render();
    let result;
    try {
      result = await this.commit(next);
    } catch (error) {
      this.note(`Not saved: ${error.message}`);
      this.busy = false;
      this.render();
      return;
    }
    for (const field of ['buttons', 'pov', 'behaviours', 'shifted', 'long_press', 'double_tap']) this.config[field] = structuredClone(next[field]);
    const entry = { label: what, sources: [key], mapping: `${describe(before)} → ${describe(after)}`.slice(0, 200) };
    const applied = result.running ? ' Applied to the running emulation.' : '';
    try {
      await this.api.put('/api/labels', [...this.labels, entry]);
      this.labels = [...this.labels, entry];
      this.renderLabels();
      this.note(`Recorded ${what}: ${describe(after)}.${applied}`);
    } catch (error) {
      this.note(`Mapping saved, but the correction log was not updated: ${error.message}.${applied}`);
    }
    this.busy = false;
    this.render();
  }

  /** Newest first; each entry reads "who → new, was old". */
  renderLabels() {
    const list = this.$('recorded');
    list.replaceChildren();
    this.labels.forEach((entry, index) => {
      const [was, to = was] = entry.mapping.split(' → ');
      const item = this.doc.createElement('li');
      item.innerHTML = '<div class="entry"><span class="who"></span><span class="to mono"></span><span class="was mono"></span></div>';
      item.querySelector('.who').textContent = entry.label;
      item.querySelector('.to').textContent = `→ ${to}`;
      item.querySelector('.was').textContent = `was ${was}`;
      const remove = this.doc.createElement('button');
      remove.type = 'button';
      remove.className = 'btn quiet';
      remove.textContent = 'Remove';
      remove.setAttribute('aria-label', `Remove ${entry.label} from the correction log`);
      remove.addEventListener('click', () => this.forget(index));
      item.append(remove);
      list.prepend(item);
    });
    this.$('recorded-count').textContent = String(this.labels.length);
    this.$('recorded-empty').hidden = this.labels.length > 0;
  }

  async forget(index) {
    const next = this.labels.filter((_, i) => i !== index);
    try {
      await this.api.put('/api/labels', next);
      this.labels = next;
      this.renderLabels();
      this.note('Removed from the correction log. The mapping itself is unchanged.');
    } catch (error) {
      this.note(`Not removed: ${error.message}`);
    }
  }

  note(text) {
    this.$('link-status').textContent = text;
  }

  status(text, links = null) {
    this.$('daemon-status').textContent = `Daemon: ${text}`;
    const state = pills(text, links);
    this.$('pill-status').dataset.state = state.status;
    for (const name of ['inputs', 'hidhide', 'neo']) {
      const pill = this.$(`pill-${name}`);
      pill.dataset.state = state[name][0];
      pill.lastElementChild.textContent = state[name][1];
    }
  }

  /**
   * Settings fields: [where in the profile, key, minimum, maximum, error element, factor]. Percent fields are stored
   * as fractions (factor 100); millisecond fields as they are (factor 1).
   */
  static FIELDS = {
    'tune-deadzone': ['axis', 'deadzone', 0, 50],
    'tune-saturation': ['axis', 'saturation', 50, 100],
    'tune-anti': ['axis', 'anti_deadzone', 0, 50],
    'tune-curve': ['axis', 'curve', -100, 100],
    'tune-smoothing': ['axis', 'smoothing', 0, 90],
    camera: [null, 'camera_strength', 0, 100],
    'camera-ramp': ['camera', 'ramp_ms', 0, 2000, undefined, 1],
    'timing-long': ['timing', 'long_ms', 200, 2000, 'timing-error', 1],
    'timing-double': ['timing', 'double_ms', 100, 1000, 'timing-error', 1],
    'timing-repeat': ['timing', 'repeat_ms', 30, 1000, 'timing-error', 1],
    'timing-pulse': ['timing', 'pulse_ms', 20, 1000, 'timing-error', 1],
    'detent-idle': ['throttle', 'idle', 0, 100, 'detent-error'],
    'detent-cruise': ['throttle', 'cruise', 0, 100, 'detent-error'],
    'detent-boost': ['throttle', 'boost', 0, 100, 'detent-error'],
    'detent-band': ['throttle', 'band', 0, 20, 'detent-error'],
    'precision-scale': ['precision', 'scale', 10, 100],
    'split-forward': ['split_high_g', 'forward', 0, 100],
    'split-back': ['split_high_g', 'back', 0, 100],
    'head-port': ['head_tracking', 'port', 1024, 65535, 'head-error', 1],
    'head-yaw': ['head_tracking', 'yaw_range', 5, 180, 'head-error', 1],
    'head-pitch': ['head_tracking', 'pitch_range', 5, 90, 'head-error', 1],
    'head-timeout': ['head_tracking', 'timeout_ms', 100, 5000, 'head-error', 1],
    'head-deadzone': ['head', 'deadzone', 0, 50, 'head-error'],
    'head-curve': ['head', 'curve', -100, 100, 'head-error'],
  };

  group(name) {
    if (name === 'head') return this.config.head_tracking.tuning;
    return name === 'axis' ? this.config.axes[this.axis] : name ? this.config[name] : this.config;
  }

  showSettings(config) {
    this.config = config;
    this.capture = null;
    for (const radio of this.doc.querySelectorAll('input[name=throttle-mode]')) radio.checked = radio.value === config.throttle.mode;
    this.$('safe-start').checked = config.safe_start;
    this.$('camera-full').checked = config.camera.full_after_hold;
    this.$('split-reverse').checked = config.split_high_g.reverse;
    for (const [id, key] of HEAD_TOGGLES) this.$(id).checked = config.head_tracking[key];
    this.leverUsers();
    this.paintBindings();
    this.fill();
  }

  /** Controls that share the split levers, from the draft. */
  leverUsers() {
    this.$('split-enabled').checked = this.config.split_high_g.enabled;
    for (const input of this.doc.querySelectorAll('input[name=camera-source]')) input.checked = input.value === (this.config.camera_lever ? 'lever' : 'hat');
    for (const input of this.doc.querySelectorAll('input[name=yaw-source]')) input.checked = input.value === this.config.yaw_source;
  }

  /** Binding selects, invert boxes, the throttle list, the levers, the camera lever and the controller list. */
  paintBindings() {
    const bindings = this.config.bindings;
    for (const [axis] of NEO_AXES) {
      this.$(`bind-${axis}`).value = encode(bindings[axis]);
      this.$(`invert-${axis}`).checked = Boolean(bindings[axis]?.invert);
      this.$(`invert-${axis}`).disabled = !bindings[axis];
    }
    this.$('throttle-add').value = '';
    this.$('throttle-list').replaceChildren(...bindings.throttle.map((binding, index) => {
      const item = this.doc.createElement('li');
      item.innerHTML = `<span></span><label class="tog"><input type="checkbox" data-throttle="${index}" form="profile"><span>Invert</span></label><button class="btn quiet" type="button" data-remove="${index}">Remove</button>`;
      item.firstElementChild.textContent = bindingName({ ...binding, invert: false }, this.names);
      item.querySelector('input').checked = binding.invert;
      return item;
    }));
    [0, 1].forEach((index) => { this.$(`lever-${index}`).value = encode(this.config.split_high_g.levers[index]); });
    const lever = this.config.camera_lever;
    this.$('camera-lever-row').hidden = !lever;
    this.$('camera-lever').value = encode(lever);
    this.$('camera-lever-invert').checked = Boolean(lever?.invert);
    const auto = this.config.devices.length === 0;
    this.$('devices-auto').checked = auto;
    for (const box of this.doc.querySelectorAll('#device-list input')) {
      const device = this.devices.find((d) => d.id === box.dataset.device);
      box.checked = auto ? device.vid === 0x044f : this.config.devices.includes(device.id);
    }
    for (const button of this.doc.querySelectorAll('[data-detect]')) button.setAttribute('aria-busy', String(button.dataset.detect === this.detecting));
  }

  /**
   * Split-lever High-G and lever yaw read the same two levers, and a camera lever on one of them pans with it:
   * turning one on turns the others off, and says so.
   */
  exclusive(chosen) {
    const config = this.config;
    const onSplitLever = () => Boolean(config.camera_lever) && config.split_high_g.levers.some((l) => sameAxis(l, config.camera_lever));
    const off = [
      ['split', 'split-lever High-G', () => { config.split_high_g.enabled = false; }, config.split_high_g.enabled],
      ['camera', 'the camera lever', () => { config.camera_lever = null; }, onSplitLever()],
      ['yaw', 'lever yaw', () => { config.yaw_source = 'twist'; }, config.yaw_source === 'lever_differential'],
    ].filter(([name, , , on]) => name !== chosen && on);
    for (const [, , turnOff] of off) turnOff();
    this.leverUsers();
    this.paintBindings();
    return off.length ? `Unsaved changes: ${off.map(([, text]) => text).join(' and ')} turned off, since it uses those levers too.` : 'Unsaved changes.';
  }

  /** Number fields and sliders from the draft (the axis fields show the selected axis). */
  fill() {
    for (const [id, [group, key, , , , factor = 100]] of Object.entries(Page.FIELDS)) {
      const value = String(Math.round(this.group(group)[key] * factor));
      this.$(id).value = value;
      this.$(id).setAttribute('aria-invalid', 'false');
      this.$(error(id)).textContent = '';
      const range = this.$(`${id}-range`);
      if (range) range.value = value;
    }
    this.renderSettings();
  }

  /** Ticking a controller turns automatic selection into an explicit list of what is ticked. */
  pickDevices() {
    this.config.devices = [...this.doc.querySelectorAll('#device-list input')].filter((box) => box.checked).map((box) => box.dataset.device);
  }

  addThrottle(binding) {
    if (binding && !this.config.bindings.throttle.some((b) => sameAxis(b, binding))) this.config.bindings.throttle.push(binding);
  }

  removeThrottle(index) {
    this.config.bindings.throttle.splice(index, 1);
    this.paintBindings();
    this.changed('Unsaved changes.');
  }

  /** Binding edits: the selects, invert boxes, levers, camera lever and controller list. Undefined for other fields. */
  bind(target) {
    const bindings = this.config.bindings;
    if (target.dataset.axis) bindings[target.dataset.axis] = decode(target.value);
    else if (target.dataset.invert) bindings[target.dataset.invert].invert = target.checked;
    else if (target.id === 'throttle-add') this.addThrottle(decode(target.value));
    else if (target.dataset.throttle) bindings.throttle[Number(target.dataset.throttle)].invert = target.checked;
    else if (target.id === 'lever-0' || target.id === 'lever-1') this.setLever(Number(target.id.slice(6)), decode(target.value));
    else if (target.id === 'camera-lever') this.config.camera_lever = decode(target.value, this.$('camera-lever-invert').checked);
    else if (target.id === 'camera-lever-invert') this.config.camera_lever.invert = target.checked;
    else if (target.id === 'split-reverse') this.config.split_high_g.reverse = target.checked;
    else if (target.id === 'devices-auto') this.config.devices = target.checked ? [] : this.active.map((d) => d.id);
    else if (target.dataset.device) this.pickDevices();
    else return undefined;
    this.paintBindings();
    return this.changed('Unsaved changes.');
  }

  /** One split lever; both are kept or neither, so a half-set pair stays in the selects until the second is picked. */
  setLever(index, binding) {
    const levers = [0, 1].map((i) => (i === index ? binding : this.config.split_high_g.levers[i] ?? null));
    this.config.split_high_g.levers = levers;
  }

  edit(target) {
    const name = target.name || target.id;
    if (name === 'axis') {
      this.axis = target.value;
      return this.fill();
    }
    if (name === 'test-input' || name === 'test-lever') {
      if (name === 'test-input') this.testInput = Number(target.value) / 100;
      else this.testLever = Number(target.value) / 100;
      return this.renderSettings();
    }
    if (name === 'try-live') return this.setPreview(target.checked);
    const bound = this.bind(target);
    if (bound !== undefined) return bound;
    let text = 'Unsaved changes.';
    if (name === 'throttle-mode') this.config.throttle.mode = target.value;
    else if (name === 'safe-start') this.config.safe_start = target.checked;
    else if (name === 'camera-full') this.config.camera.full_after_hold = target.checked;
    else if (name.startsWith('head-') && target.type === 'checkbox') this.config.head_tracking[HEAD_TOGGLES.find(([id]) => id === name)[1]] = target.checked;
    else if (name === 'split-enabled') {
      this.config.split_high_g.enabled = target.checked;
      if (target.checked) text = this.exclusive('split');
    } else if (name === 'camera-source') {
      // A camera lever starts on the first axis that is neither a flight axis nor the throttle; Detect or the select changes it.
      const used = [...NEO_AXES.map(([axis]) => this.config.bindings[axis]), ...this.config.bindings.throttle].filter(Boolean);
      const spare = this.active.flatMap((d) => d.layout.axes.map((a) => ({ device: d.id, axis: a.name, invert: false }))).find((b) => !used.some((u) => sameAxis(u, b)));
      this.config.camera_lever = target.value === 'lever' ? spare ?? null : null;
      this.paintBindings();
      if (this.config.camera_lever) text = this.exclusive('camera');
    } else if (name === 'yaw-source') {
      this.config.yaw_source = target.value;
      if (target.value === 'lever_differential') text = this.exclusive('yaw');
    } else {
      // A number field or its slider: keep the pair together and take the value into the draft when it is a number.
      const id = name.endsWith('-range') ? name.slice(0, -6) : name;
      const partner = this.$(name.endsWith('-range') ? id : `${id}-range`);
      if (partner) partner.value = target.value;
      if (target.value !== '' && Number.isFinite(Number(target.value))) {
        const [group, key, , , , factor = 100] = Page.FIELDS[id];
        this.group(group)[key] = Number(target.value) / factor;
      }
    }
    return this.changed(text);
  }

  /** The draft changed: mark it, redraw the previews, and send it to the game while trying it there. */
  changed(text) {
    this.mark(true, this.previewing ? 'Unsaved changes · running in game now.' : text);
    this.renderSettings();
    return this.pushPreview();
  }

  mark(dirty, text) {
    this.dirty = dirty;
    this.$('dirty').textContent = text;
    this.$('dirty').dataset.state = dirty ? 'dirty' : 'saved';
  }

  /** Check every visible field and the cross-field rules; flag what is wrong. */
  validate() {
    let valid = true;
    const fail = (id, text) => {
      this.$(id).setAttribute('aria-invalid', 'true');
      this.$(error(id)).textContent = text;
      valid = false;
    };
    // Clear everything first: some fields share one error message.
    for (const id of Object.keys(Page.FIELDS)) {
      this.$(id).setAttribute('aria-invalid', 'false');
      this.$(error(id)).textContent = '';
    }
    for (const [id, [, , minimum, maximum]] of Object.entries(Page.FIELDS)) {
      const field = this.$(id);
      const value = Number(field.value);
      if (field.value === '' || !(value >= minimum && value <= maximum)) fail(id, `Enter a number from ${minimum} to ${maximum}.`);
    }
    const value = (id) => Number(this.$(id).value);
    if (valid && value('tune-saturation') <= value('tune-deadzone')) fail('tune-saturation', 'Must be beyond the deadzone.');
    if (valid && value('timing-repeat') <= value('timing-pulse')) fail('timing-repeat', 'Repeat every must be longer than the pulse length.');
    if (valid && (value('detent-idle') >= value('detent-cruise') - value('detent-band') || value('detent-cruise') + value('detent-band') >= value('detent-boost'))) {
      fail('detent-cruise', 'Needs idle < cruise − band and cruise + band < boost.');
    }
    if (valid && value('split-back') >= value('split-forward')) fail('split-back', 'Must be below the forward lever threshold.');
    const split = this.config.split_high_g;
    const pair = split.levers.filter(Boolean);
    if (valid && pair.length !== 2 && (pair.length || split.enabled || this.config.yaw_source === 'lever_differential')) {
      fail('split-forward', 'Pick both levers for split High-G or lever yaw, or turn them off.');
    }
    if (!valid) {
      this.$('dirty').textContent = 'Fix the highlighted fields, then save.';
      this.$('dirty').dataset.state = 'error';
      return false;
    }
    // What is saved or previewed is exactly what the fields show; a half-set lever pair is no pair, and a lever that
    // also feeds the throttle reads with the throttle's inversion.
    for (const [id, [group, key, , , , factor = 100]] of Object.entries(Page.FIELDS)) this.group(group)[key] = value(id) / factor;
    split.levers = pair.map((lever) => ({ ...lever, invert: this.config.bindings.throttle.find((b) => sameAxis(b, lever))?.invert ?? lever.invert }));
    return true;
  }

  async save() {
    if (!this.validate()) return;
    const next = structuredClone(this.config);
    const devicesChanged = next.devices.join() !== this.full.devices.join();
    this.$('save').disabled = true;
    this.$('dirty').textContent = 'Saving…';
    try {
      const result = await this.commit(next);
      this.previewing = false;
      this.mark(false, result.running ? 'Saved. Applied to the running emulation.' : 'Saved.');
      if (devicesChanged) await this.refresh();
      this.render();
    } catch (error) {
      this.$('dirty').textContent = `Not saved: ${error.message}`;
      this.$('dirty').dataset.state = 'error';
    } finally {
      this.$('save').disabled = false;
      this.renderSettings();
    }
  }

  async discard() {
    if (this.previewing) await this.setPreview(false);
    this.showSettings(structuredClone(this.saved));
    this.mark(false, 'Changes discarded.');
  }

  /** "Try in game": start or end running the draft in the emulation. */
  async setPreview(on) {
    if (on && !this.validate()) {
      this.$('try-live').checked = false;
      return;
    }
    this.previewing = on;
    if (on) {
      await this.pushPreview();
    } else {
      try {
        await this.api.del('/api/preview');
      } catch (error) {
        this.$('dirty').textContent = `Preview not ended: ${error.message}`;
      }
    }
    this.renderSettings();
  }

  /** Send the draft while previewing; drags coalesce into one request in flight plus the latest. */
  async pushPreview() {
    if (!this.previewing || !this.validate()) return;
    if (this.sending) {
      this.queued = true;
      return;
    }
    this.sending = true;
    try {
      await this.api.put('/api/preview', compose(this.full, this.config, this.setup));
      this.mark(true, 'Unsaved changes · running in game now.');
    } catch (error) {
      this.previewing = false;
      this.$('dirty').textContent = `Preview stopped: ${error.message}`;
      this.$('dirty').dataset.state = 'error';
    }
    this.sending = false;
    this.renderSettings();
    if (this.queued) {
      this.queued = false;
      await this.pushPreview();
    }
  }

  startCapture(kind) {
    this.capture = this.capture?.kind === kind ? null : { kind, keys: [] };
    this.renderSettings();
  }

  /** Wait for the next axis that moves (live samples during emulation) and bind it to `target`. */
  detect(target) {
    this.detecting = this.detecting === target ? null : target;
    this.baseline = null;
    this.$('detect-hint').textContent = this.detecting ? 'Move the axis you want, all the way, during emulation…' : 'Detection cancelled.';
    this.paintBindings();
  }

  detected(axes) {
    if (!Object.keys(axes ?? {}).length) {
      this.$('detect-hint').textContent = 'Start emulation from the tray icon, then move the axis.';
      return;
    }
    if (!this.baseline) {
      this.baseline = axes;
      return;
    }
    const binding = moved(this.baseline, axes, this.active);
    if (!binding) return;
    const target = this.detecting;
    if (target === 'throttle') this.addThrottle(binding);
    else if (target === 'camera') this.config.camera_lever = binding;
    else if (target.startsWith('lever-')) this.setLever(Number(target.slice(6)), binding);
    else this.config.bindings[target] = binding;
    this.detecting = null;
    this.$('detect-hint').textContent = `Detected ${bindingName(binding, this.names)}. Save settings to apply.`;
    this.paintBindings();
    this.changed('Unsaved changes.');
  }

  /** Throttle travel shown on the throttle page: the live throttle while emulating, else the test lever. */
  lever() {
    return this.monitor?.throttle ?? this.testLever;
  }

  /**
   * Previews from the unsaved draft: curve, readouts, live strip, throttle, camera, helpers. Live input is the
   * default: while emulating every preview follows the device, and the test controls appear only without it.
   */
  renderSettings() {
    const live = Boolean(this.monitor);
    const tuning = this.config.axes[this.axis];
    const input = (axis) => (live ? this.monitor[axis] : axis === this.axis ? this.testInput : 0);
    const name = AXIS_NAMES[this.axis];
    for (const radio of this.doc.querySelectorAll('input[name=axis]')) radio.checked = radio.value === this.axis;
    for (const chip of this.doc.querySelectorAll('[data-livechip]')) {
      chip.dataset.live = String(live);
      chip.lastElementChild.textContent = live ? 'Live input' : 'Idle';
    }
    this.$('rail-live').dataset.live = String(live);
    this.$('rail-live-text').textContent = live ? 'Live input' : 'Idle';
    this.$('test-input-wrap').hidden = live;
    this.$('test-lever-wrap').hidden = live;
    this.$('curve-name').textContent = name;
    this.$('tune-name').textContent = name;
    this.$('curve').setAttribute('aria-label', `${name} response curve: stick input against NEO output`);
    this.$('curve-band').setAttribute('x', plot(-tuning.deadzone).toFixed(1));
    this.$('curve-band').setAttribute('width', (220 * tuning.deadzone).toFixed(1));
    this.$('curve-path').setAttribute('d', curvePath(tuning));
    const [x, y] = [input(this.axis), shape(input(this.axis), tuning)];
    for (const [attribute, value] of [['x1', plot(x)], ['x2', plot(x)], ['y2', 240 - plot(y)]]) this.$('curve-drop').setAttribute(attribute, value.toFixed(1));
    this.$('curve-dot').setAttribute('cx', plot(x).toFixed(1));
    this.$('curve-dot').setAttribute('cy', (240 - plot(y)).toFixed(1));
    this.$('read-input-label').textContent = `${name} · ${live ? 'live' : 'test'}`;
    this.$('read-input').textContent = signed(x);
    this.$('read-output').textContent = signed(y);
    this.$('copy-axis').textContent = `Copy ${name.toLowerCase()} to ${this.axis === 'yaw' ? 'roll and pitch' : this.axis === 'roll' ? 'pitch' : 'roll'}`;
    this.$('reset-axis').textContent = `Reset ${name.toLowerCase()}`;
    for (const mini of this.doc.querySelectorAll('.mini')) {
      const axis = mini.dataset.axis;
      const out = shape(input(axis), this.config.axes[axis]);
      mini.dataset.selected = String(axis === this.axis);
      mini.querySelector('[data-out]').textContent = signed(out);
      mini.querySelectorAll('.minibar span').forEach((bar, i) => bars(bar, [input(axis), out][i]));
    }
    const delay = lag(tuning.smoothing);
    this.$('tune-lag').textContent = delay ? `≈ ${delay} ms added delay` : 'No added delay';
    this.renderThrottle();
    this.renderCamera();
    this.renderHead();
    this.renderHelpers();
    this.renderPresses();
    this.levers(this.lastLevers);
    this.$('try-live').checked = this.previewing;
    this.$('pill-preview').hidden = !this.previewing;
  }

  /** Launch-order warnings from the daemon, one alert each; the banner hides when there are none. */
  warnings(list = []) {
    const banner = this.$('warnings');
    if (banner.dataset.text === list.join('\n')) return;
    banner.dataset.text = list.join('\n');
    banner.hidden = !list.length;
    banner.replaceChildren(...list.map((text) => {
      const item = this.doc.createElement('p');
      item.className = 'warning';
      item.setAttribute('role', 'alert');
      item.innerHTML = `${WARNING_ICON}<span></span>`;
      item.lastChild.textContent = text;
      return item;
    }));
    // Dismissed until the warnings change.
    const dismiss = this.doc.createElement('button');
    dismiss.type = 'button';
    dismiss.className = 'x';
    dismiss.textContent = 'Dismiss';
    dismiss.setAttribute('aria-label', 'Dismiss warnings');
    dismiss.addEventListener('click', () => { banner.hidden = true; });
    banner.append(dismiss);
  }

  /** Head tracking: whether samples arrive, and where the latest one puts the camera through the draft settings. */
  renderHead() {
    const tracking = this.config.head_tracking;
    const sample = this.monitor?.head ?? null;
    const [state, text] = !tracking.enabled ? ['off', 'Off']
      : sample ? ['ok', 'Receiving from OpenTrack']
        : this.emulating ? ['wait', `Waiting for OpenTrack on UDP port ${tracking.port}…`]
          : ['wait', 'Starts with emulation'];
    this.$('head-status').dataset.state = state;
    this.$('head-status-text').textContent = text;
    this.$('head-port-step').textContent = String(tracking.port);
    const band = 80 * tracking.tuning.deadzone;
    for (const [attribute, value] of [['x', 90 - band], ['y', 90 - band], ['width', 2 * band], ['height', 2 * band]]) this.$('head-band').setAttribute(attribute, value.toFixed(1));
    const [x, y] = sample ? headCamera(sample, tracking) : [0, 0];
    this.$('head-dot').setAttribute('cx', (90 + x * 80).toFixed(1));
    this.$('head-dot').setAttribute('cy', (90 + y * 80).toFixed(1));
    this.$('head-view').setAttribute('aria-label', sample ? `Camera at ${signed(x)} across, ${signed(y)} down` : 'No head-tracking data');
    const degrees = (value) => `${value > 0 ? '+' : value < 0 ? '−' : ''}${Math.abs(Math.round(value))}°`;
    this.$('head-yaw-read').textContent = sample ? degrees(sample[0]) : '—';
    this.$('head-pitch-read').textContent = sample ? degrees(sample[1]) : '—';
    this.$('head-camera-read').textContent = sample ? `${signed(x)} / ${signed(y)}` : '—';
  }

  /** Camera ramp graph and help, and the yaw source help, from the draft. */
  renderCamera() {
    const { camera, timing } = this.config;
    const strength = this.config.camera_strength;
    const long = rampX(timing.long_ms);
    this.$('ramp-path').setAttribute('d', rampPath(camera, strength, timing.long_ms));
    for (const [attribute, value] of [['x1', long], ['x2', long]]) this.$('ramp-long').setAttribute(attribute, value.toFixed(1));
    this.$('ramp-long-text').setAttribute('x', (long + 4).toFixed(1));
    for (const id of ['ramp-long', 'ramp-long-text']) this.$(id).style.setProperty('display', camera.full_after_hold ? '' : 'none');
    // The current hat hold, drawn on the graph as it happens, and the camera output the NEO receives.
    const holding = this.hatSince !== null;
    const held = holding ? this.clock() - this.hatSince : 0;
    const hold = rampX(held);
    this.$('ramp-hold').setAttribute('width', (hold - 10).toFixed(1));
    for (const attribute of ['x1', 'x2']) this.$('ramp-now').setAttribute(attribute, hold.toFixed(1));
    this.$('ramp-now').style.setProperty('display', holding ? '' : 'none');
    if (this.saved.hat === 'mapped') tone(this.$('hat-state'), 'Hat mapped as buttons');
    else tone(this.$('hat-state'), holding ? `Hat held · ${held} ms` : 'Hat released', holding ? 'ok' : '');
    for (const [id, index] of [['cam-x', 3], ['cam-y', 4]]) {
      const value = this.monitor ? axisValue(this.monitor.axes[index]) : 0;
      bars(this.$(id), value);
      this.$(`${id}-text`).textContent = signed(value);
    }
    this.$('ramp').setAttribute('aria-label', `Camera output while the hat is held: ${camera.ramp_ms ? `ramps to ${Math.round(strength * 100)}% over ${camera.ramp_ms} ms` : `${Math.round(strength * 100)}% at once`}${camera.full_after_hold ? `, full after ${timing.long_ms} ms` : ''}.`);
    const lever = this.config.camera_lever;
    this.$('camera-source-help').textContent = lever
      ? `${bindingName({ ...lever, invert: false }, this.names)} pans the camera left and right; centre it to look ahead. The hat still works and wins while pressed.`
      : 'Only the hat moves the camera.';
    this.$('yaw-help').textContent = {
      twist: 'The bound yaw axis (the twist on most sticks) steers the rudder, through the yaw curve.',
      off: 'No yaw input: useful if the twist is too easy to knock in combat.',
      lever_differential: 'Pushing one split lever ahead of the other yaws towards the back lever, through the yaw curve. The throttle follows the average of both levers.',
    }[this.config.yaw_source];
  }

  renderThrottle() {
    const throttle = this.config.throttle;
    const detents = throttle.mode === 'detents';
    const travel = this.lever();
    const out = throttleOut(travel, throttle);
    const [area, text] = zone(out, throttle);
    this.$('throttle-help').textContent = detents
      ? 'ACE COMBAT throttle: centre cruises, forward boosts, back brakes. Your throttle detents mark where each band starts.'
      : 'Throttle travel maps straight to the NEO throttle. Mid-travel is cruise.';
    this.$('throttle-source').textContent = `Throttle · ${this.monitor ? 'live' : 'test'}`;
    const zoneLabel = this.$('throttle-zone');
    zoneLabel.textContent = text;
    if (area) zoneLabel.dataset.zone = area; else delete zoneLabel.dataset.zone;
    const rail = this.$('throttle-rail');
    rail.dataset.mode = throttle.mode;
    const at = (value) => `${Math.round(value * 1000) / 10}%`;
    for (const [property, value] of [['--idle', throttle.idle], ['--low', throttle.cruise - throttle.band], ['--high', throttle.cruise + throttle.band], ['--boost', throttle.boost]]) {
      rail.style.setProperty(property, at(value));
    }
    for (const mark of this.doc.querySelectorAll('[data-mark]')) mark.style.setProperty('--at', at(throttle[mark.dataset.mark]));
    this.doc.querySelector('.marks').dataset.mode = throttle.mode;
    this.$('throttle-needle').style.setProperty('left', at(travel));
    this.$('test-lever').value = String(Math.round(this.testLever * 100));
    this.$('throttle-out').textContent = `${signed(out)} · ${out < 0 ? 'brake' : out > 0 ? 'boost' : 'cruise'}`;
    const bar = this.$('throttle-bar');
    bar.dataset.side = out < 0 ? 'brake' : 'boost';
    bars(bar, out);
    this.$('detents').hidden = !detents;
    this.$('detent-legend').hidden = !detents;
    const capture = this.monitor?.throttle !== undefined && this.monitor?.throttle !== null;
    for (const button of this.doc.querySelectorAll('[data-capture]')) button.disabled = !capture;
    this.$('capture-hint').textContent = capture ? 'Put the throttle in a detent, then Capture.' : 'Start emulation to capture the throttle in its detents.';
  }

  /** The precision or shift control of the draft. */
  single(kind) {
    return kind === 'precision' ? this.config.precision.source : this.config.shift;
  }

  setSingle(kind, key) {
    if (kind === 'precision') this.config.precision.source = key;
    else this.config.shift = key;
  }

  renderHelpers() {
    const capturing = this.capture?.kind;
    for (const kind of ['precision', 'shift']) {
      const key = this.single(kind);
      const chip = this.$(`${kind}-source`);
      chip.textContent = key ? `${short(key)} · ${this.label(key)}` : 'Off';
      chip.classList.toggle('off', !key);
      this.$(`${kind}-clear`).disabled = !key;
      this.$(`${kind}-pick`).textContent = capturing === kind ? 'Cancel' : 'Press a control…';
    }
    for (const [kind, field] of COMBOS) {
      this.$(`${kind}-record`).textContent = capturing === kind ? 'Done' : 'Record combo…';
      this.$(`${kind}-clear`).disabled = !this.config.combos[field].length;
      const keys = capturing === kind ? this.capture.keys : this.config.combos[field];
      const combo = this.$(`${kind}-combo`);
      // Rebuilt only when the combo changes: this runs on every live poll.
      if (combo.dataset.keys !== keys.join()) {
        combo.dataset.keys = keys.join();
        combo.replaceChildren(...(keys.length ? keys : [null]).flatMap((key, index) => {
          const chipItem = this.doc.createElement('span');
          chipItem.className = key ? 'chip static' : 'chip static off';
          chipItem.textContent = key ? short(key) : 'Off';
          if (key) {
            chipItem.title = this.label(key);
            chipItem.dataset.key = key;
          }
          if (!index) return [chipItem];
          const plus = this.doc.createElement('span');
          plus.className = 'plus';
          plus.textContent = '+';
          return [plus, chipItem];
        }));
      }
      for (const chip of combo.querySelectorAll('[data-key]')) chip.classList.toggle('held', this.held.has(chip.dataset.key));
    }
    this.renderStates();
    this.renderCapture(capturing);
  }

  /** Helper states from the running emulation: precision and shift held, safe start armed, combo progress. */
  renderStates() {
    const monitor = this.monitor;
    const { precision, shift, safe_start: safe, combos } = this.config;
    if (!precision.source) tone(this.$('precision-state'), 'Precision off');
    else tone(this.$('precision-state'), monitor?.precision ? `Precision held · ${Math.round(precision.scale * 100)}%` : 'Ready', monitor?.precision ? 'accent' : '');
    if (!shift) tone(this.$('shift-state'), 'Shift off');
    else tone(this.$('shift-state'), monitor?.shift ? 'Shift held' : 'Ready', monitor?.shift ? 'warn' : '');
    if (!safe) tone(this.$('safe-state'), 'Off');
    else if (!monitor) tone(this.$('safe-state'), 'Starts with emulation');
    else tone(this.$('safe-state'), monitor.armed ? 'Throttle armed' : 'Waiting for centre', monitor.armed ? 'ok' : 'warn');
    const progress = COMBOS.map(([, field, text]) => [text, combos[field], combos[field].filter((key) => this.held.has(key)).length]).filter(([, keys, held]) => keys.length && held);
    const full = progress.find(([, keys, held]) => held === keys.length);
    if (full) tone(this.$('combo-state'), `${full[0][0].toUpperCase()}${full[0].slice(1)} combo held`, 'ok');
    else if (progress.length) tone(this.$('combo-state'), progress.map(([text, keys, held]) => `${text} ${held}/${keys.length}`).join(' · '), 'warn');
    else tone(this.$('combo-state'), 'No combo held');
  }

  /** The last few seconds of presses of the latest control, and how they read against the timing draft. */
  renderPresses() {
    const now = this.clock();
    const timing = this.config.timing;
    const latest = this.presses.at(-1);
    const mine = latest ? this.presses.filter((press) => press.key === latest.key) : [];
    const at = (t) => Math.max(0, Math.min(100, ((t - now + PRESS_SPAN) / PRESS_SPAN) * 100));
    const span = (className, from, to) => {
      const item = this.doc.createElement('span');
      item.className = className;
      item.style.setProperty('left', `${at(from)}%`);
      item.style.setProperty('width', `${at(to) - at(from)}%`);
      return item;
    };
    const reads = gestures(mine, timing, now);
    const mark = latest && latest.start + timing.long_ms <= now ? [span('mk', latest.start + timing.long_ms, latest.start + timing.long_ms)] : [];
    this.$('press-in').replaceChildren(...mine.map((press) => span('in', press.start, press.end ?? now)), ...mark);
    this.$('press-out').replaceChildren(...reads.map((read) => span(read.kind === 'tap' ? 'out' : 'out alt', read.from, Math.min(read.to, now))));
    this.$('press-key').textContent = latest ? short(latest.key) : 'Pressed';
    let text = 'Press a control: the last 3 seconds are drawn against the long-press and double-tap times.';
    if (latest) {
      const held = (latest.end ?? now) - latest.start;
      const kind = latest.end === null && held < timing.long_ms ? 'holding' : reads.at(-1).kind;
      text = `${short(latest.key)} · ${{
        holding: `holding ${held} ms…`,
        long: `long press: held ${held} ms (long press from ${timing.long_ms} ms)`,
        double: `double tap: second tap within ${timing.double_ms} ms`,
        tap: `tap: ${held} ms`,
      }[kind]}`;
    }
    this.$('press-text').textContent = text;
    this.$('presses').setAttribute('aria-label', text);
  }

  renderCapture(capturing) {
    // The prompt sits in the card being captured for.
    for (const well of this.doc.querySelectorAll('.capture')) {
      well.hidden = well.dataset.for !== capturing;
      well.lastElementChild.textContent = !this.emulating ? 'Start emulation, then press the controls on your HOTAS.' : {
        precision: 'Press the control to hold for precision aiming.',
        shift: 'Press the control to hold for the shift layer. Its own mapping is not sent while it is the shift control.',
        pause: 'Press and release the controls for the combo (up to 4), then choose Done.',
        next: 'Press and release the controls for the combo (up to 4), then choose Done.',
      }[well.dataset.for];
    }
  }

  /** Lever rails against the draft thresholds, so they can be tuned before saving. */
  levers(levers) {
    const rule = this.config.split_high_g;
    const names = [0, 1].map((i) => (rule.levers[i] ? leverName(rule.levers[i], this.names) : ['First lever', 'Second lever'][i]));
    names.forEach((name, i) => { this.$(`lever-name-${i}`).textContent = name; });
    const rails = this.$('lever-rails');
    rails.hidden = !levers;
    rails.style.setProperty('--fw', `${Math.round(rule.forward * 100)}%`);
    rails.style.setProperty('--bk', `${Math.round(rule.back * 100)}%`);
    const state = this.$('lever-state');
    const active = Boolean(levers) && splitActive(levers, rule);
    state.textContent = !rule.enabled ? 'Rule off' : active ? 'High-G active · NEO 5 held' : 'Not split';
    state.dataset.active = String(active);
    if (!levers) {
      this.$('lever-live').textContent = 'Lever positions appear here during emulation.';
      return;
    }
    this.$('lever-live').textContent = leverText(levers, rule, names);
    levers.forEach((value, i) => {
      this.$(`needle-${i}`).style.setProperty('--pos', `${Math.round(value * 100)}%`);
      const readout = this.$(`value-${i}`);
      readout.textContent = `${Math.round(value * 100)}%`;
      readout.dataset.zone = value >= rule.forward ? 'forward' : value <= rule.back ? 'back' : 'middle';
    });
  }

  /**
   * Live input: light held controls; a new press selects that control unless a recording is being saved, or
   * becomes the precision control / joins the pause combo while one is being picked. Previews follow the raw input.
   */
  live(data) {
    this.status(data.status, data.links);
    this.warnings(data.warnings);
    this.emulating = Boolean(data.links);
    this.monitor = data.monitor ?? null;
    this.$('pill-paused').hidden = !data.paused;
    if (this.previewing && !data.preview && !this.sending) {
      this.previewing = false;
      this.mark(true, 'Unsaved changes · the preview ended with emulation.');
    }
    this.$('live-input-text').textContent = data.pressed.length ? `Now sending: ${data.pressed.map((key) => this.label(key)).join(', ')}` : data.links ? 'Nothing pressed' : 'Start emulation to see presses.';
    this.$('live-input').dataset.active = String(data.pressed.length > 0);
    this.lastLevers = data.levers;
    this.lastLive = data;
    if (this.detecting) this.detected(data.axes);
    if (this.page === 'live') this.renderLive(data);
    const pressed = data.pressed.filter((key) => !this.held.has(key));
    const fresh = pressed.filter((key) => this.selectable(key));
    this.held = new Set(data.pressed);
    // Press history for the timing timeline, and how long the camera hat has been held for the camera ramp.
    const now = this.clock();
    for (const entry of this.presses) if (entry.end === null && !this.held.has(entry.key)) entry.end = now;
    this.presses.push(...pressed.map((key) => ({ key, start: now, end: null })));
    this.presses = this.presses.filter((entry) => entry.end === null || entry.end >= now - PRESS_SPAN);
    const camera = this.camera();
    const hat = this.saved.hat !== 'mapped' && data.pressed.some((key) => key.startsWith(`${camera}/hat-`));
    this.hatSince = hat ? this.hatSince ?? now : null;
    const kind = this.capture?.kind;
    if ((kind === 'precision' || kind === 'shift') && fresh.length) {
      this.setSingle(kind, fresh[0]);
      this.capture = null;
      this.changed(`Unsaved: hold ${this.label(fresh[0])} for ${kind === 'precision' ? 'precision aiming' : 'the shift layer'}.`);
    } else if (kind === 'pause' || kind === 'next') {
      this.capture.keys = [...new Set([...this.capture.keys, ...fresh])].slice(0, 4);
    }
    if (fresh.length && this.page === 'mapping' && !this.busy) this.select(fresh[0]);
    else this.render();
    this.renderSettings();
  }

  async poll() {
    try {
      this.live(await this.api.get('/api/live'));
    } catch {
      this.status('unreachable. Reopen Configure from the tray icon.');
    }
  }
}

export async function start(win, interval = 100) {
  const page = new Page(win.document, client(win.fetch.bind(win), win.location.hash.slice(1)));
  try {
    await page.load();
  } catch (error) {
    page.status(`unavailable (${error.message})`);
    return page;
  }
  page.timer = win.setInterval(() => page.poll(), interval);
  return page;
}

if (globalThis.document?.getElementById('neo-app')) start(globalThis.window);
