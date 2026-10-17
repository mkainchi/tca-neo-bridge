// TCA-NEO bridge configuration page. The token comes from the URL fragment, so it never appears in request lines.
export const COUNTS = { s: 17, q: 31 };

/** AC8 actions for native NEO buttons (official AC8 PC mapping); unlisted buttons have no documented action. */
export const NEO_ACTIONS = { 1: 'Gun', 2: 'Missile', 3: 'Weapon select', 4: 'Target select', 5: 'High-G', 6: 'Autopilot', 7: 'Flares', 8: 'View', 9: 'Radar', 10: 'Gear', 12: 'Pause / menu' };

/** Hotspots in neo.svg viewBox units (1000 × 1060). Button 16 exists physically but not in the NEO report. */
export const NEO_SPOTS = [
  { id: 'b1', x: 500, y: 372 }, { id: 'b2', x: 500, y: 282 }, { id: 'b3', x: 262, y: 272 }, { id: 'b4', x: 622, y: 236 },
  { id: 'b5', x: 444, y: 590 }, { id: 'b6', x: 452, y: 645 }, { id: 'b7', x: 452, y: 700 }, { id: 'b8', x: 444, y: 755 },
  { id: 'b9', x: 250, y: 546 }, { id: 'b10', x: 322, y: 538 }, { id: 'b11', x: 610, y: 905 }, { id: 'b12', x: 790, y: 905 },
  { id: 'b13', x: 360, y: 950 }, { id: 'b14', x: 480, y: 950 }, { id: 'b15', x: 420, y: 985 }, { id: 'b16', x: 700, y: 955 },
  { id: 'pov-up', x: 380, y: 133 }, { id: 'pov-right', x: 437, y: 190 }, { id: 'pov-down', x: 380, y: 247 }, { id: 'pov-left', x: 323, y: 190 },
];

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

export function current(config, key) {
  return { buttons: [...(config.buttons[key] ?? [])].sort((a, b) => a - b), pov: config.pov[key] ?? null };
}

export function same(a, b) {
  return a.pov === b.pov && a.buttons.join() === b.buttons.join();
}

/** TCA sources currently mapped to a NEO hotspot. */
export function sourcesFor(config, id) {
  const keys = [...sources(), ...HAT_SOURCES];
  if (id.startsWith('pov-')) return keys.filter((key) => config.pov[key] === id.slice(4));
  const n = Number(id.slice(1));
  return keys.filter((key) => (config.buttons[key] ?? []).includes(n));
}

/** Live lever readout, flagging an active split against the saved thresholds. */
export function leverText([left, right], rule) {
  const active = rule.enabled && Math.max(left, right) >= rule.forward && Math.min(left, right) <= rule.back;
  return `Left lever ${Math.round(left * 100)}% · Right lever ${Math.round(right * 100)}%${active ? ' · High-G active' : ''}`;
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

const SELECTABLE = new Set(sources());
const ARROW = '<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1.5 L8.5 8 L1.5 8 Z"/></svg>';

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
    this.held = new Set();
    this.labels = [];
    this.busy = false;
    this.dirty = false;
    this.$ = (id) => doc.getElementById(id);
  }

  async load() {
    const data = await this.api.get('/api/config');
    this.defaults = data.defaults;
    this.saved = structuredClone(data.config);
    this.build();
    this.showSettings(data.config);
    this.status(data.status);
    this.render();
    try {
      this.labels = (await this.api.get('/api/labels')).labels;
      this.renderLabels();
    } catch (error) {
      this.note(`Earlier corrections could not be loaded: ${error.message}`);
    }
  }

  build() {
    const chips = [['extra-hat', HAT_SOURCES], ['extra-sidestick', ['s17']], ['extra-detents', sources().filter((k) => /^q(9|1[0-6])$/.test(k))], ['extra-addons', sources().filter((k) => /^q(1[7-9]|2\d|3[01])$/.test(k))]];
    for (const [id, keys] of chips) {
      for (const key of keys) {
        const chip = this.doc.createElement('button');
        chip.type = 'button';
        const hat = key.startsWith('hat-');
        chip.className = hat ? 'ctl chip hat-chip' : 'ctl chip';
        chip.dataset.source = key;
        chip.textContent = hat ? key[4].toUpperCase() + key.slice(5) : key.slice(1);
        this.$(id).append(chip);
      }
    }
    for (const control of this.doc.querySelectorAll('[data-source]')) {
      control.setAttribute('aria-label', label(control.dataset.source));
      control.setAttribute('aria-pressed', 'false');
      control.title = label(control.dataset.source);
      control.addEventListener('click', () => this.select(control.dataset.source));
    }
    const frame = this.$('neo-frame');
    for (const spot of NEO_SPOTS) {
      const button = this.doc.createElement('button');
      const pov = spot.id.startsWith('pov-');
      button.type = 'button';
      button.className = pov ? `spot pov ${spot.id}` : 'spot';
      button.dataset.neo = spot.id;
      button.style.setProperty('--x', `${spot.x / 10}%`);
      button.style.setProperty('--y', `${spot.y / 10.6}%`);
      button.innerHTML = pov ? ARROW : spot.id.slice(1);
      button.disabled = spot.id === 'b16';
      button.setAttribute('aria-label', button.disabled ? 'NEO button 16, not in the NEO report' : neoName(spot.id));
      button.title = button.getAttribute('aria-label');
      button.setAttribute('aria-pressed', 'false');
      button.addEventListener('click', () => this.pick(spot.id));
      frame.append(button);
    }
    this.$('neo-legend').textContent = Object.entries(NEO_ACTIONS).map(([n, action]) => `${n} ${action}`).join(' · ');
    this.$('handedness').addEventListener('change', () => { this.$('sidestick-frame').dataset.hand = this.$('handedness').value; });
    this.$('hat-mode').addEventListener('change', () => this.setHat(this.$('hat-mode').value));
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
    this.$('reset').addEventListener('click', () => {
      this.showSettings(structuredClone(this.defaults));
      this.mark(true, 'Unsaved: defaults, including every mapping. Save settings to apply.');
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
    const linked = new Set(this.reverse ? sourcesFor(this.saved, this.reverse) : []);
    for (const control of this.doc.querySelectorAll('[data-source]')) {
      const key = control.dataset.source;
      control.setAttribute('aria-pressed', String(key === this.selected));
      control.classList.toggle('linked', linked.has(key));
      control.classList.toggle('held', this.held.has(key));
    }
    const hatMapped = this.saved.hat === 'mapped';
    this.$('hat-mode').value = hatMapped ? 'mapped' : 'camera';
    for (const chip of this.doc.querySelectorAll('.hat-chip')) {
      chip.disabled = !hatMapped;
      chip.title = hatMapped ? label(chip.dataset.source) : 'Set the hat to Mapped controls to map it';
    }
    const hats = [...this.held].filter((key) => key.startsWith('hat-')).map((key) => key.slice(4));
    for (const hat of this.doc.querySelectorAll('.hat')) {
      hat.classList.toggle('held', hats.some((direction) => direction.split('-').includes(hat.dataset.hat)));
    }
    for (const spot of this.doc.querySelectorAll('[data-neo]')) {
      const id = spot.dataset.neo;
      const inMapping = (mapping) => (id.startsWith('pov-') ? mapping.pov === id.slice(4) : mapping.buttons.includes(Number(id.slice(1))));
      spot.setAttribute('aria-pressed', String(now ? inMapping(this.proposal) : id === this.reverse));
      spot.classList.toggle('current', Boolean(now) && inMapping(now));
      spot.classList.toggle('added', Boolean(now) && inMapping(this.proposal) && !inMapping(now));
    }
    const changed = Boolean(now) && !same(now, this.proposal);
    if (now) {
      this.$('selected-name').textContent = label(this.selected);
      this.$('mapped-now').textContent = describe(now);
      this.$('mapped-new').textContent = changed ? describe(this.proposal) : 'Unchanged';
    } else if (this.reverse) {
      const names = [...linked].map(label);
      this.$('selected-name').textContent = neoName(this.reverse);
      this.$('mapped-now').textContent = names.length ? `From ${names.join(', ')}` : 'No TCA control';
      this.$('mapped-new').textContent = '—';
    } else {
      this.$('selected-name').textContent = 'Nothing selected';
      this.$('mapped-now').textContent = '—';
      this.$('mapped-new').textContent = '—';
    }
    this.$('record').disabled = !changed || this.busy;
    this.$('revert').disabled = !changed || this.busy;
    this.$('deselect').disabled = !now && !this.reverse;
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

  renderLabels() {
    const list = this.$('recorded');
    list.replaceChildren();
    this.labels.forEach((entry, index) => {
      const item = this.doc.createElement('li');
      item.append(`${entry.label}: ${entry.mapping} `);
      const remove = this.doc.createElement('button');
      remove.type = 'button';
      remove.className = 'quiet';
      remove.textContent = 'Remove';
      remove.setAttribute('aria-label', `Remove ${entry.label} from the correction log`);
      remove.addEventListener('click', () => this.forget(index));
      item.append(remove);
      list.append(item);
    });
    this.$('recorded-count').textContent = String(this.labels.length);
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

  status(text) {
    this.$('daemon-status').textContent = `Daemon: ${text}`;
  }

  showSettings(config) {
    this.config = config;
    this.$('deadzone').value = config.deadzone;
    this.$('camera').value = config.camera_strength;
    this.$('lever').value = config.throttle_lever;
    for (const axis of ['roll', 'pitch', 'yaw', 'throttle']) this.$(`invert-${axis}`).checked = config.invert[axis];
    this.$('split-enabled').checked = config.split_high_g.enabled;
    this.$('split-forward').value = Math.round(config.split_high_g.forward * 100);
    this.$('split-back').value = Math.round(config.split_high_g.back * 100);
  }

  edit(target) {
    if (target.id === 'lever') this.config.throttle_lever = target.value;
    else if (target.id.startsWith('invert-')) this.config.invert[target.id.slice(7)] = target.checked;
    else if (target.id === 'split-enabled') this.config.split_high_g.enabled = target.checked;
    this.mark(true, 'Unsaved changes.');
  }

  mark(dirty, text) {
    this.dirty = dirty;
    this.$('dirty').textContent = text;
  }

  number(id, maximum, inclusive) {
    const field = this.$(id);
    const value = Number(field.value);
    const valid = field.value !== '' && value >= 0 && (inclusive ? value <= maximum : value < maximum);
    field.setAttribute('aria-invalid', String(!valid));
    this.$(`${id}-error`).textContent = valid ? '' : `Enter a number from 0 to ${inclusive ? maximum : 'below ' + maximum}.`;
    return valid ? value : null;
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
      this.$('dirty').textContent = 'Fix the highlighted fields, then save.';
      return;
    }
    const split = { ...this.config.split_high_g, forward: forward / 100, back: back / 100 };
    const next = structuredClone({ ...this.config, deadzone, camera_strength: camera, split_high_g: split });
    this.$('save').disabled = true;
    this.$('dirty').textContent = 'Saving…';
    try {
      const result = await this.api.put('/api/config', next);
      this.config = next;
      this.saved = structuredClone(next);
      this.mark(false, result.running ? 'Saved. Applied to the running emulation.' : 'Saved.');
      this.render();
    } catch (error) {
      this.$('dirty').textContent = `Not saved: ${error.message}`;
    } finally {
      this.$('save').disabled = false;
    }
  }

  /** Live input: light held controls; a new press selects that control unless a recording is being saved. */
  live(data) {
    this.status(data.status);
    this.$('live-input').textContent = `Now sending: ${data.pressed.length ? data.pressed.map(label).join(', ') : 'nothing'}`;
    this.$('lever-live').textContent = data.levers ? leverText(data.levers, this.saved.split_high_g) : 'Lever positions appear here during emulation.';
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
