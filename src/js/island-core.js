import { clear, el, formatSpeed, formatTimeShort, icon } from './utils.js';

export const MODULE_TABS = [
  { id: 'overview', label: 'Home', icon: 'grid', configKey: null },
  { id: 'music', label: 'Music', icon: 'music', configKey: 'music' },
  { id: 'system', label: 'System', icon: 'sliders', configKey: 'system' },
  { id: 'hardware', label: 'Stats', icon: 'cpu', configKey: 'hardware' },
  { id: 'gaming', label: 'Game', icon: 'gamepad', configKey: 'gaming' },
  { id: 'downloads', label: 'DL', icon: 'download', configKey: 'downloads' },
  { id: 'clipboard', label: 'Clip', icon: 'clipboard', configKey: 'clipboard' },
  { id: 'network', label: 'Net', icon: 'wifi', configKey: 'network' },
  { id: 'notifications', label: 'Alerts', icon: 'bell', configKey: 'notifications' },
  { id: 'actions', label: 'Actions', icon: 'zap', configKey: 'actions' },
  { id: 'widgets', label: 'Widgets', icon: 'clock', configKey: 'widgets' },
  { id: 'plugins', label: 'Plugins', icon: 'plug', configKey: 'plugins' },
  { id: 'ai', label: 'AI', icon: 'sparkles', configKey: 'ai' },
];

export function applyVisualTheme(shell, config, profileOverride) {
  if (!config) return;
  const ap = config.appearance || {};
  const activeTheme = profileOverride?.theme_override || ap.theme || 'obsidian';
  shell.setAttribute('data-theme', activeTheme);
  document.documentElement.style.setProperty('--accent', ap.accent_color || '#5b8cff');
  document.documentElement.style.setProperty('--island-opacity', String(ap.opacity ?? 0.88));
  document.documentElement.style.setProperty('--island-scale', String(ap.scale ?? 1));
  document.documentElement.style.setProperty('--compact-width', `${ap.compact_width ?? 268}px`);
  document.body.classList.toggle('reduced-motion', Boolean(ap.reduced_motion));
}

export function renderCompactAndMinimized(store) {
  const use24h = store.config?.widgets?.clock_24h ?? true;
  const nowStr = formatTimeShort(new Date(), use24h);
  const dnd = Boolean(store.config?.privacy?.dnd_mode || store.activeProfile?.dnd);
  const media = store.media;
  const activities = store.activities || [];
  const topAct = activities[0] || null;

  // Minimized pill
  const miniDot = document.getElementById('mini-dot');
  const miniTime = document.getElementById('mini-time');
  const miniLabel = document.getElementById('mini-label');
  miniTime.textContent = nowStr;
  miniDot.className = 'mini-dot' + (dnd ? ' dnd' : media?.status === 'Playing' ? ' playing' : '');
  miniLabel.textContent = dnd
    ? 'DND'
    : media?.status === 'Playing'
      ? 'PLAY'
      : topAct
        ? (topAct.badge || 'LIVE')
        : 'READY';

  // Header clock
  const hdrClock = document.getElementById('header-clock');
  if (hdrClock) hdrClock.textContent = nowStr;

  // Compact capsule
  const artWrap = document.getElementById('compact-artwork');
  const titleEl = document.getElementById('compact-title');
  const subEl = document.getElementById('compact-sub');
  const eqEl = document.getElementById('compact-eq');
  const chipEl = document.getElementById('compact-chip');

  clear(artWrap);
  if (media?.active && media.thumbnail_data_url) {
    artWrap.classList.toggle('spinning', media.status === 'Playing');
    artWrap.appendChild(el('img', { src: media.thumbnail_data_url, alt: media.title || 'Cover' }));
  } else {
    artWrap.classList.remove('spinning');
    const iconName = topAct?.kind === 'download'
      ? 'download'
      : topAct?.kind === 'timer'
        ? 'clock'
        : topAct?.kind === 'gaming'
          ? 'gamepad'
          : topAct?.kind === 'notification'
            ? 'bell'
            : media?.active
              ? 'music'
              : 'sparkles';
    artWrap.appendChild(icon(iconName, 14));
  }

  if (topAct && topAct.kind !== 'media') {
    titleEl.textContent = topAct.title;
    subEl.textContent = topAct.subtitle || 'Live Activity';
  } else if (media?.active) {
    titleEl.textContent = media.title || 'Now Playing';
    subEl.textContent = media.artist || media.source_app || 'Windows Media';
  } else if (store.gaming?.active) {
    titleEl.textContent = store.gaming.game_title || 'Gaming Focus';
    subEl.textContent = store.gaming.notifications_suppressed ? 'Notifications Muted' : 'Game Mode';
  } else {
    titleEl.textContent = 'PILLVYN';
    const hw = store.hardware;
    const net = store.network;
    if (hw && net?.connected) {
      subEl.textContent = `CPU ${Math.round(hw.cpu_usage_percent)}% · ↓ ${formatSpeed(net.download_bps)}`;
    } else if (hw) {
      subEl.textContent = `CPU ${Math.round(hw.cpu_usage_percent)}% · RAM ${Math.round(hw.ram_percent)}%`;
    } else {
      subEl.textContent = 'Windows Dynamic Hub';
    }
  }

  const isPlaying = media?.status === 'Playing';
  eqEl.classList.toggle('active', isPlaying);
  eqEl.style.display = isPlaying ? 'flex' : 'none';

  if (topAct?.badge && topAct.kind !== 'media') {
    chipEl.textContent = topAct.badge;
    chipEl.className = 'compact-chip accent';
  } else if (dnd) {
    chipEl.textContent = 'DND';
    chipEl.className = 'compact-chip accent';
  } else {
    chipEl.textContent = nowStr;
    chipEl.className = 'compact-chip';
  }
}

export function renderActivitiesStrip(store, onPickActivity) {
  const strip = document.getElementById('activities-strip');
  clear(strip);
  const items = store.activities || [];
  if (items.length === 0) return;

  for (const act of items) {
    strip.appendChild(
      el('button', {
        class: 'activity-pill',
        title: `${act.title}${act.subtitle ? ` — ${act.subtitle}` : ''}`,
        onClick: () => onPickActivity(act),
      }, [
        el('span', { class: 'activity-badge', text: act.badge || act.kind.toUpperCase() }),
        el('span', { class: 'activity-text', text: act.title }),
      ])
    );
  }
}

export function renderNavTabs(store, onSelectTab) {
  const nav = document.getElementById('module-nav');
  clear(nav);
  const modCfg = store.config?.modules || {};
  const unread = store.notifications?.unread_count || 0;
  const dlActive = store.downloads?.active_count || 0;

  for (const tab of MODULE_TABS) {
    if (tab.configKey && modCfg[tab.configKey] === false) continue;
    const isActive = store.activeTab === tab.id;
    let badgeCount = 0;
    if (tab.id === 'notifications') badgeCount = unread;
    if (tab.id === 'downloads') badgeCount = dlActive;

    const btn = el('button', {
      class: `nav-tab ${isActive ? 'active' : ''}`,
      role: 'tab',
      'aria-selected': isActive ? 'true' : 'false',
      onClick: () => onSelectTab(tab.id),
    }, [
      icon(tab.icon, 13),
      el('span', { text: tab.label }),
      badgeCount > 0 ? el('span', { class: 'nav-badge', text: String(badgeCount) }) : null,
    ]);
    nav.appendChild(btn);
  }
}
