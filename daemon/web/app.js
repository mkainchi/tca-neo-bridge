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

/** {buttons, pov} → readable text. */
export function describe(mapping) {
  const parts = mapping.buttons.map((n) => neoName(`b${n}`));
  if (mapping.pov) parts.push(neoName(`pov-${mapping.pov}`));
  return parts.length ? parts.join(', ') : 'Nothing';
}

export function current(profile, key) {
  return { buttons: [...(profile.buttons[key] ?? [])].sort((a, b) => a - b), pov: profile.pov[key] ?? null };
}

export function same(a, b) {
  return a.pov === b.pov && a.buttons.join() === b.buttons.join();
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
  return changes;
}

/** Sources (among `keys`) currently mapped to a NEO hotspot. */
export function sourcesFor(profile, id, keys) {
  if (id.startsWith('pov-')) return keys.filter((key) => profile.pov[key] === id.slice(4));
  const n = Number(id.slice(1));
  return keys.filter((key) => (profile.buttons[key] ?? []).includes(n));
}

/** The device whose hat drives the camera: the profile's choice when it has a hat, else the first with one. */
export function cameraDevice(profile, devices) {
  const hatted = devices.filter((d) => d.layout.hat);
  return (hatted.find((d) => d.id === profile.camera) ?? hatted[0])?.id ?? null;
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

/** True while the split-lever rule holds NEO 5. */
export function splitActive([first, second], rule) {
  return rule.enabled && Math.max(first, second) >= rule.forward && Math.min(first, second) <= rule.back;
}

/** Live lever readout, flagging an active split against the saved thresholds. */
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
    inputs: validation ? ['busy', 'Source: validation stream'] : links?.inputs ? ['ok', `Controllers: ${links.inputs} open`] : ['off', 'Controllers: not open'],
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
  return { get: (path) => call('GET', path), put: (path, value) => call('PUT', path, JSON.stringify(value)) };
}

const ARROW = '<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1.5 L8.5 8 L1.5 8 Z"/></svg>';
const HELP = {
  sel: 'Click NEO buttons or a POV arrow to change this control’s mapping, then record it. Changes apply immediately, even while emulating.',
  rev: 'These controls feed it. Select one of them to change its mapping.',
  none: 'Select a control, then click NEO buttons or a POV arrow to build its mapping. With nothing selected, clicking a NEO button shows what feeds it.',
};
const percent = (value) => `${Math.round(value * 1000) / 1000}%`;
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
    this.full = null;        // whole saved profile file (every controller set)
    this.saved = null;       // saved mapping of the active controller set (edits start from here)
    this.config = null;      // settings draft
    this.globals = null;     // default global settings
    this.defaultProfile = null;
    this.devices = [];       // controllers present (or in the running session)
    this.active = [];        // the ones taking part
    this.names = {};
    this.keys = [];
    this.setup = '';
    this.selected = null;    // source key
    this.proposal = null;    // {buttons, pov} being built for the selection
    this.reverse = null;     // NEO hotspot inspected with nothing selected
    this.hand = 'right';
    this.held = new Set();
    this.labels = [];
    this.busy = false;
    this.dirty = false;
    this.detecting = null;   // settings target waiting for an axis movement
    this.baseline = null;
    this.resetMapping = false;
    this.$ = (id) => doc.getElementById(id);
  }

  async load() {
    const data = await this.api.get('/api/config');
    this.full = data.config;
    this.globals = data.defaults;
    if (data.profile) this.$('profile-path').textContent = data.profile;
    this.wire();
    this.status(data.status, data.links);
    await this.refresh();
    try {
      this.labels = (await this.api.get('/api/labels')).labels;
      this.renderLabels();
    } catch (error) {
      this.note(`Earlier corrections could not be loaded: ${error.message}`);
    }
  }

  /** Controllers and the active set's mapping; rebuilds everything that depends on them. */
  async refresh() {
    const found = await this.api.get('/api/devices');
    this.devices = found.devices;
    this.active = found.devices.filter((d) => found.active.includes(d.id));
    this.names = Object.fromEntries(found.devices.map((d) => [d.id, d.name]));
    this.keys = sources(this.active);
    this.setup = found.setup;
    this.saved = structuredClone(found.profile);
    this.defaultProfile = found.default;
    this.selected = null;
    this.proposal = null;
    this.reverse = null;
    this.$('setup-name').textContent = this.active.length ? this.active.map((d) => d.name).join(' + ') : 'no controllers';
    this.buildSources();
    this.buildSettings();
    this.showSettings(this.draft());
    this.render();
  }

  label(key) {
    return label(key, this.names);
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
    row.innerHTML = '<span class="cap"></span><div class="chips"></div>';
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
        container.lastElementChild.querySelector('.device-name').textContent = device.name;
      } else {
        const figure = this.doc.createElement('figure');
        figure.className = 'generic';
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

  /** Static listeners and the NEO side, once. */
  wire() {
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
    this.$('camera-device').addEventListener('change', (event) => this.commitProfile({ camera: event.target.value }));
    this.$('record').addEventListener('click', () => this.record());
    this.$('revert').addEventListener('click', () => { this.proposal = current(this.saved, this.selected); this.render(); });
    this.$('deselect').addEventListener('click', () => { this.selected = null; this.proposal = null; this.reverse = null; this.render(); });
    for (const tab of this.doc.querySelectorAll('[role=tab]')) {
      tab.addEventListener('click', () => this.tab(tab.id));
      tab.addEventListener('keydown', (event) => {
        if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') this.tab(tab.id === 'tab-mapping' ? 'tab-settings' : 'tab-mapping', true);
      });
    }
    this.$('profile').addEventListener('input', (event) => this.edit(event.target));
    this.$('profile').addEventListener('submit', (event) => { event.preventDefault(); return this.save(); });
    this.$('profile').addEventListener('click', (event) => {
      const button = event.target.closest('button');
      if (button?.dataset.detect) this.detect(button.dataset.detect);
      if (button?.dataset.remove) this.removeThrottle(Number(button.dataset.remove));
    });
    this.$('reset').addEventListener('click', () => {
      const profile = this.defaultProfile;
      this.showSettings({ deadzone: this.globals.deadzone, camera_strength: this.globals.camera_strength, devices: [], axes: structuredClone(profile.axes), split_high_g: structuredClone(profile.split_high_g) });
      this.resetMapping = true;
      this.mark(true, 'Unsaved: defaults, including every mapping of these controllers. Save settings to apply.');
    });
  }

  /** Axis selects and the controller list for the present controllers. */
  buildSettings() {
    const options = (empty) => [this.option(empty, ''), ...this.active.flatMap((d) => d.layout.axes.map((a) => this.option(`${d.name} · ${a.name}`, `${d.id}|${a.name}`)))];
    const rows = NEO_AXES.map(([axis, neo, action]) => {
      const row = this.doc.createElement('div');
      row.className = 'binding';
      row.innerHTML = `<label class="label" for="bind-${axis}">${action}</label><select id="bind-${axis}" class="sel" data-axis="${axis}"></select><label class="tog"><input type="checkbox" id="invert-${axis}" data-invert="${axis}"><span>Invert</span></label><button class="btn quiet" type="button" data-detect="${axis}">Detect</button>`;
      row.querySelector('select').append(...options(`No source (NEO ${neo} rests)`));
      return row;
    });
    this.$('bindings').replaceChildren(...rows);
    this.$('throttle-add').replaceChildren(...options('Add an axis…'));
    for (const index of [0, 1]) this.$(`lever-${index}`).replaceChildren(...options('Not set'));
    const list = this.$('device-list');
    list.replaceChildren(...this.devices.map((d) => {
      const item = this.doc.createElement('label');
      item.className = 'tog';
      item.innerHTML = '<input type="checkbox" data-device=""><span></span>';
      item.firstElementChild.dataset.device = d.id;
      item.lastElementChild.textContent = `${d.name} (${d.id})`;
      return item;
    }));
  }

  tab(id, focus = false) {
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
    this.proposal = current(this.saved, key);
    this.reverse = null;
    this.render();
  }

  camera() {
    return cameraDevice(this.saved, this.active);
  }

  /** The camera hat cannot be mapped while it drives the camera. */
  selectable(key) {
    const [device, control] = parts(key);
    return this.keys.includes(key) && !(control.startsWith('hat-') && this.saved.hat === 'camera' && device === this.camera());
  }

  /** Save the whole file with this controller set's mapping replaced. */
  async commit(profile) {
    const next = structuredClone(this.full);
    next.setups[this.setup] = profile;
    const result = await this.api.put('/api/config', next);
    this.full = next;
    this.saved = profile;
    return result;
  }

  /** Hat mode and camera hat are part of the mapping: save them at once, like a recorded mapping. */
  async commitProfile(change) {
    this.busy = true;
    try {
      const result = await this.commit({ ...structuredClone(this.saved), ...change });
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
      chip.title = chip.disabled ? 'Set the camera hat to Mapped controls to map it' : this.label(chip.dataset.source);
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
    this.$('neo-context').dataset.mode = mode;
    this.$('link-heading').textContent = mode === 'rev' ? 'Selected NEO control' : 'Selected control';
    this.$('selected-kind').textContent = { sel: 'Your control', rev: 'NEO target', none: 'Selection' }[mode];
    this.$('link-help').textContent = HELP[mode];
    this.$('compare').hidden = mode === 'none';
    this.$('mapped-new-label').hidden = mode !== 'sel';
    this.$('mapped-new').hidden = mode !== 'sel';
    this.$('mapped-now-label').textContent = mode === 'rev' ? 'Fed by' : 'Mapped now';
    if (now) {
      this.$('selected-name').textContent = this.label(this.selected);
      this.$('mapped-now').textContent = describe(now);
      this.$('mapped-new').textContent = changed ? describe(this.proposal) : 'Unchanged';
      this.$('neo-context-text').textContent = `Mapping of ${this.label(this.selected)} → ${describe(this.proposal)}`;
    } else if (this.reverse) {
      this.$('selected-name').textContent = neoName(this.reverse);
      this.$('mapped-now').textContent = feeds.length ? feeds.map((key) => this.label(key)).join(', ') : 'No control';
      this.$('neo-context-text').textContent = `Inspecting ${neoName(this.reverse)} · fed by ${feeds.length ? feeds.map(short).join(' · ') : 'nothing'}`;
    } else {
      this.$('selected-name').textContent = 'Nothing selected';
      this.$('mapped-now').textContent = '—';
      this.$('neo-context-text').textContent = 'Select a control to light up its NEO targets';
    }
    this.$('diff').replaceChildren(...(now ? diff(now, this.proposal) : []).map(([kind, text]) => {
      const item = this.doc.createElement('li');
      item.className = kind;
      item.textContent = text;
      return item;
    }));
    this.$('record').disabled = !changed || this.busy;
    this.$('revert').disabled = !changed || this.busy;
    this.$('deselect').disabled = mode === 'none';
    this.renderAxes();
  }

  renderAxes() {
    const axes = this.saved.axes;
    const camera = this.camera();
    const name = (binding) => (binding ? bindingName(binding, this.names) : 'No source');
    const rows = [
      ...NEO_AXES.map(([axis, neo, action]) => [neo, name(axes[axis]), action]),
      ['Z', axes.throttle.length ? axes.throttle.map((b) => bindingName(b, this.names)).join(' or ') : 'No source', 'Throttle'],
      ['Rx / Ry', this.saved.hat === 'mapped' ? 'No source (hat is mapped)' : camera ? `${this.names[camera]} hat` : 'No hat', 'Camera'],
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
    const entry = { label: this.label(key), sources: [key], mapping: `${describe(before)} → ${describe(after)}`.slice(0, 200) };
    const applied = result.running ? ' Applied to the running emulation.' : '';
    try {
      await this.api.put('/api/labels', [...this.labels, entry]);
      this.labels = [...this.labels, entry];
      this.renderLabels();
      this.note(`Recorded ${this.label(key)}: ${describe(after)}.${applied}`);
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

  /** Settings draft from the saved file and the active set's mapping. */
  draft() {
    return {
      deadzone: this.full.deadzone,
      camera_strength: this.full.camera_strength,
      devices: [...this.full.devices],
      axes: structuredClone(this.saved.axes),
      split_high_g: structuredClone(this.saved.split_high_g),
    };
  }

  showSettings(config) {
    this.config = config;
    this.levers = [0, 1].map((i) => config.split_high_g.levers[i] ?? null);
    this.$('deadzone').value = config.deadzone;
    this.$('camera').value = config.camera_strength;
    this.$('split-enabled').checked = config.split_high_g.enabled;
    this.$('split-reverse').checked = config.split_high_g.reverse;
    this.$('split-forward').value = Math.round(config.split_high_g.forward * 100);
    this.$('split-back').value = Math.round(config.split_high_g.back * 100);
    for (const id of ['deadzone', 'camera', 'split-forward', 'split-back']) this.sync(this.$(id));
    this.paintAxes();
  }

  /** Binding selects, invert boxes, the throttle list, the levers and the controller list from the draft. */
  paintAxes() {
    const axes = this.config.axes;
    for (const [axis] of NEO_AXES) {
      this.$(`bind-${axis}`).value = encode(axes[axis]);
      this.$(`invert-${axis}`).checked = Boolean(axes[axis]?.invert);
      this.$(`invert-${axis}`).disabled = !axes[axis];
    }
    this.$('throttle-add').value = '';
    this.$('throttle-list').replaceChildren(...axes.throttle.map((binding, index) => {
      const item = this.doc.createElement('li');
      item.innerHTML = `<span></span><label class="tog"><input type="checkbox" data-throttle="${index}"><span>Invert</span></label><button class="btn quiet" type="button" data-remove="${index}">Remove</button>`;
      item.firstElementChild.textContent = bindingName({ ...binding, invert: false }, this.names);
      item.querySelector('input').checked = binding.invert;
      return item;
    }));
    this.levers.forEach((binding, index) => { this.$(`lever-${index}`).value = encode(binding); });
    const auto = this.config.devices.length === 0;
    this.$('devices-auto').checked = auto;
    for (const box of this.doc.querySelectorAll('#device-list input')) {
      const device = this.devices.find((d) => d.id === box.dataset.device);
      box.checked = auto ? device.vid === 0x044f : this.config.devices.includes(device.id);
    }
    for (const button of this.doc.querySelectorAll('[data-detect]')) button.setAttribute('aria-busy', String(button.dataset.detect === this.detecting));
  }

  /** Keep a number field and its slider together, and redraw the deadzone band. */
  sync(field) {
    const scale = field.id.startsWith('split-') ? 1 : 100;
    const range = field.id.endsWith('-range');
    const partner = this.$(range ? field.id.slice(0, -6) : `${field.id}-range`);
    if (range) partner.value = field.id.startsWith('split-') ? field.value : (Number(field.value) / scale).toFixed(2);
    else partner.value = String(Math.round(Number(field.value) * scale));
    const band = Math.min(100, Math.max(0, Math.round(Number(this.$('deadzone').value) * 100) || 0));
    this.$('deadzone-band').style.setProperty('--band', `${band}%`);
    this.$('deadzone-text').textContent = `Ignored band ±${band}%`;
  }

  /** Ticking a controller turns automatic selection into an explicit list of what is ticked. */
  pickDevices() {
    this.config.devices = [...this.doc.querySelectorAll('#device-list input')].filter((box) => box.checked).map((box) => box.dataset.device);
  }

  option(text, value) {
    const option = this.doc.createElement('option');
    option.textContent = text;
    option.value = value;
    return option;
  }

  edit(target) {
    const axes = this.config.axes;
    if (target.dataset.axis) axes[target.dataset.axis] = decode(target.value);
    else if (target.dataset.invert) axes[target.dataset.invert].invert = target.checked;
    else if (target.id === 'throttle-add') this.addThrottle(decode(target.value));
    else if (target.dataset.throttle) axes.throttle[Number(target.dataset.throttle)].invert = target.checked;
    else if (target.id.startsWith('lever-')) this.levers[Number(target.id.slice(6))] = decode(target.value);
    else if (target.id === 'split-enabled') this.config.split_high_g.enabled = target.checked;
    else if (target.id === 'split-reverse') this.config.split_high_g.reverse = target.checked;
    else if (target.id === 'devices-auto') this.config.devices = target.checked ? [] : this.active.map((d) => d.id);
    else if (target.dataset.device) this.pickDevices();
    else this.sync(target);
    if (!target.classList.contains('num') && !target.classList.contains('rng')) this.paintAxes();
    this.mark(true, 'Unsaved changes.');
  }

  addThrottle(binding) {
    if (binding && !this.config.axes.throttle.some((b) => sameAxis(b, binding))) this.config.axes.throttle.push(binding);
  }

  removeThrottle(index) {
    this.config.axes.throttle.splice(index, 1);
    this.paintAxes();
    this.mark(true, 'Unsaved changes.');
  }

  /** Wait for the next axis that moves (live samples during emulation) and bind it to `target`. */
  detect(target) {
    this.detecting = target;
    this.baseline = null;
    this.$('detect-hint').textContent = 'Move the axis you want, all the way, during emulation…';
    this.paintAxes();
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
    else if (target.startsWith('lever-')) this.levers[Number(target.slice(6))] = binding;
    else this.config.axes[target] = binding;
    this.detecting = null;
    this.$('detect-hint').textContent = `Detected ${bindingName(binding, this.names)}. Save settings to apply.`;
    this.paintAxes();
    this.mark(true, 'Unsaved changes.');
  }

  mark(dirty, text) {
    this.dirty = dirty;
    this.$('dirty').textContent = text;
    this.$('dirty').dataset.state = dirty ? 'dirty' : 'saved';
  }

  number(id, maximum, inclusive) {
    const field = this.$(id);
    const value = Number(field.value);
    const valid = field.value !== '' && value >= 0 && (inclusive ? value <= maximum : value < maximum);
    field.setAttribute('aria-invalid', String(!valid));
    this.$(`${id}-error`).textContent = valid ? '' : `Enter a number from 0 to ${inclusive ? maximum : 'below ' + maximum}.`;
    return valid ? value : null;
  }

  fail(text) {
    this.$('dirty').textContent = text;
    this.$('dirty').dataset.state = 'error';
  }

  async save() {
    const deadzone = this.number('deadzone', 1, false);
    const camera = this.number('camera', 1, true);
    const forward = this.number('split-forward', 100, true);
    let back = this.number('split-back', 100, true);
    if (back !== null && forward !== null && back >= forward) {
      this.$('split-back').setAttribute('aria-invalid', 'true');
      this.$('split-back-error').textContent = 'Must be below the forward lever threshold.';
      back = null;
    }
    if (deadzone === null || camera === null || forward === null || back === null) {
      this.fail('Fix the highlighted fields, then save.');
      return;
    }
    const axes = this.config.axes;
    const levers = this.levers.every(Boolean) ? this.levers.map((lever) => ({ ...lever, invert: axes.throttle.find((b) => sameAxis(b, lever))?.invert ?? false })) : [];
    const split = { ...this.config.split_high_g, forward: forward / 100, back: back / 100, levers };
    if (split.enabled && !levers.length) {
      this.fail('Pick both levers for split High-G, or turn the rule off.');
      return;
    }
    const next = structuredClone({ ...this.full, deadzone, camera_strength: camera, devices: this.config.devices });
    const base = this.resetMapping ? this.defaultProfile : this.saved;
    const profile = structuredClone({ ...base, axes, split_high_g: split });
    if (this.setup) next.setups[this.setup] = profile;
    const devicesChanged = next.devices.join() !== this.full.devices.join();
    this.$('save').disabled = true;
    this.$('dirty').textContent = 'Saving…';
    try {
      const result = await this.api.put('/api/config', next);
      this.full = next;
      this.saved = structuredClone(profile);
      this.resetMapping = false;
      this.mark(false, result.running ? 'Saved. Applied to the running emulation.' : 'Saved.');
      if (devicesChanged) await this.refresh();
      this.render();
      this.lever(this.lastLevers);
    } catch (error) {
      this.fail(`Not saved: ${error.message}`);
    } finally {
      this.$('save').disabled = false;
    }
  }

  /** Lever rails against the saved thresholds (what the running emulation uses). */
  lever(levers) {
    this.lastLevers = levers;
    const rule = this.saved.split_high_g;
    const names = [0, 1].map((i) => (rule.levers[i] ? leverName(rule.levers[i], this.names) : ['First lever', 'Second lever'][i]));
    const rails = this.$('lever-rails');
    rails.hidden = !levers;
    rails.style.setProperty('--fw', `${Math.round(rule.forward * 100)}%`);
    rails.style.setProperty('--bk', `${Math.round(rule.back * 100)}%`);
    names.forEach((name, i) => { this.$(`lever-name-${i}`).textContent = name; });
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

  /** Live input: light held controls; a new press selects that control unless a recording is being saved. */
  live(data) {
    this.status(data.status, data.links);
    this.$('live-input-text').textContent = data.pressed.length ? `Now sending: ${data.pressed.map((key) => this.label(key)).join(', ')}` : data.links ? 'Nothing pressed' : 'Start emulation to see presses.';
    this.$('live-input').dataset.active = String(data.pressed.length > 0);
    this.lever(data.levers);
    if (this.detecting) this.detected(data.axes);
    const fresh = data.pressed.filter((key) => !this.held.has(key) && this.selectable(key));
    this.held = new Set(data.pressed);
    if (fresh.length && !this.$('panel-mapping').hidden && !this.busy) this.select(fresh[0]);
    else this.render();
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
