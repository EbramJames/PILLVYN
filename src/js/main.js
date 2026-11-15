import {
  MODULE_TABS,
  applyVisualTheme,
  isTabEnabled,
  renderActivitiesStrip,
  renderCompactAndMinimized,
  renderNavTabs,
  resolveTabRoute,
} from './island-core.js';
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

const MONITOR_SUBTABS = [
  { id: 'hardware', label: 'Hardware', icon: 'cpu' },
  { id: 'network', label: 'Network', icon: 'wifi' },
  { id: 'downloads', label: 'Downloads', icon: 'download' },
  { id: 'gaming', label: 'Gaming', icon: 'gamepad' },
];

const UTILITIES_SUBTABS = [
  { id: 'widgets', label: 'Timers', icon: 'clock' },
  { id: 'actions', label: 'Shortcuts', icon: 'zap' },
  { id: 'plugins', label: 'Plugins', icon: 'plug' },
  { id: 'ai', label: 'AI', icon: 'sparkles' },
];

let autoCollapseTimer = null;
let suppressNextClickUntil = 0;

function getEnabledMainTabs() {
  return MODULE_TABS.filter((tab) => isTabEnabled(tab, store.config));
}

function getEnabledSubTabs(group) {
  const mods = store.config?.modules || {};
  if (group === 'monitor') {
    return MONITOR_SUBTABS.filter((t) => mods[t.id] !== false);
  }
  return UTILITIES_SUBTABS.filter((t) =>
    t.id === 'ai' ? (mods.ai !== false && Boolean(store.config?.ai?.enabled)) : mods[t.id] !== false
  );
}

function isUserInteractingInPanel() {
  const active = document.activeElement;
  if (!active || !panelEl.contains(active)) return false;
  const tag = active.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}

function scheduleAutoCollapse() {
  clearTimeout(autoCollapseTimer);
  if (store.pinned) return;
  const ms = store.config?.appearance?.auto_collapse_ms || 0;
  if (ms > 0 && store.islandState === 'expanded') {
    autoCollapseTimer = setTimeout(() => {
      if (!store.pinned && !shell.matches(':hover') && !isUserInteractingInPanel()) {
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
  const updates = { islandState: next };
  if (next !== 'expanded' && store.pinned) {
    updates.pinned = false;
  }
  patchStore(updates, 'state-change');
  requestAnimationFrame(reportHitbox);
  setTimeout(reportHitbox, 380);
  if (next === 'expanded') {
    scheduleAutoCollapse();
  } else {
    clearTimeout(autoCollapseTimer);
  }
}

const api = {
  switchTab(tabId) {
    const route = resolveTabRoute(tabId);
    patchStore(route, 'tab-change');
    if (store.islandState !== 'expanded') setIslandState('expanded');
  },
  switchMonitorSubTab(subId) {
    patchStore({ monitorSubTab: subId }, 'sub-tab');
  },
  switchUtilitiesSubTab(subId) {
    patchStore({ utilitiesSubTab: subId }, 'sub-tab');
  },
  togglePin() {
    const nextPinned = !store.pinned;
    patchStore({ pinned: nextPinned }, 'pin');
    invoke?.('set_island_pinned', { pinned: nextPinned }).catch(console.error);
    if (nextPinned) {
      clearTimeout(autoCollapseTimer);
    } else if (!shell.matches(':hover')) {
      scheduleAutoCollapse();
    }
  },
  mediaControl(action, seekSec = null, sourceId = null) {
    invoke?.('media_control', { action, seekSec, sourceId }).catch(console.error);
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

function renderSegmentedGroup(subTabs, currentSub, onSelect, body) {
  return el('div', { class: 'stack' }, [
    subTabs.length > 1
      ? el(
          'div',
          { class: 'sub-nav', role: 'tablist' },
          subTabs.map((sub) =>
            el(
              'button',
              {
                class: `sub-tab ${currentSub === sub.id ? 'active' : ''}`,
                role: 'tab',
                'aria-selected': currentSub === sub.id ? 'true' : 'false',
                onClick: () => onSelect(sub.id),
              },
              [icon(sub.icon, 12), el('span', { text: sub.label })]
            )
          )
        )
      : null,
    body,
  ]);
}

function renderActivePanel() {
  if (store.islandState !== 'expanded') return;
  if (isUserInteractingInPanel() && store.activeTab === 'utilities' && store.utilitiesSubTab === 'ai') return;

  let content = null;
  switch (store.activeTab) {
    case 'overview':
      content = renderOverview(store, api);
      break;
    case 'music':
      content = renderMusic(store, api);
      break;
    case 'system':
      content = renderSystem(store, api);
      break;
    case 'monitor': {
      const enabled = getEnabledSubTabs('monitor');
      const current = enabled.some((t) => t.id === store.monitorSubTab)
        ? store.monitorSubTab
        : (enabled[0]?.id || 'hardware');
      let body = null;
      if (current === 'network') body = renderNetwork(store);
      else if (current === 'downloads') body = renderDownloads(store);
      else if (current === 'gaming') body = renderGaming(store, api);
      else body = renderHardware(store);
      content = renderSegmentedGroup(enabled, current, api.switchMonitorSubTab, body);
      break;
    }
    case 'notifications':
      content = renderNotifications(store, api);
      break;
    case 'clipboard':
      content = renderClipboard(store, api, renderActivePanel);
      break;
    case 'utilities': {
      const enabled = getEnabledSubTabs('utilities');
      const current = enabled.some((t) => t.id === store.utilitiesSubTab)
        ? store.utilitiesSubTab
        : (enabled[0]?.id || 'widgets');
      let body = null;
      if (current === 'actions') body = renderActions(store, api);
      else if (current === 'plugins') body = renderPlugins(store, api);
      else if (current === 'ai') body = renderAi(store, api);
      else body = renderWidgets(store, api);
      content = renderSegmentedGroup(enabled, current, api.switchUtilitiesSubTab, body);
      break;
    }
    default:
      content = renderOverview(store, api);
      break;
  }

  clear(panelEl);
  if (content) panelEl.appendChild(content);
}

function renderHeaderButtons() {
  const pinBtn = document.getElementById('btn-pin');
  const dndBtn = document.getElementById('btn-dnd');
  const focusBtn = document.getElementById('btn-focus');
  const minBtn = document.getElementById('btn-minimize');
  const colBtn = document.getElementById('btn-collapse');
  const setBtn = document.getElementById('btn-settings');

  const dnd = Boolean(store.config?.privacy?.dnd_mode);
  const focus = Boolean(store.config?.privacy?.focus_mode);

  if (pinBtn) {
    clear(pinBtn);
    pinBtn.appendChild(icon('pin', 12));
    pinBtn.className = `icon-btn ghost ${store.pinned ? 'active' : ''}`;
    pinBtn.title = store.pinned ? 'Pinned Open (Click to Unpin)' : 'Pin Open (Ignore Click Outside)';
  }

  clear(dndBtn);
  dndBtn.appendChild(icon(dnd ? 'bellOff' : 'bell', 13));
  dndBtn.className = `icon-btn ghost ${dnd ? 'danger-active' : ''}`;

  clear(focusBtn);
  focusBtn.appendChild(icon('moon', 13));
  focusBtn.className = `icon-btn ghost ${focus ? 'active' : ''}`;

  clear(minBtn);
  minBtn.appendChild(icon('minimize', 13));
  minBtn.className = 'icon-btn ghost';

  clear(colBtn);
  colBtn.appendChild(icon('chevronUp', 13));
  colBtn.className = 'icon-btn ghost';

  clear(setBtn);
  setBtn.appendChild(icon('settings', 13));
  setBtn.className = 'icon-btn ghost';
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
  const enabledTabs = getEnabledMainTabs();
  if (enabledTabs.length > 0 && !enabledTabs.some((t) => t.id === store.activeTab)) {
    store.activeTab = enabledTabs[0].id;
  }

  renderCompactAndMinimized(store, api);
  renderHeaderButtons();

  if (store.islandState === 'expanded') {
    renderActivitiesStrip(store, handleActivityClick);
    renderNavTabs(store, (tabId) => api.switchTab(tabId));
    const skipInputClobber =
      (reason === 'system' && store.activeTab === 'system' && document.activeElement?.type === 'range') ||
      (reason === 'media' && store.activeTab === 'music' && document.activeElement?.type === 'range') ||
      (['hardware', 'network', 'downloads', 'gaming', 'media', 'system', 'activities'].includes(reason) &&
        isUserInteractingInPanel());
    if (!skipInputClobber) {
      renderActivePanel();
    }
  }
  requestAnimationFrame(reportHitbox);
}

subscribe(renderAll);

// Interactions & Dynamic Island Gestures
const viewMinimized = document.getElementById('view-minimized');
const viewCompact = document.getElementById('view-compact');

viewMinimized.addEventListener('click', () => {
  if (performance.now() < suppressNextClickUntil) return;
  setIslandState('compact');
});

viewCompact.addEventListener('click', (e) => {
  if (performance.now() < suppressNextClickUntil) return;
  if (e.target.closest('button')) return;
  setIslandState('expanded');
});

viewCompact.addEventListener('contextmenu', (e) => {
  e.preventDefault();
  setIslandState('minimized');
});

viewCompact.addEventListener('mouseenter', () => {
  patchStore({ compactHovered: true }, 'compact-hover');
});
viewCompact.addEventListener('mouseleave', () => {
  patchStore({ compactHovered: false }, 'compact-hover');
});

// Compact capsule wheel gesture: adjust master output volume (Shift+Wheel for brightness)
let lastWheelActionAt = 0;
let wheelVolCache = { ts: 0, val: null };
let wheelBrightCache = { ts: 0, val: null };

viewCompact.addEventListener(
  'wheel',
  (e) => {
    if (store.islandState !== 'compact' || !store.system) return;
    e.preventDefault();
    const now = performance.now();
    if (now - lastWheelActionAt < 55) return;
    lastWheelActionAt = now;

    if (e.shiftKey && store.system.brightness_supported) {
      const base =
        now - wheelBrightCache.ts < 400 && wheelBrightCache.val != null
          ? wheelBrightCache.val
          : (store.system.brightness_percent ?? 50);
      const next = Math.max(0, Math.min(100, base + (e.deltaY < 0 ? 5 : -5)));
      wheelBrightCache = { ts: now, val: next };
      api.setBrightness(next);
    } else {
      const base =
        now - wheelVolCache.ts < 400 && wheelVolCache.val != null
          ? wheelVolCache.val
          : (store.system.output_volume ?? 0.5);
      const next = Math.max(0, Math.min(1, Number((base + (e.deltaY < 0 ? 0.04 : -0.04)).toFixed(2))));
      wheelVolCache = { ts: now, val: next };
      api.setVolume(false, next);
    }
  },
  { passive: false }
);

// Pointer swipe gestures across Minimized, Compact, and Expanded states
let dragGesture = null;

shell.addEventListener('pointerdown', (e) => {
  if (e.button !== 0) return;
  if (e.target.closest('input, textarea, select, button, .range-slider')) return;
  dragGesture = {
    x: e.clientX,
    y: e.clientY,
    ts: performance.now(),
    startedInHeaderOrNav: Boolean(e.target.closest('.expanded-header, .module-nav, .sub-nav')),
  };
});

shell.addEventListener('pointerup', (e) => {
  if (!dragGesture) return;
  const { x, y, ts, startedInHeaderOrNav } = dragGesture;
  dragGesture = null;

  const dx = e.clientX - x;
  const dy = e.clientY - y;
  const dt = performance.now() - ts;
  if (dt > 650 || Math.hypot(dx, dy) < 32) return;

  const isHorizontal = Math.abs(dx) > Math.abs(dy) * 1.25;
  const isVertical = Math.abs(dy) > Math.abs(dx) * 1.25;

  if (store.islandState === 'minimized') {
    if (dy > 24 && isVertical) {
      suppressNextClickUntil = performance.now() + 250;
      setIslandState('compact');
    }
    return;
  }

  if (store.islandState === 'compact') {
    suppressNextClickUntil = performance.now() + 260;
    if (isHorizontal) {
      const acts = store.activities || [];
      if (acts.length > 1) {
        const len = acts.length;
        const nextIdx = (store.activityCycleIndex + (dx < 0 ? 1 : -1) + len) % len;
        patchStore({ activityCycleIndex: nextIdx }, 'activity-cycle');
      } else if (store.media?.active) {
        api.mediaControl(dx < 0 ? 'next' : 'previous');
      }
    } else if (isVertical) {
      if (dy > 28) setIslandState('expanded');
      else if (dy < -28) setIslandState('minimized');
    }
    return;
  }

  if (store.islandState === 'expanded') {
    if (isHorizontal && Math.abs(dx) > 46) {
      const tabs = getEnabledMainTabs();
      if (tabs.length > 1) {
        const curIdx = Math.max(0, tabs.findIndex((t) => t.id === store.activeTab));
        const nextIdx = (curIdx + (dx < 0 ? 1 : -1) + tabs.length) % tabs.length;
        api.switchTab(tabs[nextIdx].id);
      }
    } else if (isVertical && dy < -46 && (startedInHeaderOrNav || panelEl.scrollTop <= 2)) {
      setIslandState('compact');
    }
  }
});

shell.addEventListener('pointercancel', () => {
  dragGesture = null;
});

document.getElementById('btn-pin')?.addEventListener('click', () => api.togglePin());
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

window.addEventListener('blur', () => {
  if (
    store.islandState === 'expanded' &&
    !store.pinned &&
    store.config?.appearance?.click_outside_dismiss !== false &&
    !isUserInteractingInPanel()
  ) {
    setIslandState('compact');
  }
});

window.addEventListener('keydown', (e) => {
  if (store.islandState !== 'expanded') return;

  if (e.key === 'Escape') {
    if (isUserInteractingInPanel()) {
      document.activeElement?.blur();
      return;
    }
    setIslandState('compact');
    return;
  }

  if (isUserInteractingInPanel()) return;

  if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
    const tabs = getEnabledMainTabs();
    if (tabs.length > 1) {
      e.preventDefault();
      const curIdx = Math.max(0, tabs.findIndex((t) => t.id === store.activeTab));
      const nextIdx = (curIdx + (e.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
      api.switchTab(tabs[nextIdx].id);
    }
  } else if (
    e.key === ' ' &&
    store.media?.active &&
    ['overview', 'music'].includes(store.activeTab) &&
    document.activeElement?.tagName !== 'BUTTON'
  ) {
    e.preventDefault();
    api.mediaControl('play_pause');
  }
});

const ro = new ResizeObserver(() => reportHitbox());
ro.observe(shell);

// Update clock smoothly once every 15s
setInterval(() => renderCompactAndMinimized(store, api), 15000);

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
      activeProfile: init.active_profile || null,
      pinned: Boolean(init.island_pinned),
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
    await listen('pillvyn://pinned', (e) => {
      patchStore({ pinned: Boolean(e.payload) }, 'pin');
    });
    await listen('pillvyn://dismiss-expanded', () => {
      if (store.islandState === 'expanded' && !store.pinned) {
        setIslandState('compact');
      }
    });
    await listen('pillvyn://hotkey', (e) => {
      if (e.payload === 'toggle_expand') {
        setIslandState(store.islandState === 'expanded' ? 'compact' : 'expanded');
      } else if (e.payload === 'clipboard_drawer') {
        api.switchTab('clipboard');
      }
    });
  }

  reportHitbox();
}

bootstrap();
