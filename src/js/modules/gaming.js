import { el, icon } from '../utils.js';

export function renderGaming(store, api) {
  const gm = store.gaming;
  if (!gm) {
    return el('div', { class: 'empty-state', text: 'Monitoring active processes and Steam game state…' });
  }

  const integ = gm.integrations || {};
  const badges = [
    { label: 'Steam', on: integ.steam_running },
    { label: 'Discord', on: integ.discord_running },
    { label: 'OBS', on: integ.obs_running },
    { label: 'Spotify', on: integ.spotify_running },
    { label: 'VLC', on: integ.vlc_running },
    { label: integ.browser_running || 'Browser', on: Boolean(integ.browser_running) },
  ];

  const clickThrough = Boolean(store.config?.gaming?.click_through_hud);

  return el('div', { class: 'stack' }, [
    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: 'Game Session' }),
        el('span', {
          class: `badge ${gm.active ? 'ok' : ''}`,
          text: gm.active ? 'ACTIVE' : 'STANDBY',
        }),
      ]),
      el('div', { style: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '8px' } }, [
        el('div', {}, [
          el('div', {
            style: { fontSize: '14px', fontWeight: '700' },
            text: gm.active ? (gm.game_title || 'Detected Game') : 'No Game Running',
          }),
          el('div', {
            class: 'muted',
            text: gm.active
              ? `${gm.process_name || 'Process'}${gm.steam_app_id ? ` · Steam #${gm.steam_app_id}` : ''}`
              : `Foreground: ${gm.process_name || 'Desktop'}`,
          }),
        ]),
        gm.active && gm.process_cpu_percent != null
          ? el('div', { style: { textAlign: 'right' } }, [
              el('div', { class: 'card-value', text: `CPU ${gm.process_cpu_percent.toFixed(0)}%` }),
              el('div', { class: 'muted', text: `${gm.process_ram_mb ?? 0} MB RAM` }),
            ])
          : null,
      ]),
      el('div', { style: { display: 'flex', gap: '6px', marginTop: '9px', flexWrap: 'wrap' } }, [
        el('span', {
          class: `badge ${gm.notifications_suppressed ? 'warn' : ''}`,
          text: gm.notifications_suppressed ? 'Notifications Suppressed' : 'Alerts Normal',
        }),
        el('button', {
          class: `pill-btn ${clickThrough ? 'primary' : ''}`,
          style: { padding: '3px 9px', fontSize: '10.5px' },
          onClick: () => api.toggleGamingClickThrough(),
        }, [icon('shield', 11), el('span', { text: clickThrough ? 'HUD Click-Through: On' : 'HUD Click-Through: Off' })]),
      ]),
    ]),

    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-label', style: { marginBottom: '6px' }, text: 'Detected Ecosystem Apps' }),
      el('div', { style: { display: 'flex', gap: '6px', flexWrap: 'wrap' } }, badges.map((b) =>
        el('span', { class: `badge ${b.on ? 'ok' : ''}`, text: `${b.label}${b.on ? ' · Live' : ''}` })
      )),
    ]),

    el('div', { class: 'notice-banner', text: gm.fps_status }),
  ]);
}
