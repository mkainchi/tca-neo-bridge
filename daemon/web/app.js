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
  return { get: (path) => call('GET', path), put: (path, value) => call('PUT', path, JSON.stringify(value)) };
}

const SELECTABLE = new Set(sources());
const ARROW = '<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 1.5 L8.5 8 L1.5 8 Z"/></svg>';
const HELP = {
  sel: 'Click NEO buttons or a POV arrow to change this control’s mapping, then record it. Changes apply immediately, even while emulating.',
  rev: 'These TCA controls feed it. Select one of them to change its mapping.',
  none: 'Select a TCA control, then click NEO buttons or a POV arrow to build its mapping. With nothing selected, clicking a NEO button shows what feeds it.',
};
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
    this.$ = (id) => doc.getElementById(id);
  }

  async load() {
    const data = await this.api.get('/api/config');
    this.defaults = data.defaults;
    this.saved = structuredClone(data.config);
    if (data.profile) this.$('profile-path').textContent = data.profile;
    this.build();
    this.showSettings(data.config);
    this.status(data.status, data.links);
    this.render();
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

  showSettings(config) {
    this.config = config;
    this.$('deadzone').value = config.deadzone;
    this.$('camera').value = config.camera_strength;
    for (const radio of this.doc.querySelectorAll('input[name=lever]')) radio.checked = radio.value === config.throttle_lever;
    for (const axis of ['roll', 'pitch', 'yaw', 'throttle']) this.$(`invert-${axis}`).checked = config.invert[axis];
    this.$('split-enabled').checked = config.split_high_g.enabled;
    this.$('split-forward').value = Math.round(config.split_high_g.forward * 100);
    this.$('split-back').value = Math.round(config.split_high_g.back * 100);
    for (const id of ['deadzone', 'camera', 'split-forward', 'split-back']) this.sync(this.$(id));
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

  edit(target) {
    if (target.name === 'lever') this.config.throttle_lever = target.value;
    else if (target.id.startsWith('invert-')) this.config.invert[target.id.slice(7)] = target.checked;
    else if (target.id === 'split-enabled') this.config.split_high_g.enabled = target.checked;
    else this.sync(target);
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
      this.$('dirty').dataset.state = 'error';
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
      this.levers(this.lastLevers);
    } catch (error) {
      this.$('dirty').textContent = `Not saved: ${error.message}`;
      this.$('dirty').dataset.state = 'error';
    } finally {
      this.$('save').disabled = false;
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

  /** Live input: light held controls; a new press selects that control unless a recording is being saved. */
  live(data) {
    this.status(data.status, data.links);
    this.$('live-input-text').textContent = data.pressed.length ? `Now sending: ${data.pressed.map(label).join(', ')}` : data.links ? 'Nothing pressed' : 'Start emulation to see presses.';
    this.$('live-input').dataset.active = String(data.pressed.length > 0);
    this.levers(data.levers);
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
