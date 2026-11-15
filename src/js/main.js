import { applyVisualTheme, renderActivitiesStrip, renderCompactAndMinimized, renderNavTabs } from './island-core.js';
import { renderActions } from './modules/actions.js';
import { renderClipboard } from './modules/clipboard.js';
import { renderDownloads } from './modules/downloads.js';
import { renderGaming } from './modules/gaming.js';
import { renderHardware } from './modules/hardware.js';
import { renderMusic } from './modules/music.js';
import { renderNetwork } from './modules/network.js';
import { renderNotifications } from './modules/notifications.js';
import { renderOverview } from './modules/overview.js';
import { renderAi, renderPlugins } from './modules/plugins.js';
import { renderSystem } from './modules/system.js';
import { renderWidgets } from './modules/widgets.js';
import { patchStore, store, subscribe } from './state.js';
import { clear, el, icon } from './utils.js';

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const listen = tauri?.event?.listen;

const shell = document.getElementById('island-shell');
const panelEl = document.getElementById('module-panel');

let autoCollapseTimer = null;

function isUserInteractingInPanel() {
  const active = document.activeElement;
  if (!active || !panelEl.contains(active)) return false;
  const tag = active.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}

function scheduleAutoCollapse() {
  clearTimeout(autoCollapseTimer);
  const ms = store.config?.appearance?.auto_collapse_ms || 0;
  if (ms > 0 && store.islandState === 'expanded') {
    autoCollapseTimer = setTimeout(() => {
      if (!shell.matches(':hover') && !isUserInteractingInPanel()) {
        setIslandState('compact');
      }
    }, ms);
  }
}

function reportHitbox() {
  if (!invoke) return;
  const rect = shell.getBoundingClientRect();
  invoke('update_island_hitbox', {
    hitbox: {
      x: Math.max(0, rect.left),
      y: Math.max(0, rect.top),
      width: Math.max(80, rect.width),
      height: Math.max(24, rect.height),
    },
    expanded: store.islandState === 'expanded',
    activeTab: store.activeTab,
  }).catch(() => {});
}

function setIslandState(next) {
  if (store.islandState === next) return;
  shell.setAttribute('data-state', next);
  patchStore({ islandState: next }, 'state-change');
  requestAnimationFrame(reportHitbox);
  setTimeout(reportHitbox, 400);
  if (next === 'expanded') {
    scheduleAutoCollapse();
  } else {
    clearTimeout(autoCollapseTimer);
  }
}

const api = {
  switchTab(tabId) {
    patchStore({ activeTab: tabId }, 'tab-change');
    if (store.islandState !== 'expanded') setIslandState('expanded');
  },
  mediaControl(action, seekSec = null) {
    invoke?.('media_control', { action, seekSec }).catch(console.error);
  },
  setVolume(isInput, level) {
    invoke?.('set_volume', { isInput, level }).catch(console.error);
  },
  setMute(isInput, muted) {
    invoke?.('set_mute', { isInput, muted }).catch(console.error);
  },
  switchAudioDevice(deviceId) {
    invoke?.('switch_audio_device', { deviceId }).catch(console.error);
  },
  setBrightness(percent) {
    invoke?.('set_display_brightness', { percent }).catch(console.error);
  },
  pushNotification(appName, title, body, priority = 'normal') {
    invoke?.('push_notification', { appName, title, body, priority }).catch(console.error);
  },
  notificationAction(action, id = null) {
    invoke?.('notification_action', { action, id }).then((snap) => {
      if (snap) patchStore({ notifications: snap }, 'notifications');
    }).catch(console.error);
  },
  clipboardAction(action, id = null) {
    invoke?.('clipboard_action', { action, id }).then((snap) => {
      if (snap) patchStore({ clipboard: snap }, 'clipboard');
    }).catch(console.error);
  },
  runQuickAction(item) {
    invoke?.('run_quick_action', { item }).catch(console.error);
  },
  addQuickAction(item) {
    if (!store.config) return;
    const nextCfg = structuredClone(store.config);
    nextCfg.quick_actions = [...(nextCfg.quick_actions || []), item];
    invoke?.('save_app_config', { newConfig: nextCfg }).then((saved) => {
      if (saved) patchStore({ config: saved }, 'config');
    }).catch(console.error);
  },
  openUrl(url) {
    invoke?.('open_url', { url }).catch(console.error);
  },
  timerAction(action, id = null, label = null, seconds = null) {
    invoke?.('timer_action', { action, id, label, seconds }).catch(console.error);
  },
  refreshWeather() {
    invoke?.('refresh_weather').then((weather) => {
      if (weather) patchStore({ weather }, 'weather');
    }).catch(console.error);
  },
  pluginAction(action, pluginId = null, target = null) {
    invoke?.('plugin_action', { action, pluginId, target }).then((plugins) => {
      if (plugins) patchStore({ plugins }, 'plugins');
    }).catch(console.error);
  },
  toggleGamingClickThrough() {
    if (!store.config) return;
    const nextCfg = structuredClone(store.config);
    nextCfg.gaming.click_through_hud = !nextCfg.gaming.click_through_hud;
    invoke?.('save_app_config', { newConfig: nextCfg }).then((saved) => {
      if (saved) patchStore({ config: saved }, 'config');
    }).catch(console.error);
  },
  askAi(prompt) {
    if (!prompt?.trim()) return;
    patchStore({ aiLoading: true, aiError: null }, 'ai');
    invoke?.('ask_ai_assistant', { prompt, apiKey: null })
      .then((res) => patchStore({ aiResponse: res, aiLoading: false }, 'ai'))
      .catch((err) => patchStore({ aiError: String(err), aiLoading: false }, 'ai'));
  },
  openSettings() {
    invoke?.('open_settings_window').catch(console.error);
  },
};

function renderActivePanel() {
  if (store.islandState !== 'expanded') return;
  if (isUserInteractingInPanel() && store.activeTab === 'ai') return;

  let content = null;
  switch (store.activeTab) {
    case 'overview': content = renderOverview(store, api); break;
    case 'music': content = renderMusic(store, api); break;
    case 'system': content = renderSystem(store, api); break;
    case 'hardware': content = renderHardware(store); break;
    case 'gaming': content = renderGaming(store, api); break;
    case 'downloads': content = renderDownloads(store); break;
    case 'clipboard': content = renderClipboard(store, api, renderActivePanel); break;
    case 'network': content = renderNetwork(store); break;
    case 'notifications': content = renderNotifications(store, api); break;
    case 'actions': content = renderActions(store, api); break;
    case 'widgets': content = renderWidgets(store, api); break;
    case 'plugins': content = renderPlugins(store, api); break;
    case 'ai': content = renderAi(store, api); break;
    default: content = renderOverview(store, api); break;
  }

  clear(panelEl);
  if (content) panelEl.appendChild(content);
}

function renderHeaderButtons() {
  const dndBtn = document.getElementById('btn-dnd');
  const focusBtn = document.getElementById('btn-focus');
  const minBtn = document.getElementById('btn-minimize');
  const colBtn = document.getElementById('btn-collapse');
  const setBtn = document.getElementById('btn-settings');

  const dnd = Boolean(store.config?.privacy?.dnd_mode);
  const focus = Boolean(store.config?.privacy?.focus_mode);

  clear(dndBtn);
  dndBtn.appendChild(icon(dnd ? 'bellOff' : 'bell', 13));
  dndBtn.className = `icon-btn ${dnd ? 'danger-active' : ''}`;

  clear(focusBtn);
  focusBtn.appendChild(icon('moon', 13));
  focusBtn.className = `icon-btn ${focus ? 'active' : ''}`;

  clear(minBtn);
  minBtn.appendChild(icon('minimize', 13));

  clear(colBtn);
  colBtn.appendChild(icon('chevronUp', 13));

  clear(setBtn);
  setBtn.appendChild(icon('settings', 13));
}

function handleActivityClick(act) {
  const map = {
    media: 'music',
    timer: 'widgets',
    download: 'downloads',
    gaming: 'gaming',
    notification: 'notifications',
    system_hud: 'system',
    plugin: 'plugins',
  };
  api.switchTab(map[act.kind] || 'overview');
}

function renderAll(_state, reason) {
  applyVisualTheme(shell, store.config, store.activeProfile);
  renderCompactAndMinimized(store);
  renderHeaderButtons();
  if (store.islandState === 'expanded') {
    renderActivitiesStrip(store, handleActivityClick);
    renderNavTabs(store, (tabId) => patchStore({ activeTab: tabId }, 'tab'));
    // Avoid disrupting active range/text inputs on unrelated background telemetry ticks
    const skipInputClobber =
      (reason === 'system' && store.activeTab === 'system' && document.activeElement?.type === 'range') ||
      (reason === 'media' && store.activeTab === 'music' && document.activeElement?.type === 'range') ||
      (['hardware', 'network', 'media', 'system'].includes(reason) && isUserInteractingInPanel());
    if (!skipInputClobber) {
      renderActivePanel();
    }
  }
  requestAnimationFrame(reportHitbox);
}

subscribe(renderAll);

// Interactions
document.getElementById('view-minimized').addEventListener('click', () => setIslandState('compact'));
document.getElementById('view-compact').addEventListener('click', () => setIslandState('expanded'));
document.getElementById('view-compact').addEventListener('contextmenu', (e) => {
  e.preventDefault();
  setIslandState('minimized');
});

document.getElementById('btn-collapse').addEventListener('click', () => setIslandState('compact'));
document.getElementById('btn-minimize').addEventListener('click', () => setIslandState('minimized'));
document.getElementById('btn-settings').addEventListener('click', () => api.openSettings());

document.getElementById('btn-dnd').addEventListener('click', () => {
  if (!store.config) return;
  const nextCfg = structuredClone(store.config);
  nextCfg.privacy.dnd_mode = !nextCfg.privacy.dnd_mode;
  invoke?.('save_app_config', { newConfig: nextCfg }).then((saved) => {
    if (saved) patchStore({ config: saved }, 'config');
  });
});

document.getElementById('btn-focus').addEventListener('click', () => {
  if (!store.config) return;
  const nextCfg = structuredClone(store.config);
  nextCfg.privacy.focus_mode = !nextCfg.privacy.focus_mode;
  invoke?.('save_app_config', { newConfig: nextCfg }).then((saved) => {
    if (saved) patchStore({ config: saved }, 'config');
  });
});

shell.addEventListener('mouseenter', () => {
  if (store.config?.appearance?.hover_expand && store.islandState === 'compact') {
    setIslandState('expanded');
  }
});

shell.addEventListener('mouseleave', () => {
  scheduleAutoCollapse();
});

window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && store.islandState === 'expanded') {
    setIslandState('compact');
  }
});

const ro = new ResizeObserver(() => reportHitbox());
ro.observe(shell);

// Update clock smoothly once every 15s
setInterval(() => renderCompactAndMinimized(store), 15000);

async function bootstrap() {
  if (!invoke) return;

  try {
    const init = await invoke('get_initial_state');
    patchStore({
      config: init.config,
      monitors: init.monitors,
      activities: init.activities,
      media: init.media,
      system: init.system,
      hardware: init.hardware,
      gaming: init.gaming,
      downloads: init.downloads,
      notifications: init.notifications,
      clipboard: init.clipboard,
      network: init.network,
      timers: init.timers,
      stopwatchMs: init.stopwatch_ms,
      stopwatchRunning: init.stopwatch_running,
      weather: init.weather,
      plugins: init.plugins,
      appVersion: init.app_version,
    }, 'init');
  } catch (err) {
    console.error('Failed to load PILLVYN initial state:', err);
  }

  if (listen) {
    await listen('pillvyn://config', (e) => patchStore({ config: e.payload }, 'config'));
    await listen('pillvyn://activities', (e) => patchStore({ activities: e.payload }, 'activities'));
    await listen('pillvyn://media', (e) => patchStore({ media: e.payload }, 'media'));
    await listen('pillvyn://system', (e) => patchStore({ system: e.payload }, 'system'));
    await listen('pillvyn://hardware', (e) => patchStore({ hardware: e.payload }, 'hardware'));
    await listen('pillvyn://gaming', (e) => patchStore({ gaming: e.payload }, 'gaming'));
    await listen('pillvyn://downloads', (e) => patchStore({ downloads: e.payload }, 'downloads'));
    await listen('pillvyn://notifications', (e) => patchStore({ notifications: e.payload }, 'notifications'));
    await listen('pillvyn://clipboard', (e) => patchStore({ clipboard: e.payload }, 'clipboard'));
    await listen('pillvyn://network', (e) => patchStore({ network: e.payload }, 'network'));
    await listen('pillvyn://weather', (e) => patchStore({ weather: e.payload }, 'weather'));
    await listen('pillvyn://plugins', (e) => patchStore({ plugins: e.payload }, 'plugins'));
    await listen('pillvyn://widgets-timers', (e) => {
      patchStore({
        timers: e.payload.timers,
        stopwatchMs: e.payload.stopwatch_ms,
        stopwatchRunning: e.payload.stopwatch_running,
      }, 'timers');
    });
    await listen('pillvyn://profile-applied', (e) => {
      patchStore({ activeProfile: e.payload || null }, 'profile');
    });
    await listen('pillvyn://hotkey', (e) => {
      if (e.payload === 'toggle_expand') {
        setIslandState(store.islandState === 'expanded' ? 'compact' : 'expanded');
      } else if (e.payload === 'clipboard_drawer') {
        patchStore({ activeTab: 'clipboard' }, 'hotkey');
        setIslandState('expanded');
      }
    });
  }

  reportHitbox();
}

bootstrap();
