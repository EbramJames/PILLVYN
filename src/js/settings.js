import { clear, el, icon } from './utils.js';

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const listen = tauri?.event?.listen;

const SECTIONS = [
  { id: 'appearance', label: 'Appearance', icon: 'sun', desc: 'Customize theme, glass opacity, scale, and multi-monitor position.' },
  { id: 'modules', label: 'Modules', icon: 'grid', desc: 'Choose which Dynamic Island tabs and sensors stay active.' },
  { id: 'shortcuts', label: 'Shortcuts & Profiles', icon: 'zap', desc: 'Configure global hotkeys, Windows startup, and automatic app profiles.' },
  { id: 'gaming', label: 'Gaming & Widgets', icon: 'gamepad', desc: 'Configure game detection, weather location, and clock settings.' },
  { id: 'privacy', label: 'Privacy & Updates', icon: 'shield', desc: 'Control clipboard filters, DND, performance, and consented updates.' },
  { id: 'integrations', label: 'Plugins & AI', icon: 'sparkles', desc: 'Manage local plugins and optional Ollama or OpenAI-compatible AI.' },
];

let config = null;
let monitors = [];
let appVersion = '1.0.0';
let activeSection = 'appearance';
let dirty = false;
let updateStatus = null;

function showToast(msg) {
  const bar = document.getElementById('toast-bar');
  bar.textContent = msg;
  bar.classList.add('visible');
  setTimeout(() => bar.classList.remove('visible'), 2800);
}

function markDirty() {
  dirty = true;
  document.getElementById('dirty-badge').style.display = 'inline-flex';
}

function clearDirty() {
  dirty = false;
  document.getElementById('dirty-badge').style.display = 'none';
}

function toggleSwitch(checked, onChange) {
  return el('button', {
    type: 'button',
    class: 'toggle-switch',
    role: 'switch',
    'aria-checked': checked ? 'true' : 'false',
    onClick: () => {
      onChange(!checked);
      markDirty();
      renderSection();
    },
  });
}

function row(name, desc, control) {
  return el('div', { class: 'setting-row' }, [
    el('div', { class: 'setting-info' }, [
      el('div', { class: 'setting-name', text: name }),
      desc ? el('div', { class: 'setting-desc', text: desc }) : null,
    ]),
    el('div', { class: 'setting-control' }, control),
  ]);
}

function renderSidebar() {
  const nav = document.getElementById('settings-nav');
  clear(nav);
  for (const s of SECTIONS) {
    nav.appendChild(
      el('button', {
        class: `sidebar-item ${activeSection === s.id ? 'active' : ''}`,
        onClick: () => {
          activeSection = s.id;
          renderSidebar();
          renderSection();
        },
      }, [icon(s.icon, 15), el('span', { text: s.label })])
    );
  }
}

function renderSection() {
  if (!config) return;
  const meta = SECTIONS.find((s) => s.id === activeSection) || SECTIONS[0];
  document.getElementById('settings-title').textContent = meta.label;
  document.getElementById('settings-desc').textContent = meta.desc;
  document.body.setAttribute('data-theme', config.appearance.theme);
  document.documentElement.style.setProperty('--accent', config.appearance.accent_color);

  const container = document.getElementById('settings-content');
  clear(container);

  if (activeSection === 'appearance') {
    const themes = [
      { id: 'obsidian', label: 'Obsidian', color: '#141926' },
      { id: 'aurora', label: 'Aurora', color: '#133138' },
      { id: 'midnight', label: 'Midnight', color: '#15122b' },
      { id: 'crimson', label: 'Crimson', color: '#2b121b' },
      { id: 'frosted', label: 'Frosted', color: '#2b3347' },
    ];
    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Theme & Accent' }),
        el('div', { class: 'theme-grid' }, themes.map((t) =>
          el('button', {
            class: `theme-chip ${config.appearance.theme === t.id ? 'active' : ''}`,
            onClick: () => {
              config.appearance.theme = t.id;
              markDirty();
              renderSection();
            },
          }, [
            el('span', { class: 'theme-swatch', style: { background: t.color } }),
            el('span', { text: t.label }),
          ])
        )),
        row('Accent Color', 'Primary highlight and progress indicator color.', [
          el('input', {
            type: 'color',
            value: config.appearance.accent_color,
            onInput: (e) => {
              config.appearance.accent_color = e.target.value;
              markDirty();
              document.documentElement.style.setProperty('--accent', e.target.value);
            },
          }),
          el('span', { class: 'muted', text: config.appearance.accent_color }),
        ]),
        row('Glass Opacity', 'Translucency of the floating Dynamic Island capsule.', [
          el('input', {
            type: 'range',
            class: 'range-slider',
            min: '45',
            max: '98',
            value: String(Math.round(config.appearance.opacity * 100)),
            onInput: (e) => {
              config.appearance.opacity = Number(e.target.value) / 100;
              markDirty();
            },
          }),
          el('span', { class: 'card-value', text: `${Math.round(config.appearance.opacity * 100)}%` }),
        ]),
      ])
    );

    const monitorOpts = [
      el('option', { value: '', selected: !config.appearance.monitor_name ? 'selected' : null, text: 'Primary Monitor (Auto)' }),
      ...monitors.map((m) =>
        el('option', {
          value: m.name,
          selected: config.appearance.monitor_name === m.name ? 'selected' : null,
          text: `${m.name} (${m.width}×${m.height} @ ${Math.round(m.scale_factor * 100)}%)`,
        })
      ),
    ];

    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Size, Placement & Behavior' }),
        row('Target Monitor', 'Supports multi-monitor and mixed Windows DPI scaling.', [
          el('select', {
            class: 'select-glass',
            onChange: (e) => {
              config.appearance.monitor_name = e.target.value || null;
              markDirty();
            },
          }, monitorOpts),
        ]),
        row('Scale Factor', 'Scale the Dynamic Island UI cleanly on any screen.', [
          el('input', {
            type: 'range',
            class: 'range-slider',
            min: '80',
            max: '135',
            value: String(Math.round(config.appearance.scale * 100)),
            onInput: (e) => {
              config.appearance.scale = Number(e.target.value) / 100;
              markDirty();
            },
          }),
          el('span', { class: 'card-value', text: `${Math.round(config.appearance.scale * 100)}%` }),
        ]),
        row('Compact Width (px)', 'Width of the idle Dynamic Island capsule.', [
          el('input', {
            type: 'range',
            class: 'range-slider',
            min: '210',
            max: '380',
            value: String(config.appearance.compact_width),
            onInput: (e) => {
              config.appearance.compact_width = Number(e.target.value);
              markDirty();
            },
          }),
          el('span', { class: 'card-value', text: `${config.appearance.compact_width}px` }),
        ]),
        row('Top Offset (px)', 'Distance from the top edge of the selected display.', [
          el('input', {
            type: 'number',
            class: 'input-glass',
            min: '0',
            max: '120',
            value: String(config.appearance.top_offset),
            onInput: (e) => {
              config.appearance.top_offset = Number(e.target.value) || 0;
              markDirty();
            },
          }),
        ]),
        row('Hover to Expand', 'Automatically expand when the pointer rests over the capsule.', toggleSwitch(config.appearance.hover_expand, (v) => { config.appearance.hover_expand = v; })),
        row('Reduced Motion', 'Minimize spring animations for accessibility or low power.', toggleSwitch(config.appearance.reduced_motion, (v) => { config.appearance.reduced_motion = v; })),
      ])
    );
  } else if (activeSection === 'modules') {
    const labels = [
      ['music', 'Music Player', 'Windows GSMTC playback & album artwork'],
      ['notifications', 'Notifications', 'Priority alert hub and history'],
      ['system', 'System Controls', 'WASAPI volume, mic, audio devices, brightness, battery'],
      ['hardware', 'Hardware Monitor', 'CPU, RAM, GPU (NVML/DXGI/PDH), and Disk usage'],
      ['gaming', 'Gaming Mode', 'Active game detection, Steam integration & focus'],
      ['downloads', 'Download Monitor', 'Browser (.crdownload/.part) and Steam download progress'],
      ['clipboard', 'Clipboard Manager', 'Event-driven text and image clipboard history'],
      ['network', 'Network Monitor', 'Adapter Rx/Tx speed, ICMP ping, and VPN status'],
      ['actions', 'Quick Actions', 'Windows system tools and custom shortcuts'],
      ['widgets', 'Widgets', 'Clock, timers, stopwatch, and cached weather'],
      ['plugins', 'Plugins Drawer', 'Local manifest plugins and custom Live Activities'],
      ['ai', 'Optional AI Assistant', 'Opt-in local Ollama or OpenAI-compatible assistant'],
    ];

    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Active Island Modules' }),
        ...labels.map(([k, title, desc]) =>
          row(title, desc, toggleSwitch(Boolean(config.modules[k]), (v) => {
            config.modules[k] = v;
          }))
        ),
      ])
    );
  } else if (activeSection === 'shortcuts') {
    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Startup & Global Hotkeys' }),
        row('Launch on Windows Startup', 'Start PILLVYN automatically when signing into Windows.', toggleSwitch(config.autostart, (v) => { config.autostart = v; })),
        row('Expand / Collapse Hotkey', 'Global shortcut anywhere in Windows (e.g. Alt+Shift+Space).', [
          el('input', {
            class: 'input-glass',
            value: config.hotkeys.toggle_expand,
            onInput: (e) => { config.hotkeys.toggle_expand = e.target.value; markDirty(); },
          }),
        ]),
        row('Show / Hide Island Hotkey', 'Toggle island visibility.', [
          el('input', {
            class: 'input-glass',
            value: config.hotkeys.toggle_visibility,
            onInput: (e) => { config.hotkeys.toggle_visibility = e.target.value; markDirty(); },
          }),
        ]),
        row('Do Not Disturb Hotkey', 'Mute non-critical alerts globally.', [
          el('input', {
            class: 'input-glass',
            value: config.hotkeys.toggle_dnd,
            onInput: (e) => { config.hotkeys.toggle_dnd = e.target.value; markDirty(); },
          }),
        ]),
        row('Clipboard Drawer Hotkey', 'Jump straight to clipboard history.', [
          el('input', {
            class: 'input-glass',
            value: config.hotkeys.clipboard_drawer,
            onInput: (e) => { config.hotkeys.clipboard_drawer = e.target.value; markDirty(); },
          }),
        ]),
      ])
    );

    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Automatic Application Profiles' }),
        ...config.profiles.map((p, idx) =>
          row(p.name, `Triggers when ${p.process_name} is in foreground`, [
            el('input', {
              class: 'input-glass',
              style: { width: '130px' },
              value: p.process_name,
              onInput: (e) => { config.profiles[idx].process_name = e.target.value; markDirty(); },
            }),
            toggleSwitch(p.dnd, (v) => { config.profiles[idx].dnd = v; }),
          ])
        ),
      ])
    );
  } else if (activeSection === 'gaming') {
    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Gaming Mode' }),
        row('Auto Game Detection', 'Detect Steam RunningAppID, known games, and fullscreen apps.', toggleSwitch(config.gaming.auto_detect, (v) => { config.gaming.auto_detect = v; })),
        row('Suppress Notifications in Game', 'Automatically mute non-critical banners while gaming.', toggleSwitch(config.gaming.suppress_notifications, (v) => { config.gaming.suppress_notifications = v; })),
        row('Click-Through HUD in Game', 'Ignore mouse clicks on the island during active games.', toggleSwitch(config.gaming.click_through_hud, (v) => { config.gaming.click_through_hud = v; })),
        row('Custom Game Executables', 'Comma-separated process names (e.g. mygame.exe).', [
          el('input', {
            class: 'input-glass',
            value: (config.gaming.custom_processes || []).join(', '),
            onInput: (e) => {
              config.gaming.custom_processes = e.target.value.split(',').map((s) => s.trim()).filter(Boolean);
              markDirty();
            },
          }),
        ]),
      ])
    );

    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Widgets & Weather' }),
        row('Weather Integration', 'Fetch local weather via Open-Meteo with offline cache.', toggleSwitch(config.widgets.weather_enabled, (v) => { config.widgets.weather_enabled = v; })),
        row('Weather City', 'Leave "Auto (Local)" or type any city (e.g. London, Tokyo).', [
          el('input', {
            class: 'input-glass',
            value: config.widgets.weather_city,
            onInput: (e) => {
              config.widgets.weather_city = e.target.value;
              config.widgets.weather_lat = null;
              config.widgets.weather_lon = null;
              markDirty();
            },
          }),
        ]),
        row('Temperature Unit', 'Celsius or Fahrenheit display.', [
          el('select', {
            class: 'select-glass',
            onChange: (e) => { config.widgets.temperature_unit = e.target.value; markDirty(); },
          }, [
            el('option', { value: 'C', selected: config.widgets.temperature_unit === 'C' ? 'selected' : null, text: 'Celsius (°C)' }),
            el('option', { value: 'F', selected: config.widgets.temperature_unit === 'F' ? 'selected' : null, text: 'Fahrenheit (°F)' }),
          ]),
        ]),
        row('24-Hour Clock', 'Use 24-hour format on capsule and widgets.', toggleSwitch(config.widgets.clock_24h, (v) => { config.widgets.clock_24h = v; })),
      ])
    );
  } else if (activeSection === 'privacy') {
    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Privacy & Performance' }),
        row('Clipboard History Capture', 'Listen to clipboard updates for quick recall.', toggleSwitch(config.privacy.clipboard_enabled, (v) => { config.privacy.clipboard_enabled = v; })),
        row('Skip Sensitive Tokens', 'Skip strings matching API keys, GitHub tokens, and private keys.', toggleSwitch(config.privacy.clipboard_skip_sensitive, (v) => { config.privacy.clipboard_skip_sensitive = v; })),
        row('Low-Power Battery Mode', 'Double telemetry sampling intervals when running on battery.', toggleSwitch(config.performance.low_power_on_battery, (v) => { config.performance.low_power_on_battery = v; })),
        row('Telemetry Interval (ms)', 'Balance responsiveness vs CPU usage (1000–10000 ms).', [
          el('input', {
            type: 'number',
            class: 'input-glass',
            min: '1000',
            max: '10000',
            step: '250',
            value: String(config.performance.hardware_interval_ms),
            onInput: (e) => { config.performance.hardware_interval_ms = Number(e.target.value) || 2000; markDirty(); },
          }),
        ]),
      ])
    );

    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Updates (Consent Required)' }),
        row('Allow Update Checks', 'Only contact GitHub Releases API when you opt in.', toggleSwitch(config.privacy.update_check_consent, (v) => { config.privacy.update_check_consent = v; })),
        row('GitHub Release Repository', 'Format: owner/repo', [
          el('input', {
            class: 'input-glass',
            value: config.privacy.update_repo,
            onInput: (e) => { config.privacy.update_repo = e.target.value; markDirty(); },
          }),
        ]),
        row('Check for Updates', `Current version: v${appVersion}`, [
          el('button', {
            class: 'pill-btn primary',
            onClick: async () => {
              try {
                updateStatus = 'Checking GitHub Releases…';
                renderSection();
                const res = await invoke('check_updates');
                updateStatus = res.status_message;
                renderSection();
              } catch (err) {
                updateStatus = String(err);
                renderSection();
              }
            },
          }, 'Check Now'),
        ]),
        updateStatus ? el('div', { class: 'notice-banner', text: updateStatus }) : null,
      ])
    );
  } else if (activeSection === 'integrations') {
    container.appendChild(
      el('div', { class: 'setting-section' }, [
        el('h3', { text: 'Optional AI Integration (No Mandatory Paid APIs)' }),
        row('Enable AI Module', 'Shows the AI tab in the Dynamic Island.', toggleSwitch(config.ai.enabled, (v) => {
          config.ai.enabled = v;
          config.modules.ai = v;
        })),
        row('AI Provider', 'Local Ollama runs 100% offline with zero API costs.', [
          el('select', {
            class: 'select-glass',
            onChange: (e) => { config.ai.provider = e.target.value; markDirty(); },
          }, [
            el('option', { value: 'ollama', selected: config.ai.provider === 'ollama' ? 'selected' : null, text: 'Local Ollama' }),
            el('option', { value: 'openai_compatible', selected: config.ai.provider === 'openai_compatible' ? 'selected' : null, text: 'OpenAI-Compatible Local/Custom' }),
          ]),
        ]),
        row('Endpoint URL', 'Default Ollama URL: http://127.0.0.1:11434', [
          el('input', {
            class: 'input-glass',
            value: config.ai.endpoint,
            onInput: (e) => { config.ai.endpoint = e.target.value; markDirty(); },
          }),
        ]),
        row('Model Name', 'e.g. llama3.2, qwen2.5, mistral', [
          el('input', {
            class: 'input-glass',
            value: config.ai.model,
            onInput: (e) => { config.ai.model = e.target.value; markDirty(); },
          }),
        ]),
      ])
    );
  }
}

document.getElementById('btn-save-settings').addEventListener('click', async () => {
  if (!config || !invoke) return;
  try {
    config = await invoke('save_app_config', { newConfig: config });
    clearDirty();
    showToast('Settings saved and applied.');
    renderSection();
  } catch (err) {
    showToast(`Save failed: ${err}`);
  }
});

document.getElementById('btn-reset-defaults').addEventListener('click', async () => {
  if (!invoke) return;
  try {
    config = await invoke('save_app_config', { newConfig: { version: 1 } });
    clearDirty();
    showToast('Restored default PILLVYN settings.');
    renderSection();
  } catch (err) {
    showToast(`Reset failed: ${err}`);
  }
});

async function init() {
  if (!invoke) return;
  const initData = await invoke('get_initial_state');
  config = initData.config;
  monitors = initData.monitors || [];
  appVersion = initData.app_version || '1.0.0';
  document.getElementById('sidebar-version').textContent = `v${appVersion} · Settings Studio`;

  if (listen) {
    await listen('pillvyn://config', (e) => {
      if (!dirty) {
        config = e.payload;
        renderSection();
      }
    });
  }

  renderSidebar();
  renderSection();
}

init();
