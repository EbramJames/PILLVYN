import { playWebDing } from './effects.js';
import {
  THEME_PRESETS,
  applyAppearanceVars,
  getThemePreset,
  normalizeHexColor,
  updateRangeTrackFill,
} from './theme.js';

const { invoke } = window.__TAURI__?.core || {};
const { listen } = window.__TAURI__?.event || {};

const SECTIONS = [
  {
    id: 'appearance',
    label: 'Appearance',
    subtitle: 'Theme palette, accent glow, live transparency, and capsule proportions.',
    icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r=".5" fill="currentColor"/><circle cx="17.5" cy="10.5" r=".5" fill="currentColor"/><circle cx="8.5" cy="7.5" r=".5" fill="currentColor"/><circle cx="6.5" cy="12.5" r=".5" fill="currentColor"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.9 0 1.6-.8 1.6-1.7 0-.4-.2-.8-.4-1.1-.3-.3-.4-.7-.4-1.1 0-.9.8-1.7 1.7-1.7H17c2.8 0 5-2.2 5-5 0-4.9-4.5-8.4-10-8.4z"/></svg>`,
  },
  {
    id: 'behavior',
    label: 'Display & Behavior',
    subtitle: 'Monitor positioning, hover interactions, and auto-collapse timing.',
    icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>`,
  },
  {
    id: 'modules',
    label: 'Modules & Tools',
    subtitle: 'Choose active hub modules, weather units, gaming focus, and local AI.',
    icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/></svg>`,
  },
  {
    id: 'shortcuts',
    label: 'Shortcuts & Apps',
    subtitle: 'Global hotkeys, custom quick actions, and per-application focus rules.',
    icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>`,
  },
  {
    id: 'system',
    label: 'Privacy & System',
    subtitle: 'Startup behavior, clipboard safety, focus toggles, and app updates.',
    icon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>`,
  },
];

const QUICK_ACCENTS = ['#5b8cff', '#38d9c0', '#8c78ff', '#f05a74', '#36d399', '#f4b740'];

const MODULE_META = [
  ['music', 'Music Player', 'Active media detection, artwork, and playback controls'],
  ['system', 'System Controls', 'Master volume, microphone mute, brightness, and audio outputs'],
  ['hardware', 'Hardware Monitor', 'Live CPU, RAM, GPU, and storage utilization'],
  ['network', 'Network Monitor', 'Real-time download/upload speed, ping latency, and VPN state'],
  ['gaming', 'Gaming Mode', 'Detect fullscreen games, suppress distractions, click-through HUD'],
  ['downloads', 'Download Monitor', 'Track Steam & browser download progress inside the capsule'],
  ['notifications', 'Notification Center', 'Alert inbox with priority filtering and Do Not Disturb'],
  ['clipboard', 'Clipboard History', 'Recent copies with one-click paste and privacy filter'],
  ['actions', 'Quick Actions', 'One-click launchpad for system tools, URIs, and URLs'],
  ['widgets', 'Clock & Weather', 'Timers, stopwatch, 24h/12h clock, and local weather forecast'],
  ['plugins', 'Plugins', 'Load lightweight community live-card extensions'],
  ['ai', 'AI Assistant', 'Local Ollama or OpenAI-compatible prompt assistant'],
];

const TAB_CHOICES = [
  ['', 'Keep Current Tab'],
  ['overview', 'Hub Overview'],
  ['music', 'Media Player'],
  ['system', 'System Controls'],
  ['gaming', 'Gaming Focus'],
  ['hardware', 'Hardware Stats'],
  ['network', 'Network Monitor'],
  ['notifications', 'Alerts'],
  ['clipboard', 'Clipboard'],
  ['actions', 'Quick Actions'],
  ['widgets', 'Timers & Weather'],
];

let savedConfig = null;
let config = null;
let monitors = [];
let plugins = null;
let appVersion = '2.0.0';
let islandVisible = true;
let islandExpanded = false;
let activeSection = 'appearance';
let dirty = false;
let resetConfirmArmed = false;
let resetConfirmTimer = null;
let updateResult = null;
let recordingHotkeyKey = null;
let previewDebounceTimer = null;
let repositionModeActive = false;

function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') node.className = v;
    else if (k === 'text') node.textContent = v;
    else if (k === 'style' && typeof v === 'object') Object.assign(node.style, v);
    else if (k.startsWith('on') && typeof v === 'function') node.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === 'disabled') node.disabled = Boolean(v);
    else if (k === 'checked') node.checked = Boolean(v);
    else if (k === 'selected') node.selected = Boolean(v);
    else if (k === 'value') node.value = String(v);
    else node.setAttribute(k, v === true ? '' : String(v));
  }
  for (const child of Array.isArray(children) ? children : [children]) {
    if (child == null) continue;
    node.appendChild(typeof child === 'string' ? document.createTextNode(child) : child);
  }
  return node;
}

function showBanner(message, tone = 'ok') {
  const banner = document.getElementById('settings-banner');
  if (!banner) return;
  if (!message) {
    banner.className = 'settings-banner is-hidden';
    banner.textContent = '';
    return;
  }
  banner.className = `settings-banner is-${tone}`;
  banner.innerHTML = '';
  banner.appendChild(el('span', { text: message }));
  banner.appendChild(
    el('button', {
      type: 'button',
      class: 'btn-mini',
      onClick: () => showBanner(''),
    }, 'Dismiss')
  );
}

function setDirtyState(nextDirty) {
  dirty = Boolean(nextDirty);
  const pill = document.getElementById('dirty-pill');
  const revertBtn = document.getElementById('btn-revert');
  if (pill) {
    pill.textContent = dirty ? 'Unsaved Changes' : 'All Saved';
    pill.className = `dirty-pill ${dirty ? 'is-dirty' : 'is-saved'}`;
  }
  if (revertBtn) {
    revertBtn.disabled = !dirty;
  }
}

function syncSettingsTheme() {
  if (!config?.appearance) return;
  applyAppearanceVars(config.appearance, {
    themeTarget: document.body,
    varTarget: document.documentElement,
    applyOpacity: true,
  });
}

function scheduleLiveAppearancePreview() {
  syncSettingsTheme();
  updateLivePreviewBox();
  if (!invoke || !config?.appearance) return;
  clearTimeout(previewDebounceTimer);
  previewDebounceTimer = setTimeout(() => {
    invoke('preview_appearance', { appearance: config.appearance }).catch(() => {});
  }, 45);
}

function updateSidebarStatus() {
  const dot = document.getElementById('sidebar-status-dot');
  const text = document.getElementById('sidebar-status-text');
  const meta = document.getElementById('sidebar-status-meta');
  const visBtn = document.getElementById('btn-toggle-island');
  const expBtn = document.getElementById('btn-expand-island');
  if (dot) {
    dot.className = `status-dot ${islandVisible ? 'is-online' : 'is-hidden'}`;
  }
  if (text) {
    text.textContent = !islandVisible
      ? 'Island Hidden'
      : islandExpanded
        ? 'Island Expanded'
        : 'Island Compact';
  }
  if (meta) {
    const count = monitors.length || 1;
    meta.textContent = `${count} ${count === 1 ? 'display' : 'displays'} detected`;
  }
  if (visBtn) {
    visBtn.textContent = islandVisible ? 'Hide Island' : 'Show Island';
  }
  if (expBtn) {
    expBtn.textContent = islandExpanded ? 'Collapse' : 'Expand';
  }
}

function updateLivePreviewBox() {
  const cap = document.getElementById('preview-capsule');
  const titleEl = document.getElementById('preview-title');
  const badgeEl = document.getElementById('preview-badge');
  const captionEl = document.getElementById('preview-caption');
  if (!cap || !config?.appearance) return;
  const ap = config.appearance;
  const preset = getThemePreset(ap.theme);
  const scale = Number(ap.scale || 1);
  applyAppearanceVars(ap, { themeTarget: cap, varTarget: cap, applyOpacity: true });
  document.body.classList.toggle('reduced-motion', Boolean(ap.reduced_motion));
  cap.style.width = `${Math.round(ap.compact_width || 268)}px`;
  cap.style.transform = `scale(${scale.toFixed(2)})`;
  const emojisOn = ap.show_emojis !== false;
  if (titleEl) {
    titleEl.textContent = config.privacy?.dnd_mode
      ? `${emojisOn ? '🔕 ' : ''}Do Not Disturb`
      : config.privacy?.focus_mode
        ? `${emojisOn ? '🌙 ' : ''}Focus Active`
        : `${emojisOn ? '✨ ' : ''}PILLVYN · Ready`;
  }
  if (badgeEl) {
    badgeEl.textContent = `${preset.name}`;
  }
  if (captionEl) {
    captionEl.textContent = `Theme ${preset.name} · Accent ${normalizeHexColor(ap.accent_color).toUpperCase()} · Opacity ${Math.round((ap.opacity ?? 0.88) * 100)}% · Scale ${Math.round(scale * 100)}% · Width ${ap.compact_width}px`;
  }
}

function toggleRow(title, desc, checked, onChange, { id } = {}) {
  const input = el('input', { type: 'checkbox', id });
  input.checked = Boolean(checked);
  input.addEventListener('change', (e) => {
    setDirtyState(true);
    onChange(Boolean(e.target.checked));
  });

  return el('div', { class: 'setting-row' }, [
    el('div', { class: 'row-info' }, [
      el('div', { class: 'row-title', text: title }),
      desc ? el('div', { class: 'row-desc', text: desc }) : null,
    ]),
    el('label', { class: 'switch' }, [input, el('span', { class: 'switch-track' })]),
  ]);
}

function sliderField({
  id,
  title,
  desc,
  value,
  min,
  max,
  step = 1,
  defaultValue,
  format = (v) => String(v),
  minLabel,
  maxLabel,
  onChange,
  livePreview = false,
}) {
  const readout = el('span', { class: 'slider-readout', id: id ? `${id}-value` : undefined, text: format(value) });
  const resetBtn = el('button', {
    type: 'button',
    class: `slider-reset-link ${Math.abs(value - defaultValue) < step * 0.5 ? 'is-hidden' : ''}`,
    text: 'Default',
  });

  const input = el('input', {
    type: 'range',
    id,
    class: 'pro-slider',
    min: String(min),
    max: String(max),
    step: String(step),
    value: String(value),
  });
  updateRangeTrackFill(input);

  const applyNewVal = (rawNum) => {
    input.value = String(rawNum);
    updateRangeTrackFill(input);
    readout.textContent = format(rawNum);
    resetBtn.classList.toggle('is-hidden', Math.abs(rawNum - defaultValue) < step * 0.5);
    setDirtyState(true);
    onChange(rawNum);
    if (livePreview) {
      scheduleLiveAppearancePreview();
    }
  };

  input.addEventListener('input', (e) => {
    applyNewVal(Number(e.target.value));
  });

  resetBtn.addEventListener('click', () => {
    applyNewVal(defaultValue);
  });

  return el('div', { class: 'slider-row' }, [
    el('div', { class: 'slider-header' }, [
      el('div', { class: 'row-info' }, [
        el('div', { class: 'row-title', text: title }),
        desc ? el('div', { class: 'row-desc', text: desc }) : null,
      ]),
      el('div', { class: 'slider-right' }, [resetBtn, readout]),
    ]),
    el('div', { class: 'slider-track-wrap' }, [
      el('span', { class: 'slider-bound', text: minLabel || format(min) }),
      input,
      el('span', { class: 'slider-bound is-max', text: maxLabel || format(max) }),
    ]),
  ]);
}

function renderNav() {
  const nav = document.getElementById('settings-nav');
  if (!nav) return;
  nav.innerHTML = '';

  const titleEl = document.getElementById('section-title');
  const subEl = document.getElementById('section-subtitle');

  for (const sec of SECTIONS) {
    const active = sec.id === activeSection;
    if (active) {
      if (titleEl) titleEl.textContent = sec.label;
      if (subEl) subEl.textContent = sec.subtitle;
    }

    const iconWrap = el('span', { class: 'nav-item-icon' });
    iconWrap.innerHTML = sec.icon;

    nav.appendChild(
      el('button', {
        type: 'button',
        class: `nav-item ${active ? 'is-active' : ''}`,
        onClick: () => {
          activeSection = sec.id;
          renderNav();
          renderSection();
          if (sec.id === 'behavior' && invoke) {
            invoke('get_monitors').then((list) => {
              if (Array.isArray(list)) {
                monitors = list;
                updateSidebarStatus();
                if (activeSection === 'behavior') renderSection();
              }
            }).catch(() => {});
          }
        },
      }, [iconWrap, el('span', { text: sec.label })])
    );
  }
}

function renderAppearanceSection(container) {
  const ap = config.appearance;

  // Live capsule preview
  container.appendChild(
    el('div', { class: 'preview-stage' }, [
      el('div', { class: 'preview-capsule', id: 'preview-capsule' }, [
        el('div', { class: 'preview-capsule-left' }, [
          el('span', { class: 'preview-dot' }),
          el('span', { class: 'preview-title', id: 'preview-title', text: 'PILLVYN · Ready' }),
        ]),
        el('span', { class: 'preview-badge', id: 'preview-badge', text: getThemePreset(ap.theme).name }),
      ]),
      el('div', { class: 'preview-caption', id: 'preview-caption' }),
    ])
  );

  // Theme selector card
  const themeGrid = el('div', { class: 'theme-grid' }, THEME_PRESETS.map((preset) => {
    const selected = ap.theme === preset.id;
    return el('button', {
      type: 'button',
      class: `theme-swatch-card ${selected ? 'is-selected' : ''}`,
      onClick: () => {
        ap.theme = preset.id;
        ap.accent_color = preset.accent;
        setDirtyState(true);
        scheduleLiveAppearancePreview();
        renderSection();
      },
    }, [
      el('div', { class: 'theme-chip', style: { background: preset.surface } }, [
        el('span', { class: 'theme-chip-pill', style: { background: preset.accent } }),
      ]),
      el('div', { class: 'theme-card-name', text: preset.name }),
      el('div', { class: 'theme-card-desc', text: preset.desc }),
    ]);
  }));

  const currentHex = normalizeHexColor(ap.accent_color);
  const hexSpan = el('span', { class: 'hex-code', id: 'accent-hex-readout', text: currentHex.toUpperCase() });
  const colorInput = el('input', {
    type: 'color',
    id: 'accent-color-input',
    value: currentHex,
    onInput: (e) => {
      const nextHex = normalizeHexColor(e.target.value, currentHex);
      ap.accent_color = nextHex;
      hexSpan.textContent = nextHex.toUpperCase();
      document.querySelectorAll('.accent-dot-btn').forEach((btn) => {
        btn.classList.toggle('is-selected', btn.dataset.hex === nextHex);
      });
      setDirtyState(true);
      scheduleLiveAppearancePreview();
    },
  });

  const dotWrap = el('div', { class: 'accent-controls' }, [
    ...QUICK_ACCENTS.map((hex) =>
      el('button', {
        type: 'button',
        class: `accent-dot-btn ${currentHex === hex ? 'is-selected' : ''}`,
        'data-hex': hex,
        title: hex.toUpperCase(),
        style: { background: hex },
        onClick: () => {
          ap.accent_color = hex;
          colorInput.value = hex;
          hexSpan.textContent = hex.toUpperCase();
          document.querySelectorAll('.accent-dot-btn').forEach((b) => {
            b.classList.toggle('is-selected', b.dataset.hex === hex);
          });
          setDirtyState(true);
          scheduleLiveAppearancePreview();
        },
      })
    ),
    el('label', { class: 'color-picker-wrap', title: 'Custom accent color' }, [colorInput, hexSpan]),
  ]);

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Theme & Accent' }),
        el('span', { text: 'Applies to Island, Settings, and Tray Menu' }),
      ]),
      themeGrid,
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'Accent Color' }),
          el('div', { class: 'row-desc', text: 'Highlights active switches, progress bars, and sliders across all themes.' }),
        ]),
        dotWrap,
      ]),
    ])
  );

  // Dimensions & Glass sliders
  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Glass & Proportions' }),
        el('span', { text: 'Drag sliders to preview live' }),
      ]),
      sliderField({
        id: 'slider-opacity',
        title: 'Glass Opacity',
        desc: 'Adjust how translucent or solid the dark glass surface appears.',
        value: Number(ap.opacity ?? 0.88),
        min: 0.45,
        max: 0.98,
        step: 0.01,
        defaultValue: 0.88,
        format: (v) => `${Math.round(v * 100)}%`,
        onChange: (v) => {
          ap.opacity = Number(v.toFixed(2));
        },
        livePreview: true,
      }),
      sliderField({
        id: 'slider-scale',
        title: 'Interface Scale',
        desc: 'Resize the entire Dynamic Island proportionally for your screen.',
        value: Number(ap.scale ?? 1.0),
        min: 0.80,
        max: 1.35,
        step: 0.05,
        defaultValue: 1.0,
        format: (v) => `${Math.round(v * 100)}%`,
        onChange: (v) => {
          ap.scale = Number(v.toFixed(2));
        },
        livePreview: true,
      }),
      sliderField({
        id: 'slider-compact-width',
        title: 'Compact Width',
        desc: 'Width of the floating capsule while idling at the top edge.',
        value: Number(ap.compact_width ?? 268),
        min: 210,
        max: 380,
        step: 2,
        defaultValue: 268,
        format: (v) => `${Math.round(v)}px`,
        onChange: (v) => {
          ap.compact_width = Math.round(v);
        },
        livePreview: true,
      }),
    ])
  );

  updateLivePreviewBox();
}

function getMonitorLogicalBounds(monitorName) {
  const fallback = monitors.find((m) => m.is_primary) || monitors[0];
  const m = (monitorName && monitors.find((item) => item.name === monitorName)) || fallback;
  const scale = Math.max(1, Number(m?.scale_factor || 1));
  const monW = Math.max(800, Math.round(Number(m?.width || 1920) / scale));
  const monH = Math.max(540, Math.round(Number(m?.height || 1080) / scale));
  const maxTop = Math.max(120, monH - 68);
  const maxX = Math.max(240, Math.round(monW / 2 - 120));
  return { monW, monH, maxTop, maxX, scale, name: m?.name || 'Primary Display' };
}

function formatPositionReadout(topOffset, xOffset) {
  const sx = xOffset > 0 ? `+${Math.round(xOffset)}` : `${Math.round(xOffset)}`;
  return `X ${sx}px · Y ${Math.round(topOffset)}px`;
}

function syncRepositionModeButton() {
  const btn = document.getElementById('btn-reposition-desktop');
  if (!btn) return;
  btn.textContent = repositionModeActive ? 'Done Moving' : 'Move on Desktop';
  btn.classList.toggle('is-active', repositionModeActive);
}

function syncBehaviorPositionWidgets() {
  if (!config?.appearance) return;
  const ap = config.appearance;
  const bounds = getMonitorLogicalBounds(ap.monitor_name);
  const topVal = Math.max(0, Math.min(bounds.maxTop, Math.round(Number(ap.top_offset ?? 10))));
  const xVal = Math.max(-bounds.maxX, Math.min(bounds.maxX, Math.round(Number(ap.x_offset ?? 0))));
  ap.top_offset = topVal;
  ap.x_offset = xVal;

  const topSlider = document.getElementById('slider-top-offset');
  const topReadout = document.getElementById('slider-top-offset-value');
  if (topSlider) {
    topSlider.value = String(topVal);
    updateRangeTrackFill(topSlider);
    const resetLink = topSlider.closest('.slider-row')?.querySelector('.slider-reset-link');
    if (resetLink) resetLink.classList.toggle('is-hidden', Math.abs(topVal - 10) < 0.5);
  }
  if (topReadout) topReadout.textContent = `${topVal}px`;

  const xSlider = document.getElementById('slider-x-offset');
  const xReadout = document.getElementById('slider-x-offset-value');
  if (xSlider) {
    xSlider.value = String(xVal);
    updateRangeTrackFill(xSlider);
    const resetLink = xSlider.closest('.slider-row')?.querySelector('.slider-reset-link');
    if (resetLink) resetLink.classList.toggle('is-hidden', Math.abs(xVal) < 2.5);
  }
  if (xReadout) xReadout.textContent = `${xVal > 0 ? '+' : ''}${xVal}px`;

  const padReadout = document.getElementById('position-pad-readout');
  if (padReadout) padReadout.textContent = formatPositionReadout(topVal, xVal);

  const pill = document.getElementById('position-pad-pill');
  if (pill) {
    const xNorm = (xVal + bounds.maxX) / Math.max(1, bounds.maxX * 2);
    const yNorm = topVal / Math.max(1, bounds.maxTop);
    pill.style.left = `${(12 + xNorm * 76).toFixed(2)}%`;
    pill.style.top = `${(6 + yNorm * 82).toFixed(2)}%`;
  }

  document.querySelectorAll('.btn-preset[data-preset]').forEach((btn) => {
    const px = Number(btn.dataset.x);
    const py = Number(btn.dataset.y);
    btn.classList.toggle('is-active', Math.abs(px - xVal) <= 6 && Math.abs(py - topVal) <= 6);
  });

  syncRepositionModeButton();
}

function renderBehaviorSection(container) {
  const ap = config.appearance;
  const bounds = getMonitorLogicalBounds(ap.monitor_name);
  ap.top_offset = Math.max(0, Math.min(bounds.maxTop, Math.round(Number(ap.top_offset ?? 10))));
  ap.x_offset = Math.max(-bounds.maxX, Math.min(bounds.maxX, Math.round(Number(ap.x_offset ?? 0))));

  const monitorSelect = el('select', {
    class: 'select-field',
    id: 'select-monitor',
    onChange: (e) => {
      ap.monitor_name = e.target.value || null;
      const nextBounds = getMonitorLogicalBounds(ap.monitor_name);
      ap.top_offset = Math.max(0, Math.min(nextBounds.maxTop, Math.round(Number(ap.top_offset ?? 10))));
      ap.x_offset = Math.max(-nextBounds.maxX, Math.min(nextBounds.maxX, Math.round(Number(ap.x_offset ?? 0))));
      setDirtyState(true);
      scheduleLiveAppearancePreview();
      renderSection();
    },
  }, [
    el('option', { value: '', text: 'Auto (Primary Display)' }),
    ...monitors.map((m) =>
      el('option', {
        value: m.name,
        text: `${m.name} (${m.width}×${m.height})${m.is_primary ? ' · Primary' : ''}`,
      })
    ),
  ]);
  monitorSelect.value = ap.monitor_name || '';

  const autoCollapseSelect = el('select', {
    class: 'select-field',
    id: 'select-autocollapse',
    onChange: (e) => {
      ap.auto_collapse_ms = Number(e.target.value);
      setDirtyState(true);
    },
  }, [
    el('option', { value: '0', text: 'Manual / Click Outside Only' }),
    el('option', { value: '4000', text: 'After 4 seconds idle' }),
    el('option', { value: '8000', text: 'After 8 seconds idle' }),
    el('option', { value: '15000', text: 'After 15 seconds idle' }),
    el('option', { value: '30000', text: 'After 30 seconds idle' }),
  ]);
  autoCollapseSelect.value = String(ap.auto_collapse_ms || 0);

  const applyNewPosition = (nextTop, nextX) => {
    ap.top_offset = Math.max(0, Math.min(bounds.maxTop, Math.round(nextTop)));
    ap.x_offset = Math.max(-bounds.maxX, Math.min(bounds.maxX, Math.round(nextX)));
    setDirtyState(true);
    syncBehaviorPositionWidgets();
    scheduleLiveAppearancePreview();
  };

  const padPill = el('div', { class: 'position-pad-pill', id: 'position-pad-pill' });
  const padStage = el(
    'div',
    {
      class: 'position-pad-stage',
      id: 'position-pad-stage',
      title: 'Click or drag to reposition Dynamic Island on your monitor',
      style: { aspectRatio: `${bounds.monW} / ${bounds.monH}` },
    },
    [padPill]
  );

  let padDragging = false;
  const updateFromPointerEvent = (e) => {
    const rect = padStage.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const relX = (e.clientX - rect.left) / rect.width;
    const relY = (e.clientY - rect.top) / rect.height;
    const xNorm = Math.max(0, Math.min(1, (relX - 0.12) / 0.76));
    const yNorm = Math.max(0, Math.min(1, (relY - 0.06) / 0.82));
    let nextX = Math.round((xNorm * 2 - 1) * bounds.maxX);
    if (Math.abs(nextX) <= 16) nextX = 0;
    const nextTop = Math.round(yNorm * bounds.maxTop);
    applyNewPosition(nextTop, nextX);
  };

  padStage.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    padDragging = true;
    padStage.classList.add('is-dragging');
    padStage.setPointerCapture?.(e.pointerId);
    updateFromPointerEvent(e);
  });
  padStage.addEventListener('pointermove', (e) => {
    if (!padDragging) return;
    updateFromPointerEvent(e);
  });
  const endPadDrag = (e) => {
    if (!padDragging) return;
    padDragging = false;
    padStage.classList.remove('is-dragging');
    if (e?.pointerId != null && padStage.hasPointerCapture?.(e.pointerId)) {
      padStage.releasePointerCapture(e.pointerId);
    }
  };
  padStage.addEventListener('pointerup', endPadDrag);
  padStage.addEventListener('pointercancel', endPadDrag);

  const presets = [
    { id: 'top-left', label: 'Top Left', x: -bounds.maxX, y: 10 },
    { id: 'top-center', label: 'Top Center', x: 0, y: 10 },
    { id: 'top-right', label: 'Top Right', x: bounds.maxX, y: 10 },
    { id: 'center', label: 'Center', x: 0, y: Math.round((bounds.monH - 44) / 2) },
    { id: 'bottom-center', label: 'Bottom Center', x: 0, y: Math.max(0, bounds.maxTop - 48) },
  ];

  const desktopMoveBtn = el('button', {
    type: 'button',
    class: `btn-preset is-desktop-move ${repositionModeActive ? 'is-active' : ''}`,
    id: 'btn-reposition-desktop',
    text: repositionModeActive ? 'Done Moving' : 'Move on Desktop',
    onClick: () => {
      if (!invoke) return;
      invoke('set_reposition_mode', { enabled: !repositionModeActive }).catch(() => {});
    },
  });

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Screen Position' }),
        el('span', { text: `${bounds.monW}×${bounds.monH} logical workspace` }),
      ]),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'Target Monitor' }),
          el('div', { class: 'row-desc', text: 'Select which monitor displays the floating Dynamic Island.' }),
        ]),
        monitorSelect,
      ]),
      el('div', { class: 'position-pad-wrap' }, [
        el('div', { class: 'position-pad-top' }, [
          el('span', { class: 'position-pad-title', text: 'Interactive Screen Positioner' }),
          el('span', {
            class: 'position-pad-readout',
            id: 'position-pad-readout',
            text: formatPositionReadout(ap.top_offset, ap.x_offset),
          }),
        ]),
        padStage,
        el('div', { class: 'position-presets' }, [
          ...presets.map((p) =>
            el('button', {
              type: 'button',
              class: 'btn-preset',
              'data-preset': p.id,
              'data-x': String(p.x),
              'data-y': String(p.y),
              text: p.label,
              onClick: () => applyNewPosition(p.y, p.x),
            })
          ),
          desktopMoveBtn,
        ]),
        el('div', {
          class: 'row-desc',
          text: 'Drag on the map above, pick a preset, or click Move on Desktop to drag the live capsule directly across your screen.',
        }),
      ]),
      sliderField({
        id: 'slider-top-offset',
        title: 'Vertical Position (From Top)',
        desc: 'Move the capsule anywhere from the top edge down to the bottom of your monitor.',
        value: Number(ap.top_offset ?? 10),
        min: 0,
        max: bounds.maxTop,
        step: 1,
        defaultValue: 10,
        format: (v) => `${Math.round(v)}px`,
        onChange: (v) => {
          ap.top_offset = Math.round(v);
          syncBehaviorPositionWidgets();
        },
        livePreview: true,
      }),
      sliderField({
        id: 'slider-x-offset',
        title: 'Horizontal Position',
        desc: 'Shift left or right from screen center (0px = perfectly centered).',
        value: Number(ap.x_offset ?? 0),
        min: -bounds.maxX,
        max: bounds.maxX,
        step: 5,
        defaultValue: 0,
        format: (v) => `${v > 0 ? '+' : ''}${Math.round(v)}px`,
        onChange: (v) => {
          ap.x_offset = Math.round(v);
          syncBehaviorPositionWidgets();
        },
        livePreview: true,
      }),
    ])
  );

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Gestures & Behavior' }),
        el('span', { text: 'Keep interactions effortless and distraction-free' }),
      ]),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'Auto-Collapse Expanded Hub' }),
          el('div', { class: 'row-desc', text: 'Automatically return to compact mode when not pinned or hovered.' }),
        ]),
        autoCollapseSelect,
      ]),
      toggleRow(
        'Minimal Idle Capsule',
        'Show only the active track or alert in compact mode to avoid screen clutter.',
        ap.minimal_idle !== false,
        (v) => { ap.minimal_idle = v; }
      ),
      toggleRow(
        'Hover Quick-Peek',
        'Reveal playback controls or expand hint softly when hovering compact mode.',
        ap.hover_peek !== false,
        (v) => { ap.hover_peek = v; }
      ),
      toggleRow(
        'Auto-Expand on Hover',
        'Open the full panel immediately on hover instead of waiting for a click.',
        ap.hover_expand,
        (v) => { ap.hover_expand = v; }
      ),
      toggleRow(
        'Click Outside to Collapse',
        'Clicking anywhere outside the expanded capsule closes it automatically.',
        ap.click_outside_dismiss !== false,
        (v) => { ap.click_outside_dismiss = v; }
      ),
      toggleRow(
        'Volume & Brightness Popups',
        'Briefly display changes in the capsule when using media or brightness keys.',
        ap.show_volume_hud !== false,
        (v) => { ap.show_volume_hud = v; }
      ),
      toggleRow(
        'Reduced Motion',
        'Use calm fades instead of spring resize transitions.',
        ap.reduced_motion,
        (v) => {
          ap.reduced_motion = v;
          scheduleLiveAppearancePreview();
        }
      ),
      toggleRow(
        'Intuitive Emojis',
        'Display expressive status, SMS, weather, media, and power emojis across the Island & Tray.',
        ap.show_emojis !== false,
        (v) => {
          ap.show_emojis = v;
          scheduleLiveAppearancePreview();
        },
        { id: 'toggle-show-emojis' }
      ),
      toggleRow(
        'SMS Arrival Ding Effect',
        'Trigger a glowing capsule shimmer and priority badge when an SMS or message arrives.',
        ap.sms_ding_enabled !== false,
        (v) => {
          ap.sms_ding_enabled = v;
          scheduleLiveAppearancePreview();
        },
        { id: 'toggle-sms-ding-enabled' }
      ),
      toggleRow(
        'SMS Chime Sound',
        'Play a soft crystalline ding tone when an SMS arrives (muted during Do Not Disturb & Focus Mode).',
        ap.sms_ding_sound !== false,
        (v) => {
          ap.sms_ding_sound = v;
          scheduleLiveAppearancePreview();
        },
        { id: 'toggle-sms-ding-sound' }
      ),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'Preview SMS Arrival Ding' }),
          el('div', { class: 'row-desc', text: 'Test the live Dynamic Island SMS chime sound and shimmer animation immediately.' }),
        ]),
        el('div', { class: 'row-control' }, [
          el('button', {
            type: 'button',
            id: 'btn-settings-test-sms',
            class: 'btn-secondary',
            text: '💬 Test SMS Ding',
            onClick: () => {
              if (invoke) {
                invoke('trigger_test_sms_ding').catch((err) => showBanner(String(err), 'err'));
              } else {
                playWebDing();
              }
            },
          }),
        ]),
      ]),
    ])
  );

  syncBehaviorPositionWidgets();
}

function renderModulesSection(container) {
  const mods = config.modules;

  const grid = el('div', { class: 'module-grid' }, MODULE_META.map(([key, label, desc]) => {
    const isAi = key === 'ai';
    const checked = isAi ? (mods.ai !== false && config.ai.enabled) : Boolean(mods[key]);
    const input = el('input', { type: 'checkbox' });
    input.checked = checked;
    input.addEventListener('change', (e) => {
      const next = Boolean(e.target.checked);
      mods[key] = next;
      if (isAi) {
        config.ai.enabled = next;
      }
      setDirtyState(true);
      if (isAi) {
        renderSection();
      }
    });

    return el('div', { class: 'module-tile' }, [
      el('div', { class: 'row-info' }, [
        el('div', { class: 'row-title', text: label }),
        el('div', { class: 'row-desc', text: desc }),
      ]),
      el('label', { class: 'switch' }, [input, el('span', { class: 'switch-track' })]),
    ]);
  }));

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Enabled Modules' }),
        el('span', { text: 'Turn off unused tabs to keep your Island minimal' }),
      ]),
      grid,
    ])
  );

  // Weather & Gaming preferences
  const unitSelect = el('select', {
    class: 'select-field',
    onChange: (e) => {
      config.widgets.temperature_unit = e.target.value;
      setDirtyState(true);
    },
  }, [
    el('option', { value: 'C', text: 'Celsius (°C)' }),
    el('option', { value: 'F', text: 'Fahrenheit (°F)' }),
  ]);
  unitSelect.value = config.widgets.temperature_unit || 'C';

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Clock, Weather & Gaming' }),
        el('span', { text: 'Fine-tune widgets and automatic game detection' }),
      ]),
      toggleRow(
        '24-Hour Clock Format',
        'Use 24-hour time in the compact pill and clock widget (turn off for AM/PM).',
        config.widgets.clock_24h !== false,
        (v) => { config.widgets.clock_24h = v; }
      ),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'Weather City' }),
          el('div', { class: 'row-desc', text: 'Used with Open-Meteo when weather widget is active.' }),
        ]),
        el('div', { class: 'row-control' }, [
          el('input', {
            class: 'input-field',
            value: config.widgets.weather_city || '',
            placeholder: 'e.g. London, Tokyo',
            onInput: (e) => {
              config.widgets.weather_city = e.target.value;
              setDirtyState(true);
            },
          }),
          unitSelect,
        ]),
      ]),
      toggleRow(
        'Auto-Detect Fullscreen Games',
        'Switch to Gaming Focus activity when a known or custom game runs.',
        config.gaming.auto_detect,
        (v) => { config.gaming.auto_detect = v; }
      ),
      toggleRow(
        'Click-Through HUD While Gaming',
        'Prevent accidental mouse captures over the Island during gameplay.',
        config.gaming.click_through_hud,
        (v) => { config.gaming.click_through_hud = v; }
      ),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'Custom Game Executables' }),
          el('div', { class: 'row-desc', text: 'Comma-separated process names (e.g. mygame.exe, rocket.exe).' }),
        ]),
        el('input', {
          class: 'input-field',
          style: { width: '240px' },
          value: (config.gaming.custom_processes || []).join(', '),
          placeholder: 'game.exe, client.exe',
          onInput: (e) => {
            config.gaming.custom_processes = e.target.value
              .split(',')
              .map((s) => s.trim())
              .filter(Boolean);
            setDirtyState(true);
          },
        }),
      ]),
    ])
  );

  // Performance + AI section
  const aiEnabled = mods.ai !== false && config.ai.enabled;
  const perfAndAi = [
    el('div', { class: 'card-heading' }, [
      el('h2', { text: 'Performance & AI Engine' }),
      el('span', { text: 'Low-CPU background polling and local LLM settings' }),
    ]),
    sliderField({
      id: 'slider-hw-interval',
      title: 'Telemetry Refresh Interval',
      desc: 'Higher values reduce background CPU usage; battery mode doubles this automatically.',
      value: Number(config.performance.hardware_interval_ms ?? 2000),
      min: 1000,
      max: 10000,
      step: 500,
      defaultValue: 2000,
      format: (v) => `${(v / 1000).toFixed(1)}s`,
      onChange: (v) => {
        config.performance.hardware_interval_ms = Math.round(v);
        config.performance.network_interval_ms = Math.round(v);
      },
    }),
    toggleRow(
      'Battery Saver Polling',
      'Reduce sensor refresh frequency automatically when running on battery.',
      config.performance.low_power_on_battery,
      (v) => { config.performance.low_power_on_battery = v; }
    ),
  ];

  if (aiEnabled) {
    const providerSelect = el('select', {
      class: 'select-field',
      onChange: (e) => {
        config.ai.provider = e.target.value;
        setDirtyState(true);
      },
    }, [
      el('option', { value: 'ollama', text: 'Ollama (Local)' }),
      el('option', { value: 'openai_compatible', text: 'OpenAI-Compatible Endpoint' }),
    ]);
    providerSelect.value = config.ai.provider || 'ollama';

    perfAndAi.push(
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'AI Provider & Model' }),
          el('div', { class: 'row-desc', text: 'Local Ollama runs privately without API keys.' }),
        ]),
        el('div', { class: 'row-control' }, [
          providerSelect,
          el('input', {
            class: 'input-field',
            style: { width: '130px' },
            value: config.ai.model || 'llama3.2',
            placeholder: 'Model name',
            onInput: (e) => {
              config.ai.model = e.target.value;
              setDirtyState(true);
            },
          }),
        ]),
      ]),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'AI Endpoint URL' }),
          el('div', { class: 'row-desc', text: 'Default Ollama endpoint: http://127.0.0.1:11434' }),
        ]),
        el('input', {
          class: 'input-field',
          style: { width: '250px' },
          value: config.ai.endpoint || 'http://127.0.0.1:11434',
          onInput: (e) => {
            config.ai.endpoint = e.target.value;
            setDirtyState(true);
          },
        }),
      ])
    );
  }

  container.appendChild(el('div', { class: 'setting-card' }, perfAndAi));
}

function buildComboFromKeyEvent(e) {
  const ignoreOnly = ['Control', 'Shift', 'Alt', 'Meta', 'OS', 'Tab'];
  if (ignoreOnly.includes(e.key)) return null;
  const parts = [];
  if (e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  if (parts.length === 0) return null; // require at least one modifier for global hotkeys
  const specialKeys = {
    ' ': 'Space',
    '+': 'Plus',
    ArrowUp: 'Up',
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right',
  };
  const keyName = specialKeys[e.key] || (e.key.length === 1 ? e.key.toUpperCase() : e.key);
  parts.push(keyName);
  return parts.join('+');
}

function renderShortcutsSection(container) {
  const hotkeysMeta = [
    ['toggle_expand', 'Expand / Collapse Island', 'Toggle compact and full workspace view'],
    ['toggle_visibility', 'Show / Hide Island', 'Completely hide or restore the floating capsule'],
    ['toggle_dnd', 'Do Not Disturb', 'Mute non-critical alerts with a quick banner'],
    ['clipboard_drawer', 'Open Clipboard Drawer', 'Jump straight to recent clipboard items'],
  ];

  const hkRows = hotkeysMeta.map(([key, label, desc]) => {
    const isRecording = recordingHotkeyKey === key;
    const input = el('input', {
      class: 'input-field hotkey-input',
      value: config.hotkeys[key] || '',
      placeholder: 'Not assigned',
      onInput: (e) => {
        config.hotkeys[key] = e.target.value;
        setDirtyState(true);
      },
    });
    const recBtn = el('button', {
      type: 'button',
      class: `btn-record ${isRecording ? 'is-recording' : ''}`,
      text: isRecording ? 'Press Keys…' : 'Record',
      onClick: () => {
        recordingHotkeyKey = isRecording ? null : key;
        renderSection();
      },
    });
    const clearBtn = el('button', {
      type: 'button',
      class: 'btn-mini',
      text: 'Clear',
      onClick: () => {
        config.hotkeys[key] = '';
        recordingHotkeyKey = null;
        setDirtyState(true);
        renderSection();
      },
    });

    return el('div', { class: 'setting-row' }, [
      el('div', { class: 'row-info' }, [
        el('div', { class: 'row-title', text: label }),
        el('div', { class: 'row-desc', text: desc }),
      ]),
      el('div', { class: 'row-control' }, [input, recBtn, clearBtn]),
    ]);
  });

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Global Keyboard Shortcuts' }),
        el('span', { text: 'Click Record and press modifier + key (or type e.g. Alt+Shift+Space)' }),
      ]),
      ...hkRows,
    ])
  );

  // App Rules (Profiles) CRUD
  const profileRows = (config.profiles || []).map((prof, idx) =>
    el('div', { class: 'list-item-row' }, [
      el('div', { class: 'list-item-meta' }, [
        el('div', { class: 'list-item-title', text: `${prof.name} (${prof.process_name})` }),
        el('div', {
          class: 'list-item-sub',
          text: `${prof.dnd ? 'Auto DND On' : 'Alerts Active'}${prof.active_tab ? ` · Opens: ${prof.active_tab}` : ''}`,
        }),
      ]),
      el('button', {
        type: 'button',
        class: 'btn-delete',
        onClick: () => {
          config.profiles.splice(idx, 1);
          setDirtyState(true);
          renderSection();
        },
      }, 'Remove'),
    ])
  );

  const profNameIn = el('input', { class: 'input-field', placeholder: 'Rule name (e.g. Meet)', style: { flex: '1', minWidth: '120px' } });
  const profExeIn = el('input', { class: 'input-field', placeholder: 'Process (e.g. zoom.exe)', style: { flex: '1', minWidth: '135px' } });
  const profTabSelect = el('select', { class: 'select-field' }, TAB_CHOICES.map(([val, lbl]) => el('option', { value: val, text: lbl })));
  const profDndCheck = el('input', { type: 'checkbox' });

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Per-App Rules & Profiles' }),
        el('span', { text: 'Automatically mute alerts or switch tabs when an app gains focus' }),
      ]),
      ...(profileRows.length ? profileRows : [el('div', { class: 'row-desc', text: 'No application rules configured.' })]),
      el('div', { class: 'inline-add-form' }, [
        profNameIn,
        profExeIn,
        profTabSelect,
        el('label', { style: { display: 'inline-flex', alignItems: 'center', gap: '6px', fontSize: '12px', color: 'var(--text-2)' } }, [
          profDndCheck,
          'Auto DND',
        ]),
        el('button', {
          type: 'button',
          class: 'btn-secondary',
          onClick: () => {
            const name = profNameIn.value.trim();
            const proc = profExeIn.value.trim();
            if (!name || !proc) {
              showBanner('Enter both a profile name and an executable (e.g. code.exe).', 'warn');
              return;
            }
            config.profiles.push({
              id: `prof-${Date.now()}`,
              name,
              process_name: proc.toLowerCase(),
              dnd: Boolean(profDndCheck.checked),
              theme_override: null,
              active_tab: profTabSelect.value || null,
            });
            setDirtyState(true);
            renderSection();
          },
        }, 'Add Rule'),
      ]),
    ])
  );

  // Quick Actions CRUD
  const qaRows = (config.quick_actions || []).map((qa, idx) =>
    el('div', { class: 'list-item-row' }, [
      el('div', { class: 'list-item-meta' }, [
        el('div', { class: 'list-item-title', text: qa.label }),
        el('div', { class: 'list-item-sub', text: `${qa.kind} · ${qa.target}` }),
      ]),
      el('button', {
        type: 'button',
        class: 'btn-delete',
        onClick: () => {
          config.quick_actions.splice(idx, 1);
          setDirtyState(true);
          renderSection();
        },
      }, 'Remove'),
    ])
  );

  const qaLabelIn = el('input', { class: 'input-field', placeholder: 'Label (e.g. GitHub)', style: { flex: '1', minWidth: '120px' } });
  const qaTargetIn = el('input', { class: 'input-field', placeholder: 'https://..., ms-settings:..., or wt', style: { flex: '1.5', minWidth: '170px' } });

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Quick Actions' }),
        el('span', { text: 'Shown in Overview top row and Tools -> Quick Actions' }),
      ]),
      ...(qaRows.length ? qaRows : [el('div', { class: 'row-desc', text: 'No quick actions added yet.' })]),
      el('div', { class: 'inline-add-form' }, [
        qaLabelIn,
        qaTargetIn,
        el('button', {
          type: 'button',
          class: 'btn-secondary',
          onClick: async () => {
            const label = qaLabelIn.value.trim();
            const target = qaTargetIn.value.trim();
            if (!label || !target) {
              showBanner('Enter both a shortcut label and target command/URL.', 'warn');
              return;
            }
            const kind = target.startsWith('http://') || target.startsWith('https://') || target.startsWith('ms-')
              ? 'uri'
              : (target.toLowerCase().endsWith('.exe') && (target.includes('\\') || target.includes('/')))
                ? 'app'
                : 'system_tool';
            try {
              const cleanTarget = invoke
                ? await invoke('validate_quick_action_cmd', { kind, target })
                : target;
              config.quick_actions.push({
                id: `qa-${Date.now()}`,
                label,
                icon: 'zap',
                kind,
                target: cleanTarget,
              });
              setDirtyState(true);
              showBanner('');
              renderSection();
            } catch (err) {
              showBanner(String(err), 'err');
            }
          },
        }, 'Add Shortcut'),
      ]),
    ])
  );
}

function renderSystemSection(container) {
  const priv = config.privacy;

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Startup & Focus Modes' }),
        el('span', { text: 'Also accessible directly from the system tray popup' }),
      ]),
      toggleRow(
        'Launch PILLVYN at Windows Startup',
        'Enabled by default — starts quietly in Dynamic Island mode when signing in to Windows.',
        config.autostart !== false,
        (v) => { config.autostart = v; }
      ),
      toggleRow(
        'Do Not Disturb Mode',
        'Suppress normal application popups; keep critical hardware and battery warnings.',
        priv.dnd_mode,
        (v) => { priv.dnd_mode = v; updateLivePreviewBox(); }
      ),
      toggleRow(
        'Focus Mode',
        'Keep the capsule calm and minimal while working or presenting.',
        priv.focus_mode,
        (v) => { priv.focus_mode = v; updateLivePreviewBox(); }
      ),
    ])
  );

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Clipboard & Privacy' }),
        el('span', { text: 'History stays local in memory only' }),
      ]),
      toggleRow(
        'Capture Clipboard History',
        'Allow quick access to recent text and image copies inside the Clip tab.',
        priv.clipboard_enabled,
        (v) => { priv.clipboard_enabled = v; }
      ),
      toggleRow(
        'Filter Passwords & API Tokens',
        'Skip keys, JWTs, bearer headers, and high-entropy secrets automatically.',
        priv.clipboard_skip_sensitive,
        (v) => { priv.clipboard_skip_sensitive = v; }
      ),
      sliderField({
        id: 'slider-clipboard-max',
        title: 'Clipboard Item Limit',
        desc: 'Maximum number of recent copies retained in memory.',
        value: Number(priv.clipboard_max_items ?? 25),
        min: 5,
        max: 100,
        step: 5,
        defaultValue: 25,
        format: (v) => `${Math.round(v)} items`,
        onChange: (v) => { priv.clipboard_max_items = Math.round(v); },
      }),
    ])
  );

  // Updates + Plugins folder info
  const updateBody = el('div', { class: 'list-item-sub', text: `Installed version: v${appVersion} · Repo: ${priv.update_repo || 'EbramJames/PILLVYN'}` });
  const updateOpenBtn = el('button', {
    type: 'button',
    class: 'btn-mini',
    style: { display: 'none' },
    text: 'Open Release',
    onClick: () => {
      if (updateResult?.release_url && invoke) {
        invoke('open_url', { url: updateResult.release_url }).catch((e) => showBanner(String(e), 'err'));
      }
    },
  });

  if (updateResult) {
    updateBody.textContent = updateResult.update_available
      ? `Update available: v${updateResult.latest_version} (current v${updateResult.current_version})`
      : `Up to date (v${updateResult.current_version})`;
    if (updateResult.release_url) {
      updateOpenBtn.style.display = 'inline-flex';
    }
  }

  container.appendChild(
    el('div', { class: 'setting-card' }, [
      el('div', { class: 'card-heading' }, [
        el('h2', { text: 'Updates & Extensions' }),
        el('span', { text: 'Updates are checked only when requested' }),
      ]),
      toggleRow(
        'Allow Update Checks',
        'Enable manual GitHub Release checks from Settings Studio.',
        priv.update_check_consent,
        (v) => { priv.update_check_consent = v; }
      ),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'PILLVYN Releases' }),
          updateBody,
        ]),
        el('div', { class: 'row-control' }, [
          updateOpenBtn,
          el('button', {
            type: 'button',
            class: 'btn-secondary',
            onClick: async () => {
              if (!priv.update_check_consent) {
                showBanner('Enable "Allow Update Checks" above (or click Save Changes) first.', 'warn');
                return;
              }
              updateBody.textContent = 'Checking GitHub Releases…';
              try {
                updateResult = await invoke('check_updates', {
                  consentOverride: Boolean(priv.update_check_consent),
                  repoOverride: priv.update_repo || null,
                });
                renderSection();
              } catch (err) {
                showBanner(`Update check failed: ${err}`, 'err');
                updateBody.textContent = `Check failed: ${err}`;
              }
            },
          }, 'Check Now'),
        ]),
      ]),
      el('div', { class: 'setting-row' }, [
        el('div', { class: 'row-info' }, [
          el('div', { class: 'row-title', text: 'Installed Plugins' }),
          el('div', {
            class: 'row-desc',
            text: plugins ? `${plugins.plugins.length} plugin(s) in ${plugins.plugins_dir}` : 'Local manifest plugins directory',
          }),
        ]),
        el('button', {
          type: 'button',
          class: 'btn-secondary',
          onClick: async () => {
            try {
              plugins = await invoke('plugin_action', { action: 'create_sample' });
              showBanner('Created sample Pomodoro plugin in plugins folder.', 'ok');
              renderSection();
            } catch (err) {
              showBanner(String(err), 'err');
            }
          },
        }, 'Create Sample Plugin'),
      ]),
    ])
  );
}

function renderSection() {
  const container = document.getElementById('settings-content');
  if (!container || !config) return;
  container.innerHTML = '';

  if (activeSection === 'appearance') renderAppearanceSection(container);
  else if (activeSection === 'behavior') renderBehaviorSection(container);
  else if (activeSection === 'modules') renderModulesSection(container);
  else if (activeSection === 'shortcuts') renderShortcutsSection(container);
  else if (activeSection === 'system') renderSystemSection(container);
}

async function handleSave() {
  if (!config || !invoke) return;
  const saveBtn = document.getElementById('btn-save');
  if (saveBtn) saveBtn.disabled = true;

  try {
    config.modules.ai = Boolean(config.ai.enabled && config.modules.ai);
    config.ai.enabled = Boolean(config.modules.ai);
    const outcome = await invoke('save_settings_config', { newConfig: config });
    config = structuredClone(outcome.config);
    savedConfig = structuredClone(outcome.config);
    setDirtyState(false);
    syncSettingsTheme();
    renderSection();
    if (outcome.warnings && outcome.warnings.length > 0) {
      showBanner(`Saved, with warning: ${outcome.warnings.join(' · ')}`, 'warn');
    } else {
      showBanner('Settings saved and synchronized across PILLVYN.', 'ok');
    }
  } catch (err) {
    showBanner(String(err), 'err');
  } finally {
    if (saveBtn) saveBtn.disabled = false;
  }
}

async function handleRevert() {
  if (!savedConfig) return;
  config = structuredClone(savedConfig);
  setDirtyState(false);
  recordingHotkeyKey = null;
  if (invoke) {
    await invoke('cancel_appearance_preview').catch(() => {});
  }
  syncSettingsTheme();
  renderSection();
  showBanner('Reverted unsaved edits back to last saved configuration.', 'ok');
}

async function handleResetDefaults() {
  const btn = document.getElementById('btn-reset');
  if (!resetConfirmArmed) {
    resetConfirmArmed = true;
    if (btn) {
      btn.textContent = 'Confirm Reset';
      btn.classList.add('is-danger-confirm');
    }
    clearTimeout(resetConfirmTimer);
    resetConfirmTimer = setTimeout(() => {
      resetConfirmArmed = false;
      if (btn) {
        btn.textContent = 'Reset Defaults';
        btn.classList.remove('is-danger-confirm');
      }
    }, 3500);
    return;
  }

  resetConfirmArmed = false;
  clearTimeout(resetConfirmTimer);
  if (btn) {
    btn.textContent = 'Reset Defaults';
    btn.classList.remove('is-danger-confirm');
  }

  try {
    const outcome = await invoke('save_settings_config', { newConfig: { version: 2 } });
    config = structuredClone(outcome.config);
    savedConfig = structuredClone(outcome.config);
    setDirtyState(false);
    syncSettingsTheme();
    updateSidebarStatus();
    renderSection();
    showBanner('All settings restored to factory defaults.', 'ok');
  } catch (err) {
    showBanner(String(err), 'err');
  }
}

async function init() {
  if (!invoke) return;

  try {
    const initPayload = await invoke('get_initial_state');
    savedConfig = structuredClone(initPayload.config);
    config = structuredClone(initPayload.config);
    monitors = initPayload.monitors || [];
    plugins = initPayload.plugins || null;
    appVersion = initPayload.app_version || '2.0.0';
    islandVisible = initPayload.island_visible !== false;
    islandExpanded = Boolean(initPayload.island_expanded);

    const verEl = document.getElementById('studio-version');
    if (verEl) verEl.textContent = `Studio · v${appVersion}`;

    syncSettingsTheme();
    updateSidebarStatus();
    setDirtyState(false);
    renderNav();
    renderSection();

    if (initPayload.config_warnings && initPayload.config_warnings.length > 0) {
      showBanner(`Startup warning: ${initPayload.config_warnings.join(' · ')}`, 'warn');
    }
  } catch (err) {
    showBanner(`Failed to initialize Settings Studio: ${err}`, 'err');
  }

  document.getElementById('btn-save')?.addEventListener('click', handleSave);
  document.getElementById('btn-revert')?.addEventListener('click', handleRevert);
  document.getElementById('btn-reset')?.addEventListener('click', handleResetDefaults);

  document.getElementById('btn-toggle-island')?.addEventListener('click', async () => {
    const snap = await invoke('tray_action', { action: 'toggle_visibility' }).catch(() => null);
    if (snap) {
      islandVisible = snap.visible;
      islandExpanded = snap.expanded;
      updateSidebarStatus();
    }
  });

  document.getElementById('btn-expand-island')?.addEventListener('click', async () => {
    const snap = await invoke('tray_action', { action: 'toggle_expand' }).catch(() => null);
    if (snap) {
      islandVisible = snap.visible;
      islandExpanded = snap.expanded;
      updateSidebarStatus();
    }
  });

  window.addEventListener('keydown', (e) => {
    if (recordingHotkeyKey) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Escape') {
        recordingHotkeyKey = null;
        renderSection();
        return;
      }
      const combo = buildComboFromKeyEvent(e);
      if (combo) {
        config.hotkeys[recordingHotkeyKey] = combo;
        recordingHotkeyKey = null;
        setDirtyState(true);
        renderSection();
      }
      return;
    }

    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      handleSave();
    }
  });

  if (listen) {
    listen('pillvyn://config', (e) => {
      if (!e.payload) return;
      savedConfig = structuredClone(e.payload);
      if (!dirty) {
        config = structuredClone(e.payload);
        syncSettingsTheme();
        renderSection();
      }
    });
    listen('pillvyn://position-updated', (e) => {
      if (!e.payload || !config?.appearance) return;
      config.appearance.top_offset = Number(e.payload.top_offset ?? config.appearance.top_offset);
      config.appearance.x_offset = Number(e.payload.x_offset ?? config.appearance.x_offset);
      if (e.payload.monitor_name !== undefined) {
        config.appearance.monitor_name = e.payload.monitor_name;
      }
      if (e.payload.committed) {
        savedConfig = structuredClone(config);
        setDirtyState(false);
      } else {
        setDirtyState(true);
      }
      if (activeSection === 'behavior') {
        syncBehaviorPositionWidgets();
      }
    });
    listen('pillvyn://reposition-mode', (e) => {
      repositionModeActive = Boolean(e.payload);
      syncRepositionModeButton();
    });
    listen('pillvyn://island-state', (e) => {
      if (!e.payload) return;
      islandVisible = e.payload.visible !== false;
      islandExpanded = Boolean(e.payload.expanded);
      updateSidebarStatus();
    });
  }
}

init();
