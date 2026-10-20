// NEO daemon configuration page. The token comes from the URL fragment, so it never appears in request lines.
export const COUNTS = { s: 17, q: 31 };

/** AC8 actions for native NEO buttons (official AC8 PC mapping); unlisted buttons have no documented action. */
export const NEO_ACTIONS = { 1: 'Gun', 2: 'Missile', 3: 'Weapon select', 4: 'Target select', 5: 'High-G', 6: 'Autopilot', 7: 'Flares', 8: 'View', 9: 'Radar', 10: 'Gear', 12: 'Pause / menu' };

/**
 * Diagram hotspots: [key, x, y, diameter] in each SVG's viewBox units. They sit on the numbered callouts.
 * Sidestick: viewBox -76 -8 390 340 (left-hand setup mirrors 5–10 and 11–16). Quadrant: 533 × 364 (two
 * callouts read 3). NEO: 606 × 726; button 16 exists physically but not in the NEO report.
 */
export const DIAGRAMS = {
  right: { box: [-76, -8, 390, 340], spots: [[5, -48, 125], [6, -48, 153], [7, -48, 181], [10, -48, 209], [9, -48, 237], [8, -48, 265], [11, 284, 125], [12, 284, 153], [13, 284, 181], [16, 284, 209], [15, 284, 237], [14, 284, 265], [1, -48, 79], [2, -48, 43], [3, -13, 98], [4, 244, 99], [17, 260, 303]].map(([n, x, y]) => [`s${n}`, x, y, 27]) },
  left: { box: [-76, -8, 390, 340], spots: [[11, -48, 125], [12, -48, 153], [13, -48, 181], [16, -48, 209], [15, -48, 237], [14, -48, 265], [5, 284, 125], [6, 284, 153], [7, 284, 181], [10, 284, 209], [9, 284, 237], [8, 284, 265], [1, -48, 79], [2, -48, 43], [3, -13, 98], [4, 244, 99], [17, 260, 303]].map(([n, x, y]) => [`s${n}`, x, y, 27]) },
  quadrant: { box: [0, 0, 533, 364], spots: [['q1', 79, 49, 30], ['q2', 447, 65, 30], ['q3', 473, 153, 30], ['q3', 67, 179, 30], ['q4', 470, 204, 30], ['q5', 58, 218, 30], ['q6', 274, 327, 30], ['q7', 64, 308, 22], ['q8', 86, 308, 22]] },
  neo: { box: [0, 0, 606, 726], spots: [[1, 546, 159], [2, 278, 185], [3, 427, 126], [4, 320, 165], [5, 262, 321], [6, 262, 362], [7, 262, 402], [8, 262, 444], [9, 54, 218], [10, 54, 255], [11, 407, 598], [12, 471, 598], [13, 301, 636], [14, 366, 636], [15, 329, 663], [16, 436, 621]].map(([n, x, y]) => [`b${n}`, x, y, 34]).concat([['pov-up', 199, 104, 18], ['pov-right', 222, 126, 18], ['pov-down', 199, 147, 18], ['pov-left', 176, 126, 18]]) },
};

/** Sidestick hat directions; mappable only when the hat is set to Mapped controls. */
export const HAT_SOURCES = ['hat-up', 'hat-right', 'hat-down', 'hat-left'];

export function sources() {
  return Object.entries(COUNTS).flatMap(([prefix, count]) => Array.from({ length: count }, (_, i) => `${prefix}${i + 1}`));
}

export function label(key) {
  if (key.startsWith('hat-')) return `Sidestick hat ${key.slice(4)}`;
  const n = Number(key.slice(1));
  if (key[0] === 's') return n === 17 ? 'Sidestick input 17' : `Sidestick button ${n}`;
  if (n > 16) return `Quadrant add-on input ${n}`;
  return n > 8 ? `Quadrant detent state ${n}` : `Quadrant button ${n}`;
}

/** Compact source name for tables: "S6", "Q2", "Hat up". */
export function short(key) {
  return key.startsWith('hat-') ? `Hat ${key.slice(4)}` : key[0].toUpperCase() + key.slice(1);
}

export function neoName(id) {
  if (id.startsWith('pov-')) return `NEO POV ${id.slice(4)}`;
  const n = Number(id.slice(1));
  return NEO_ACTIONS[n] ? `NEO ${n} (${NEO_ACTIONS[n]})` : `NEO ${n}`;
}

/** {buttons, pov, mode} → readable text. */
export function describe(mapping) {
  const parts = mapping.buttons.map((n) => neoName(`b${n}`));
  if (mapping.pov) parts.push(neoName(`pov-${mapping.pov}`));
  const text = parts.length ? parts.join(', ') : 'Nothing';
  return mapping.mode && mapping.mode !== 'hold' ? `${text} · ${MODES[mapping.mode].label.toLowerCase()}` : text;
}

/** Source behaviours: name, what the game receives, and the NEO pulses drawn on the timeline ([start%, width%]). */
export const MODES = {
  hold: { label: 'Hold', help: 'The NEO button stays pressed while the control is on. A latched switch keeps it held.', pulses: [[15, 55]] },
  pulse: { label: 'Pulse', help: 'One short press when the control turns on. Good for the mode selector: one flare burst, not a stream.', pulses: [[15, 6]] },
  pulse_change: { label: 'Pulse on change', help: 'One short press each time the control moves, on and off. Turning ENG 1 on and off toggles Autopilot on and off.', pulses: [[15, 6], [70, 6]] },
  toggle: { label: 'Toggle', help: 'Each press turns the NEO button on or off, like a latch for a momentary button.', pulses: [[15, 85]] },
};

export function current(config, key) {
  return { buttons: [...(config.buttons[key] ?? [])].sort((a, b) => a - b), pov: config.pov[key] ?? null, mode: config.behaviours?.[key] ?? 'hold' };
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

/** Detent throttle: lever travel (0 idle, 1 forward) to -1 full brake, 0 cruise, +1 full boost (same as neo::detent). */
export function detent(travel, t) {
  const [low, high] = [t.cruise - t.band, t.cruise + t.band];
  if (travel <= t.idle) return -1;
  if (travel < low) return (travel - t.idle) / (low - t.idle) - 1;
  if (travel <= high) return 0;
  return travel < t.boost ? (travel - high) / (t.boost - high) : 1;
}

/** What the game reads for a lever travel: -1 brake … +1 boost. */
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

/** TCA sources currently mapped to a NEO hotspot. */
export function sourcesFor(config, id) {
  const keys = [...sources(), ...HAT_SOURCES];
  if (id.startsWith('pov-')) return keys.filter((key) => config.pov[key] === id.slice(4));
  const n = Number(id.slice(1));
  return keys.filter((key) => (config.buttons[key] ?? []).includes(n));
}

/** True while the split-lever rule holds NEO 5. */
export function splitActive([left, right], rule) {
  return rule.enabled && Math.max(left, right) >= rule.forward && Math.min(left, right) <= rule.back;
}

/** Live lever readout, flagging an active split against the saved thresholds. */
export function leverText(levers, rule) {
  const [left, right] = levers;
  return `Left lever ${Math.round(left * 100)}% · Right lever ${Math.round(right * 100)}%${splitActive(levers, rule) ? ' · High-G active' : ''}`;
}

/** Header pills from the daemon status and what the running session holds. */
export function pills(status, links) {
  const state = /^Emulating/.test(status) ? 'ok' : /^(Starting|Stopping)/.test(status) ? 'busy' : /^(Error|unreachable|unavailable)/.test(status) ? 'fault' : 'off';
  const validation = links?.source === 'validation';
  return {
    status: state,
    tca: validation ? ['busy', 'Source: validation stream'] : links?.tca ? ['ok', 'TCA: sidestick + quadrant open'] : ['off', 'TCA: not open'],
    hidhide: links?.hidden ? ['ok', 'HidHide: TCA hidden'] : ['off', validation ? 'HidHide: not needed' : 'HidHide: off'],
    neo: links?.virtual_neo ? ['ok', 'Virtual NEO present'] : ['off', 'Virtual NEO: absent'],
  };
}

export function client(fetchImpl, token) {
  const call = async (method, path, body) => {
    const response = await fetchImpl(path, { method, headers: { 'X-Neo-Token': token, 'Content-Type': 'application/json' }, body });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error);
    return data;
  };
  return { get: (path) => call('GET', path), put: (path, value) => call('PUT', path, JSON.stringify(value)), del: (path) => call('DELETE', path) };
}

const SELECTABLE = new Set(sources());
const ARROW = '<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1.5 L8.5 8 L1.5 8 Z"/></svg>';
const HELP = {
  sel: 'Click NEO buttons or a POV arrow to change this control’s mapping, then record it. Changes apply immediately, even while emulating.',
  rev: 'These TCA controls feed it. Select one of them to change its mapping.',
  none: 'Select a TCA control, then click NEO buttons or a POV arrow to build its mapping. With nothing selected, clicking a NEO button shows what feeds it.',
};
const error = (id) => Page.FIELDS[id]?.[4] ?? `${id}-error`;
const percent = (value) => `${Math.round(value * 1000) / 1000}%`;

export class Page {
  constructor(doc, api) {
    this.doc = doc;
    this.api = api;
    this.saved = null;      // last saved profile (mapping edits start from here)
    this.config = null;     // settings draft
    this.defaults = null;
    this.selected = null;   // TCA source key
    this.proposal = null;   // {buttons, pov} being built for the selection
    this.reverse = null;    // NEO hotspot inspected with nothing selected
    this.hand = 'right';
    this.held = new Set();
    this.labels = [];
    this.busy = false;
    this.dirty = false;
    this.axis = 'roll';         // flight axis shown on the settings tab
    this.source = 'test';       // preview input: the test scrubber or the live stick
    this.testInput = 0.4;
    this.testLever = 0.5;
    this.monitor = null;        // raw inputs from the running emulation
    this.capture = null;        // {kind: 'precision' | 'pause', keys} while picking controls by pressing them
    this.previewing = false;    // "Try in game": the draft runs in the emulation without being saved
    this.sending = false;
    this.queued = false;
    this.$ = (id) => doc.getElementById(id);
  }

  async load() {
    const data = await this.api.get('/api/config');
    this.defaults = data.defaults;
    this.saved = structuredClone(data.config);
    if (data.profile) this.$('profile-path').textContent = data.profile;
    this.build();
    this.showSettings(structuredClone(data.preview ?? data.config));
    if (data.preview) {
      this.previewing = true;
      this.mark(true, 'Unsaved changes · running in game now.');
    }
    this.status(data.status, data.links);
    this.render();
    this.renderSettings();
    try {
      this.labels = (await this.api.get('/api/labels')).labels;
      this.renderLabels();
    } catch (error) {
      this.note(`Earlier corrections could not be loaded: ${error.message}`);
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

  tcaControl(control) {
    const name = label(control.dataset.source);
    control.setAttribute('aria-label', name);
    control.setAttribute('aria-pressed', 'false');
    control.title = name;
    control.addEventListener('click', () => this.select(control.dataset.source));
  }

  stickSpots() {
    const container = this.$('stick-spots');
    for (const old of container.querySelectorAll('[data-source]')) old.remove();
    const diagram = DIAGRAMS[this.hand];
    for (const spot of diagram.spots) this.tcaControl(this.spot(container, spot, diagram.box, 'source'));
    const setup = this.hand === 'right' ? 'Right-hand setup' : 'Left-hand setup';
    const image = this.$('stick-img');
    image.src = `device/sidestick-${this.hand}.svg`;
    image.alt = `TCA Sidestick, top view, ${setup.toLowerCase()}, numbered controls 1 to 17`;
    this.$('hand-caption').textContent = setup;
  }

  build() {
    this.stickSpots();
    for (const spot of DIAGRAMS.quadrant.spots) this.tcaControl(this.spot(this.$('quad-spots'), spot, DIAGRAMS.quadrant.box, 'source'));
    const chips = [['extra-hat', HAT_SOURCES], ['extra-detents', sources().filter((k) => /^q(9|1[0-6])$/.test(k))], ['extra-addons', sources().filter((k) => /^q(1[7-9]|2\d|3[01])$/.test(k))]];
    for (const [id, keys] of chips) {
      for (const key of keys) {
        const chip = this.doc.createElement('button');
        chip.type = 'button';
        const hat = key.startsWith('hat-');
        chip.className = hat ? 'chip hat-chip' : 'chip';
        chip.dataset.source = key;
        chip.textContent = hat ? key[4].toUpperCase() + key.slice(5) : key.slice(1);
        this.tcaControl(chip);
        this.$(id).append(chip);
      }
    }
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
    for (const radio of this.doc.querySelectorAll('input[name=hand]')) {
      radio.addEventListener('change', () => { this.hand = radio.value; this.stickSpots(); this.render(); });
    }
    for (const radio of this.doc.querySelectorAll('input[name=hat]')) radio.addEventListener('change', () => this.setHat(radio.value));
    this.$('record').addEventListener('click', () => this.record());
    this.$('revert').addEventListener('click', () => { this.proposal = current(this.saved, this.selected); this.render(); });
    this.$('deselect').addEventListener('click', () => { this.selected = null; this.proposal = null; this.reverse = null; this.render(); });
    for (const tab of this.doc.querySelectorAll('[role=tab]')) {
      tab.addEventListener('click', () => this.tab(tab.id));
      tab.addEventListener('keydown', (event) => {
        if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') this.tab(tab.id === 'tab-mapping' ? 'tab-settings' : 'tab-mapping', true);
      });
    }
    for (const radio of this.doc.querySelectorAll('input[name=behaviour]')) {
      radio.addEventListener('change', () => { this.proposal.mode = radio.value; this.render(); });
    }
    this.$('profile').addEventListener('input', (event) => this.edit(event.target));
    this.$('profile').addEventListener('submit', (event) => { event.preventDefault(); return this.save(); });
    this.$('reset').addEventListener('click', () => {
      this.showSettings(structuredClone(this.defaults));
      this.changed('Unsaved: defaults, including every mapping. Save settings to apply.');
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
    this.$('precision-pick').addEventListener('click', () => this.startCapture('precision'));
    this.$('precision-clear').addEventListener('click', () => {
      this.config.precision.source = null;
      this.changed('Unsaved: precision aiming off.');
    });
    this.$('pause-record').addEventListener('click', () => {
      if (this.capture?.kind !== 'pause') return this.startCapture('pause');
      this.config.combos.pause = this.capture.keys;
      this.capture = null;
      return this.changed(this.config.combos.pause.length ? 'Unsaved: new pause combo.' : 'Unsaved: pause combo off.');
    });
    this.$('pause-clear').addEventListener('click', () => {
      this.config.combos.pause = [];
      this.changed('Unsaved: pause combo off.');
    });
  }

  tab(id, focus = false) {
    for (const tab of this.doc.querySelectorAll('[role=tab]')) {
      const active = tab.id === id;
      tab.setAttribute('aria-selected', String(active));
      tab.tabIndex = active ? 0 : -1;
      this.$(tab.getAttribute('aria-controls')).hidden = !active;
      if (active && focus) tab.focus();
    }
    // A preview belongs to the settings tab: leaving it ends the preview, keeping the draft.
    if (id === 'tab-mapping' && this.previewing) {
      this.setPreview(false).then(() => this.mark(true, 'Unsaved changes · preview ended when you left this tab.'));
    }
  }

  select(key) {
    this.selected = key;
    this.proposal = current(this.saved, key);
    this.reverse = null;
    this.render();
  }

  /** Hat mode is part of the profile: save it at once, like a recorded mapping. */
  async setHat(mode) {
    const next = { ...structuredClone(this.saved), hat: mode };
    this.busy = true;
    try {
      const result = await this.api.put('/api/config', next);
      this.saved = next;
      this.config.hat = mode;
      if (mode === 'camera' && this.selected?.startsWith('hat-')) { this.selected = null; this.proposal = null; }
      const applied = result.running ? ' Applied to the running emulation.' : '';
      this.note(mode === 'mapped'
        ? `The hat now sends its mapped controls; the camera mini-stick gets no input.${applied}`
        : `The hat drives the camera again; hat mappings are kept but inactive.${applied}`);
    } catch (error) {
      this.note(`Not saved: ${error.message}`);
    }
    this.busy = false;
    this.render();
  }

  selectable(key) {
    return SELECTABLE.has(key) || (HAT_SOURCES.includes(key) && this.saved.hat === 'mapped');
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
    const now = this.selected ? current(this.saved, this.selected) : null;
    const feeds = this.reverse ? sourcesFor(this.saved, this.reverse) : [];
    const mode = now ? 'sel' : this.reverse ? 'rev' : 'none';
    for (const control of this.doc.querySelectorAll('[data-source]')) {
      const key = control.dataset.source;
      control.setAttribute('aria-pressed', String(key === this.selected));
      control.classList.toggle('is-feed', feeds.includes(key));
      control.classList.toggle('held', this.held.has(key));
    }
    const hatMapped = this.saved.hat === 'mapped';
    for (const radio of this.doc.querySelectorAll('input[name=hat]')) radio.checked = radio.value === (hatMapped ? 'mapped' : 'camera');
    this.$('hat-hint').textContent = hatMapped ? 'Each direction maps like a button; the camera gets no input.' : 'Pans the camera through the NEO mini-stick (Rx/Ry).';
    for (const chip of this.doc.querySelectorAll('.hat-chip')) {
      chip.disabled = !hatMapped;
      chip.title = hatMapped ? label(chip.dataset.source) : 'Set the hat to Mapped controls to map it';
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
        ? HAT_SOURCES.map((hat) => hat.slice(4)).map((d) => [d, sourcesFor(this.saved, `pov-${d}`)]).filter(([, keys]) => keys.length).map(([d, keys]) => `${d[0].toUpperCase()}: ${keys.map(short).join(' · ')}`).join('  ')
        : sourcesFor(this.saved, id).map(short).join(' · ');
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
    this.$('selected-kind').textContent = { sel: 'TCA source', rev: 'NEO target', none: 'Selection' }[mode];
    this.$('link-help').textContent = HELP[mode];
    this.$('compare').hidden = mode === 'none';
    this.$('mapped-new-label').hidden = mode !== 'sel';
    this.$('mapped-new').hidden = mode !== 'sel';
    this.$('mapped-now-label').textContent = mode === 'rev' ? 'Fed by' : 'Mapped now';
    if (now) {
      this.$('selected-name').textContent = label(this.selected);
      this.$('mapped-now').textContent = describe(now);
      this.$('mapped-new').textContent = changed ? describe(this.proposal) : 'Unchanged';
      this.$('neo-context-text').textContent = `Mapping of ${label(this.selected)} → ${describe(this.proposal)}`;
    } else if (this.reverse) {
      this.$('selected-name').textContent = neoName(this.reverse);
      this.$('mapped-now').textContent = feeds.length ? feeds.map(label).join(', ') : 'No TCA control';
      this.$('neo-context-text').textContent = `Inspecting ${neoName(this.reverse)} · fed by ${feeds.length ? feeds.map(short).join(' · ') : 'nothing'}`;
    } else {
      this.$('selected-name').textContent = 'Nothing selected';
      this.$('mapped-now').textContent = '—';
      this.$('neo-context-text').textContent = 'Select a TCA control to light up its NEO targets';
    }
    const list = this.$('diff');
    list.replaceChildren(...(now ? diff(now, this.proposal) : []).map(([kind, text]) => {
      const item = this.doc.createElement('li');
      item.className = kind;
      item.textContent = text;
      return item;
    }));
    this.$('behaviour').hidden = !now;
    if (now) {
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

  renderAxes() {
    const lever = this.saved.throttle_lever === 'right' ? 'Right' : 'Left';
    const rows = [
      ['X / Y', 'Sidestick stick', 'Roll / pitch'],
      ['Rz', 'Sidestick twist', 'Yaw'],
      ['Z', `${lever} quadrant lever or sidestick slider`, 'Throttle'],
      ['Rx / Ry', this.saved.hat === 'mapped' ? 'No source (hat is mapped)' : 'Sidestick hat', 'Camera'],
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
    const before = current(this.saved, key);
    const after = structuredClone(this.proposal);
    const next = structuredClone(this.saved);
    if (after.buttons.length) next.buttons[key] = after.buttons; else delete next.buttons[key];
    if (after.pov) next.pov[key] = after.pov; else delete next.pov[key];
    next.behaviours = { ...next.behaviours };
    if (after.mode !== 'hold') next.behaviours[key] = after.mode; else delete next.behaviours[key];
    this.busy = true;
    this.render();
    let result;
    try {
      result = await this.api.put('/api/config', next);
    } catch (error) {
      this.note(`Not saved: ${error.message}`);
      this.busy = false;
      this.render();
      return;
    }
    this.saved = next;
    this.config.buttons = structuredClone(next.buttons);
    this.config.pov = structuredClone(next.pov);
    this.config.behaviours = structuredClone(next.behaviours);
    const entry = { label: label(key), sources: [key], mapping: `${describe(before)} → ${describe(after)}`.slice(0, 200) };
    const applied = result.running ? ' Applied to the running emulation.' : '';
    try {
      await this.api.put('/api/labels', [...this.labels, entry]);
      this.labels = [...this.labels, entry];
      this.renderLabels();
      this.note(`Recorded ${label(key)}: ${describe(after)}.${applied}`);
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
    for (const name of ['tca', 'hidhide', 'neo']) {
      const pill = this.$(`pill-${name}`);
      pill.dataset.state = state[name][0];
      pill.lastElementChild.textContent = state[name][1];
    }
  }

  /** Settings fields: [where in the profile, key, minimum %, maximum %, error element]. Values are stored as fractions. */
  static FIELDS = {
    'tune-deadzone': ['axis', 'deadzone', 0, 50],
    'tune-saturation': ['axis', 'saturation', 50, 100],
    'tune-anti': ['axis', 'anti_deadzone', 0, 50],
    'tune-curve': ['axis', 'curve', -100, 100],
    camera: [null, 'camera_strength', 0, 100],
    'detent-idle': ['throttle', 'idle', 0, 100, 'detent-error'],
    'detent-cruise': ['throttle', 'cruise', 0, 100, 'detent-error'],
    'detent-boost': ['throttle', 'boost', 0, 100, 'detent-error'],
    'detent-band': ['throttle', 'band', 0, 20, 'detent-error'],
    'precision-scale': ['precision', 'scale', 10, 100],
    'split-forward': ['split_high_g', 'forward', 0, 100],
    'split-back': ['split_high_g', 'back', 0, 100],
  };

  group(name) {
    return name === 'axis' ? this.config.axes[this.axis] : name ? this.config[name] : this.config;
  }

  showSettings(config) {
    this.config = config;
    this.capture = null;
    for (const radio of this.doc.querySelectorAll('input[name=lever]')) radio.checked = radio.value === config.throttle_lever;
    for (const radio of this.doc.querySelectorAll('input[name=throttle-mode]')) radio.checked = radio.value === config.throttle.mode;
    for (const axis of ['roll', 'pitch', 'yaw', 'throttle']) this.$(`invert-${axis}`).checked = config.invert[axis];
    this.$('split-enabled').checked = config.split_high_g.enabled;
    this.$('safe-start').checked = config.safe_start;
    this.fill();
  }

  /** Number fields and sliders from the draft (the axis fields show the selected axis). */
  fill() {
    for (const [id, [group, key]] of Object.entries(Page.FIELDS)) {
      const value = String(Math.round(this.group(group)[key] * 100));
      this.$(id).value = value;
      this.$(id).setAttribute('aria-invalid', 'false');
      this.$(error(id)).textContent = '';
      const range = this.$(`${id}-range`);
      if (range) range.value = value;
    }
    this.renderSettings();
  }

  edit(target) {
    const name = target.name || target.id;
    if (name === 'axis') {
      this.axis = target.value;
      return this.fill();
    }
    if (name === 'preview-source') {
      this.source = target.value;
      return this.renderSettings();
    }
    if (name === 'test-input' || name === 'test-lever') {
      // Dragging a test control previews it instead of the live stick.
      if (name === 'test-input') this.testInput = Number(target.value) / 100;
      else this.testLever = Number(target.value) / 100;
      this.source = 'test';
      this.doc.querySelector('input[name=preview-source][value=test]').checked = true;
      return this.renderSettings();
    }
    if (name === 'try-live') return this.setPreview(target.checked);
    if (name === 'lever') this.config.throttle_lever = target.value;
    else if (name === 'throttle-mode') this.config.throttle.mode = target.value;
    else if (name.startsWith('invert-')) this.config.invert[name.slice(7)] = target.checked;
    else if (name === 'split-enabled') this.config.split_high_g.enabled = target.checked;
    else if (name === 'safe-start') this.config.safe_start = target.checked;
    else {
      // A number field or its slider: keep the pair together and take the value into the draft when it is a number.
      const id = name.endsWith('-range') ? name.slice(0, -6) : name;
      const partner = this.$(name.endsWith('-range') ? id : `${id}-range`);
      if (partner) partner.value = target.value;
      if (target.value !== '' && Number.isFinite(Number(target.value))) {
        const [group, key] = Page.FIELDS[id];
        this.group(group)[key] = Number(target.value) / 100;
      }
    }
    return this.changed('Unsaved changes.');
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
    for (const [id, [, , minimum, maximum]] of Object.entries(Page.FIELDS)) {
      const field = this.$(id);
      const value = Number(field.value);
      field.setAttribute('aria-invalid', 'false');
      this.$(error(id)).textContent = '';
      if (field.value === '' || !(value >= minimum && value <= maximum)) fail(id, `Enter a number from ${minimum} to ${maximum}.`);
    }
    const value = (id) => Number(this.$(id).value);
    if (valid && value('tune-saturation') <= value('tune-deadzone')) fail('tune-saturation', 'Must be beyond the deadzone.');
    if (valid && (value('detent-idle') >= value('detent-cruise') - value('detent-band') || value('detent-cruise') + value('detent-band') >= value('detent-boost'))) {
      fail('detent-cruise', 'Needs idle < cruise − band and cruise + band < boost.');
    }
    if (valid && value('split-back') >= value('split-forward')) fail('split-back', 'Must be below the forward lever threshold.');
    if (!valid) {
      this.$('dirty').textContent = 'Fix the highlighted fields, then save.';
      this.$('dirty').dataset.state = 'error';
      return false;
    }
    // What is saved or previewed is exactly what the fields show.
    for (const [id, [group, key]] of Object.entries(Page.FIELDS)) this.group(group)[key] = value(id) / 100;
    return true;
  }

  async save() {
    if (!this.validate()) return;
    const next = structuredClone(this.config);
    this.$('save').disabled = true;
    this.$('dirty').textContent = 'Saving…';
    try {
      const result = await this.api.put('/api/config', next);
      this.saved = structuredClone(next);
      this.previewing = false;
      this.mark(false, result.running ? 'Saved. Applied to the running emulation.' : 'Saved.');
      this.render();
      this.levers(this.lastLevers);
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
      await this.api.put('/api/preview', structuredClone(this.config));
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

  /** Lever travel shown on the throttle card: the live lever while emulating and selected, else the test lever. */
  lever() {
    const index = this.config.throttle_lever === 'right' ? 1 : 0;
    return this.source === 'live' && this.monitor ? this.monitor.levers[index] : this.testLever;
  }

  /** Previews from the unsaved draft: curve, readouts, live strip, throttle bands and output, helpers. */
  renderSettings() {
    const live = this.source === 'live';
    const tuning = this.config.axes[this.axis];
    const input = (axis) => (live ? (this.monitor ? this.monitor[axis] : 0) : axis === this.axis ? this.testInput : 0);
    const name = AXIS_NAMES[this.axis];
    for (const radio of this.doc.querySelectorAll('input[name=axis]')) radio.checked = radio.value === this.axis;
    this.$('curve').setAttribute('aria-label', `${name} response curve: stick input against NEO output`);
    this.$('curve-band').setAttribute('x', plot(-tuning.deadzone).toFixed(1));
    this.$('curve-band').setAttribute('width', (220 * tuning.deadzone).toFixed(1));
    this.$('curve-path').setAttribute('d', curvePath(tuning));
    const [x, y] = [input(this.axis), shape(input(this.axis), tuning)];
    for (const [attribute, value] of [['x1', plot(x)], ['x2', plot(x)], ['y2', 240 - plot(y)]]) this.$('curve-drop').setAttribute(attribute, value.toFixed(1));
    this.$('curve-dot').setAttribute('cx', plot(x).toFixed(1));
    this.$('curve-dot').setAttribute('cy', (240 - plot(y)).toFixed(1));
    this.$('read-input-label').textContent = `${name} · ${live ? (this.monitor ? 'live' : 'no input yet') : 'test'}`;
    this.$('read-input').textContent = signed(x);
    this.$('read-output').textContent = signed(y);
    this.$('copy-axis').textContent = `Copy ${name.toLowerCase()} to ${this.axis === 'yaw' ? 'roll and pitch' : this.axis === 'roll' ? 'pitch' : 'roll'}`;
    this.$('reset-axis').textContent = `Reset ${name.toLowerCase()}`;
    for (const mini of this.doc.querySelectorAll('.mini')) {
      const axis = mini.dataset.axis;
      const out = shape(input(axis), this.config.axes[axis]);
      mini.dataset.selected = String(axis === this.axis);
      mini.querySelector('[data-out]').textContent = signed(out);
      const bar = mini.querySelector('.minibar span');
      bar.style.setProperty('--from', `${50 + Math.min(out, 0) * 50}%`);
      bar.style.setProperty('--size', `${Math.abs(out) * 50}%`);
    }
    this.renderThrottle();
    this.renderHelpers();
    this.$('try-live').checked = this.previewing;
    this.$('pill-preview').hidden = !this.previewing;
  }

  renderThrottle() {
    const throttle = this.config.throttle;
    const detents = throttle.mode === 'detents';
    const travel = this.lever();
    const out = throttleOut(travel, throttle);
    const [area, text] = zone(out, throttle);
    const side = this.config.throttle_lever === 'right' ? 'Right' : 'Left';
    this.$('throttle-help').textContent = detents
      ? 'ACE COMBAT throttle: centre cruises, forward boosts, back brakes. The quadrant detents mark where each band starts.'
      : 'Lever travel maps straight to the NEO throttle. Mid-travel is cruise.';
    this.$('throttle-source').textContent = `${side} lever · ${this.source === 'live' && this.monitor ? 'live' : 'test'}`;
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
    bar.style.setProperty('--from', `${50 + Math.min(out, 0) * 50}%`);
    bar.style.setProperty('--size', `${Math.abs(out) * 50}%`);
    this.$('detents').hidden = !detents;
    this.$('detent-legend').hidden = !detents;
    for (const button of this.doc.querySelectorAll('[data-capture]')) button.disabled = !this.monitor;
    this.$('capture-hint').textContent = this.monitor
      ? `Put the ${side.toLowerCase()} lever in a detent, then Capture.`
      : 'Start emulation to capture the lever in its detents.';
  }

  renderHelpers() {
    const precision = this.config.precision.source;
    const chip = this.$('precision-source');
    chip.textContent = precision ? `${short(precision)} · ${label(precision)}` : 'Off';
    chip.classList.toggle('off', !precision);
    this.$('precision-clear').disabled = !precision;
    const capturing = this.capture?.kind;
    this.$('precision-pick').textContent = capturing === 'precision' ? 'Cancel' : 'Press a control…';
    this.$('pause-record').textContent = capturing === 'pause' ? 'Done' : 'Record combo…';
    const keys = capturing === 'pause' ? this.capture.keys : this.config.combos.pause;
    const combo = this.$('pause-combo');
    // Rebuilt only when the combo changes: this runs on every live poll.
    if (combo.dataset.keys === keys.join()) return this.renderCapture(capturing);
    combo.dataset.keys = keys.join();
    combo.replaceChildren(...(keys.length ? keys : [null]).flatMap((key, index) => {
      const chipItem = this.doc.createElement('span');
      chipItem.className = key ? 'chip static' : 'chip static off';
      chipItem.textContent = key ? short(key) : 'Off';
      if (key) chipItem.title = label(key);
      if (!index) return [chipItem];
      const plus = this.doc.createElement('span');
      plus.className = 'plus';
      plus.textContent = '+';
      return [plus, chipItem];
    }));
    return this.renderCapture(capturing);
  }

  renderCapture(capturing) {
    this.$('pause-clear').disabled = !this.config.combos.pause.length;
    // The prompt sits in the card being captured for.
    for (const well of this.doc.querySelectorAll('.capture')) {
      well.hidden = well.dataset.for !== capturing;
      well.lastElementChild.textContent = !this.emulating ? 'Start emulation, then press the controls on the TCA.'
        : well.dataset.for === 'precision' ? 'Press the TCA control to hold for precision aiming.'
          : 'Press and release the controls for the combo (up to 4), then choose Done.';
    }
  }

  /** Lever rails against the saved thresholds (what the running emulation uses). */
  levers(levers) {
    this.lastLevers = levers;
    const rule = this.saved.split_high_g;
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
    this.$('lever-live').textContent = leverText(levers, rule);
    ['left', 'right'].forEach((side, i) => {
      const value = levers[i];
      this.$(`needle-${side}`).style.setProperty('--pos', `${Math.round(value * 100)}%`);
      const readout = this.$(`value-${side}`);
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
    this.emulating = Boolean(data.links);
    this.monitor = data.monitor ?? null;
    this.$('pill-paused').hidden = !data.paused;
    if (this.previewing && !data.preview && !this.sending) {
      this.previewing = false;
      this.mark(true, 'Unsaved changes · the preview ended with emulation.');
    }
    this.$('live-input-text').textContent = data.pressed.length ? `Now sending: ${data.pressed.map(label).join(', ')}` : data.links ? 'Nothing pressed' : 'Start emulation to see presses.';
    this.$('live-input').dataset.active = String(data.pressed.length > 0);
    this.levers(data.levers);
    const pressed = data.pressed.filter((key) => !this.held.has(key));
    const fresh = pressed.filter((key) => SELECTABLE.has(key));
    this.held = new Set(data.pressed);
    if (this.capture?.kind === 'precision' && fresh.length) {
      this.config.precision.source = fresh[0];
      this.capture = null;
      this.changed(`Unsaved: hold ${label(fresh[0])} for precision aiming.`);
    } else if (this.capture?.kind === 'pause') {
      this.capture.keys = [...new Set([...this.capture.keys, ...fresh])].slice(0, 4);
    }
    const picked = pressed.filter((key) => this.selectable(key));
    if (picked.length && !this.$('panel-mapping').hidden && !this.busy) this.select(picked[0]);
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

export async function start(win, interval = 250) {
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
