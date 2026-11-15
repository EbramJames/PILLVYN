import { applyAppearanceVars } from './theme.js';

const { invoke } = window.__TAURI__?.core || {};
const { listen } = window.__TAURI__?.event || {};
const currentWindow = window.__TAURI__?.window?.getCurrentWindow?.();
const LogicalSize = window.__TAURI__?.dpi?.LogicalSize;

const TRAY_ICONS = {
  eye: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6-10-6-10-6Z"/><circle cx="12" cy="12" r="2.8"/></svg>`,
  eyeOff: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m3 3 18 18"/><path d="M10.6 10.6a2.5 2.5 0 0 0 3.3 3.3"/><path d="M9.9 5.2A10.7 10.7 0 0 1 12 5c6.5 0 10 7 10 7a16.8 16.8 0 0 1-3.3 4.3"/><path d="M6.6 6.6C4.1 8.2 2 12 2 12s3.5 7 10 7c1.8 0 3.4-.4 4.8-1"/></svg>`,
  expand: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M15 3h6v6"/><path d="M9 21H3v-6"/><path d="m21 3-7 7"/><path d="m3 21 7-7"/></svg>`,
  bellOff: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8.7 3A6 6 0 0 1 18 8a21.3 21.3 0 0 0 .6 5"/><path d="M17 17H4a4 4 0 0 0 2-3v-3a6 6 0 0 1 .3-1.9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/><path d="m2 2 20 20"/></svg>`,
  moon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/></svg>`,
  sliders: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><line x1="4" x2="20" y1="6" y2="6"/><line x1="4" x2="20" y1="12" y2="12"/><line x1="4" x2="20" y1="18" y2="18"/><circle cx="9" cy="6" r="2"/><circle cx="15" cy="12" r="2"/><circle cx="8" cy="18" r="2"/></svg>`,
  power: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v8"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/></svg>`,
};

function hydrateIcons() {
  document.querySelectorAll('.tray-icon[data-icon]').forEach((el) => {
    const key = el.dataset.icon;
    el.innerHTML = TRAY_ICONS[key] || '';
  });
}

function fitWindowHeight() {
  const root = document.getElementById('tray-root');
  if (!root || !currentWindow || !LogicalSize) return;
  const h = Math.max(260, Math.min(440, Math.ceil(root.getBoundingClientRect().height)));
  currentWindow.setSize(new LogicalSize(262, h)).catch(() => {});
}

function renderTrayState(snap) {
  if (!snap) return;

  applyAppearanceVars(
    {
      theme: snap.theme,
      accent_color: snap.accent_color,
      opacity: typeof snap.opacity === 'number' ? Math.max(0.9, snap.opacity) : 0.94,
    },
    { themeTarget: document.body, varTarget: document.documentElement, minOpacity: 0.9 }
  );

  const verEl = document.getElementById('tray-version');
  if (verEl && snap.version) {
    verEl.textContent = `v${snap.version}`;
  }

  const dotEl = document.getElementById('tray-live-dot');
  const statusLine = document.getElementById('tray-status-line');
  if (dotEl && statusLine) {
    dotEl.className = 'tray-status-dot';
    if (!snap.visible) {
      dotEl.classList.add('is-hidden');
      statusLine.textContent = 'Island Hidden';
    } else if (snap.dnd) {
      dotEl.classList.add('is-dnd');
      statusLine.textContent = 'Do Not Disturb On';
    } else if (snap.focus_mode) {
      dotEl.classList.add('is-online');
      statusLine.textContent = 'Focus Mode Active';
    } else {
      dotEl.classList.add('is-online');
      const liveText = snap.active_activities > 0
        ? `${snap.active_activities} Live ${snap.active_activities === 1 ? 'Activity' : 'Activities'}`
        : 'All systems synced';
      statusLine.textContent = liveText;
    }
  }

  const mediaBar = document.getElementById('tray-now-playing');
  const mediaText = document.getElementById('tray-media-text');
  if (mediaBar && mediaText) {
    if (snap.media_active && snap.media_title) {
      mediaBar.classList.remove('is-hidden');
      mediaText.textContent = snap.media_artist
        ? `${snap.media_title} · ${snap.media_artist}`
        : snap.media_title;
    } else {
      mediaBar.classList.add('is-hidden');
    }
  }

  const visBtn = document.querySelector('[data-action="toggle_visibility"]');
  const visLabel = document.getElementById('tray-vis-label');
  const visSwitch = document.getElementById('tray-vis-switch');
  if (visBtn && visLabel && visSwitch) {
    visBtn.setAttribute('aria-checked', String(Boolean(snap.visible)));
    visLabel.textContent = 'Island Visible';
    visSwitch.classList.toggle('is-on', Boolean(snap.visible));
    const iconHolder = visBtn.querySelector('.tray-icon');
    if (iconHolder) {
      iconHolder.innerHTML = snap.visible ? TRAY_ICONS.eye : TRAY_ICONS.eyeOff;
    }
  }

  const expandLabel = document.getElementById('tray-expand-label');
  const expandPill = document.getElementById('tray-expand-pill');
  if (expandLabel && expandPill) {
    expandLabel.textContent = snap.expanded ? 'Collapse Island' : 'Expand Island';
    expandPill.textContent = snap.expanded ? 'Expanded' : 'Compact';
    expandPill.classList.toggle('is-active', Boolean(snap.expanded));
  }

  const dndBtn = document.querySelector('[data-action="toggle_dnd"]');
  const dndSwitch = document.getElementById('tray-dnd-switch');
  if (dndBtn && dndSwitch) {
    dndBtn.setAttribute('aria-checked', String(Boolean(snap.dnd)));
    dndSwitch.classList.toggle('is-on', Boolean(snap.dnd));
  }

  const focusBtn = document.querySelector('[data-action="toggle_focus"]');
  const focusSwitch = document.getElementById('tray-focus-switch');
  if (focusBtn && focusSwitch) {
    focusBtn.setAttribute('aria-checked', String(Boolean(snap.focus_mode)));
    focusSwitch.classList.toggle('is-on', Boolean(snap.focus_mode));
  }

  requestAnimationFrame(fitWindowHeight);
}

async function refreshTrayState() {
  if (!invoke) return;
  try {
    const snap = await invoke('get_tray_state');
    renderTrayState(snap);
  } catch (err) {
    console.error('Failed to load tray state:', err);
  }
}

async function runAction(action) {
  if (!invoke) return;
  try {
    const snap = await invoke('tray_action', { action });
    if (snap) renderTrayState(snap);
  } catch (err) {
    console.error('Tray action failed:', err);
  }
}

function wireTrayEvents() {
  document.querySelectorAll('.tray-item[data-action]').forEach((btn) => {
    btn.addEventListener('click', () => runAction(btn.dataset.action));
  });

  window.addEventListener('keydown', (e) => {
    const items = [...document.querySelectorAll('.tray-item[data-action]')];
    const currentIdx = items.indexOf(document.activeElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      runAction('hide_tray');
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const next = items[(currentIdx + 1) % items.length];
      next?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const prev = items[(currentIdx - 1 + items.length) % items.length];
      prev?.focus();
    }
  });

  if (listen) {
    listen('pillvyn://tray-refresh', refreshTrayState);
    listen('pillvyn://config', refreshTrayState);
    listen('pillvyn://island-state', refreshTrayState);
    listen('pillvyn://media', refreshTrayState);
    listen('pillvyn://activities', refreshTrayState);
  }
}

hydrateIcons();
wireTrayEvents();
refreshTrayState();
