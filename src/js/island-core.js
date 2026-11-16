import { applyAppearanceVars } from './theme.js';
import { clear, el, formatSpeed, formatTimeShort, icon } from './utils.js';

export const MODULE_TABS = [
  { id: 'overview', label: 'Hub', icon: 'home' },
  { id: 'music', label: 'Media', icon: 'music', configKey: 'music' },
  { id: 'system', label: 'Controls', icon: 'sliders', configKey: 'system' },
  {
    id: 'monitor',
    label: 'Monitor',
    icon: 'activity',
    configKeys: ['hardware', 'network', 'downloads', 'gaming'],
  },
  { id: 'notifications', label: 'Alerts', icon: 'bell', configKey: 'notifications' },
  { id: 'clipboard', label: 'Clip', icon: 'clipboard', configKey: 'clipboard' },
  {
    id: 'utilities',
    label: 'Tools',
    icon: 'grid',
    configKeys: ['widgets', 'actions', 'plugins', 'ai'],
  },
];

export function resolveTabRoute(tabId) {
  if (['hardware', 'network', 'downloads', 'gaming'].includes(tabId)) {
    return { activeTab: 'monitor', monitorSubTab: tabId };
  }
  if (['widgets', 'actions', 'plugins', 'ai'].includes(tabId)) {
    return { activeTab: 'utilities', utilitiesSubTab: tabId };
  }
  return { activeTab: tabId || 'overview' };
}

export function isTabEnabled(tab, config) {
  const mods = config?.modules || {};
  if (tab.configKey) return mods[tab.configKey] !== false;
  if (tab.configKeys) {
    return tab.configKeys.some((k) => {
      if (k === 'ai') return mods.ai !== false && Boolean(config?.ai?.enabled);
      return mods[k] !== false;
    });
  }
  return true;
}

export function applyVisualTheme(shell, config, profileOverride) {
  if (!config) return;
  const ap = config.appearance || {};
  const activeTheme = profileOverride?.theme_override || ap.theme || 'obsidian';
  applyAppearanceVars(
    { ...ap, theme: activeTheme },
    {
      themeTarget: shell,
      varTarget: document.documentElement,
      applyOpacity: true,
    }
  );
  document.documentElement.style.setProperty('--compact-width', `${ap.compact_width ?? 268}px`);
  document.body.classList.toggle('reduced-motion', Boolean(ap.reduced_motion));
}

export function renderCompactAndMinimized(store, api) {
  const use24h = store.config?.widgets?.clock_24h ?? true;
  const nowStr = formatTimeShort(new Date(), use24h);
  const dnd = Boolean(store.config?.privacy?.dnd_mode || store.activeProfile?.dnd);
  const media = store.media;
  const activities = store.activities || [];
  const topAct = activities.length > 0
    ? activities[store.activityCycleIndex % activities.length]
    : null;
  const minimalIdle = store.config?.appearance?.minimal_idle !== false;
  const hoverPeek = store.config?.appearance?.hover_peek !== false;

  // Minimized pill (clean micro status + time, no noisy uppercase IDLE)
  const miniDot = document.getElementById('mini-dot');
  const miniTime = document.getElementById('mini-time');
  const miniLabel = document.getElementById('mini-label');
  miniTime.textContent = nowStr;
  miniDot.className = 'mini-dot' + (dnd ? ' dnd' : media?.status === 'Playing' ? ' playing' : '');
  miniLabel.textContent = dnd
    ? 'DND'
    : (topAct && topAct.kind !== 'media' ? (topAct.badge || '') : '');

  // Header clock + active profile pill
  const hdrClock = document.getElementById('header-clock');
  if (hdrClock) hdrClock.textContent = nowStr;
  const hdrProfile = document.getElementById('header-profile');
  if (hdrProfile) {
    if (store.activeProfile?.name) {
      hdrProfile.textContent = store.activeProfile.name;
      hdrProfile.style.display = 'inline-block';
    } else {
      hdrProfile.style.display = 'none';
    }
  }

  // Compact capsule
  const compactView = document.getElementById('view-compact');
  const artWrap = document.getElementById('compact-artwork');
  const titleEl = document.getElementById('compact-title');
  const subEl = document.getElementById('compact-sub');
  const eqEl = document.getElementById('compact-eq');
  const chipEl = document.getElementById('compact-chip');
  const peekEl = document.getElementById('compact-peek');
  const progEl = document.getElementById('compact-progress');
  const progFillEl = document.getElementById('compact-progress-fill');

  clear(artWrap);
  if (media?.active && media.thumbnail_data_url && (!topAct || topAct.kind === 'media')) {
    artWrap.classList.add('round');
    artWrap.appendChild(el('img', { src: media.thumbnail_data_url, alt: media.title || 'Cover' }));
  } else if (topAct && topAct.kind !== 'media') {
    artWrap.classList.remove('round');
    const iconName = topAct.kind === 'download'
      ? 'download'
      : topAct.kind === 'timer'
        ? 'clock'
        : topAct.kind === 'gaming'
          ? 'gamepad'
          : topAct.kind === 'notification'
            ? 'bell'
            : topAct.kind === 'system_hud'
              ? (topAct.title === 'Muted' ? 'mute' : topAct.title === 'Brightness' ? 'sun' : 'volume')
              : 'layers';
    artWrap.appendChild(icon(iconName, 13));
  } else if (media?.active) {
    artWrap.classList.remove('round');
    artWrap.appendChild(icon('music', 13));
  } else {
    artWrap.classList.remove('round');
    artWrap.appendChild(el('img', { src: 'assets/icon.png', alt: 'PILLVYN' }));
  }

  if (topAct && topAct.kind !== 'media') {
    titleEl.textContent = topAct.title;
    subEl.textContent = topAct.subtitle || '';
  } else if (media?.active) {
    titleEl.textContent = media.title || 'Now Playing';
    subEl.textContent = media.artist || media.source_app || '';
  } else if (store.gaming?.active) {
    titleEl.textContent = store.gaming.game_title || 'Gaming Focus';
    subEl.textContent = store.gaming.notifications_suppressed ? 'Notifications Muted' : 'Game Mode';
  } else {
    titleEl.textContent = store.activeProfile?.name || 'PILLVYN';
    if (!minimalIdle) {
      const hw = store.hardware;
      const net = store.network;
      if (hw && net?.connected) {
        subEl.textContent = `CPU ${Math.round(hw.cpu_usage_percent)}% · ↓ ${formatSpeed(net.download_bps)}`;
      } else if (hw) {
        subEl.textContent = `CPU ${Math.round(hw.cpu_usage_percent)}% · RAM ${Math.round(hw.ram_percent)}%`;
      } else {
        subEl.textContent = '';
      }
    } else {
      subEl.textContent = dnd ? 'Do Not Disturb' : '';
    }
  }

  const isPlaying = media?.status === 'Playing';
  eqEl.classList.toggle('active', isPlaying);
  eqEl.style.display = isPlaying ? 'flex' : 'none';

  // Hover-to-Peek inline playback controls on Compact pill
  const canPeek = hoverPeek && Boolean(media?.active) && (!topAct || topAct.kind === 'media');
  compactView.classList.toggle('peek-ready', canPeek);
  clear(peekEl);
  if (canPeek && api) {
    const mkPeekBtn = (iconName, title, disabled, actionName) => {
      const btn = el(
        'button',
        {
          type: 'button',
          class: 'peek-btn',
          title,
          disabled: Boolean(disabled),
          onClick: (e) => {
            e.stopPropagation();
            if (!disabled) api.mediaControl(actionName);
          },
        },
        icon(iconName, 11)
      );
      btn.addEventListener('pointerdown', (e) => e.stopPropagation());
      return btn;
    };
    peekEl.appendChild(mkPeekBtn('prev', 'Previous Track', media.can_previous === false, 'previous'));
    peekEl.appendChild(
      mkPeekBtn(
        isPlaying ? 'pause' : 'play',
        isPlaying ? 'Pause' : 'Play',
        media.can_play_pause === false,
        'play_pause'
      )
    );
    peekEl.appendChild(mkPeekBtn('next', 'Next Track', media.can_next === false, 'next'));
  }

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

  // Subtle hairline bottom progress indicator on Compact pill
  let pct = null;
  if (topAct?.progress != null) {
    pct = Math.max(0, Math.min(100, topAct.progress * 100));
  } else if (media?.active && media.duration_sec > 1) {
    pct = Math.max(0, Math.min(100, (media.position_sec / media.duration_sec) * 100));
  }
  progEl.classList.toggle('visible', pct != null);
  if (pct != null) {
    progFillEl.style.width = `${pct.toFixed(1)}%`;
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
  const unread = store.notifications?.unread_count || 0;
  const dlActive = store.downloads?.active_count || 0;

  for (const tab of MODULE_TABS) {
    if (!isTabEnabled(tab, store.config)) continue;
    const isActive = store.activeTab === tab.id;
    let badgeCount = 0;
    if (tab.id === 'notifications') badgeCount = unread;
    if (tab.id === 'monitor') badgeCount = dlActive;

    const btn = el('button', {
      class: `nav-tab ${isActive ? 'active' : ''}`,
      role: 'tab',
      title: tab.label,
      'aria-label': tab.label,
      'aria-selected': isActive ? 'true' : 'false',
      onClick: () => onSelectTab(tab.id),
    }, [
      icon(tab.icon, 13),
      el('span', { class: 'nav-label', text: tab.label }),
      badgeCount > 0 ? el('span', { class: 'nav-badge', text: String(badgeCount) }) : null,
    ]);
    nav.appendChild(btn);
  }
}
