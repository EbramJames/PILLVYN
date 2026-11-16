import { el, formatDuration, formatTimeShort, icon } from '../utils.js';

export function renderWidgets(store, api) {
  const use24h = store.config?.widgets?.clock_24h ?? true;
  const now = new Date();
  const dateStr = now.toLocaleDateString([], { weekday: 'short', month: 'short', day: 'numeric' });
  const weather = store.weather;
  const unit = store.config?.widgets?.temperature_unit || 'C';

  let tempDisplay = '--';
  if (weather?.temperature_c != null) {
    const val = unit === 'F' ? weather.temperature_c * 1.8 + 32 : weather.temperature_c;
    tempDisplay = `${Math.round(val)}°${unit}`;
  }

  const labelInput = el('input', { class: 'input-glass', placeholder: 'Timer label', value: 'Focus', style: { flex: '1' } });
  const minInput = el('input', { class: 'input-glass', type: 'number', min: '1', max: '180', value: '10', style: { width: '64px' } });

  const startCustomTimer = () => {
    const mins = Math.max(1, Math.min(240, Number(minInput.value) || 5));
    api.timerAction('add', null, labelInput.value.trim() || 'Timer', mins * 60);
  };
  labelInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); startCustomTimer(); }
  });
  minInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); startCustomTimer(); }
  });

  const swSec = Math.floor((store.stopwatchMs || 0) / 1000);

  const root = el('div', { class: 'stack' }, [
    el('div', { class: 'grid-2' }, [
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-label', text: 'Clock & Date' }),
        el('div', { class: 'card-value', style: { fontSize: '20px', marginTop: '3px' }, text: formatTimeShort(now, use24h) }),
        el('div', { class: 'muted', text: dateStr }),
      ]),
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row', style: { marginBottom: '2px' } }, [
          el('span', { class: 'card-label', text: weather?.city || 'Weather' }),
          el('button', {
            type: 'button',
            class: 'pill-btn',
            style: { padding: '2px 8px', fontSize: '10px' },
            disabled: Boolean(store.weatherLoading),
            onClick: () => api.refreshWeather(),
          }, store.weatherLoading ? 'Syncing…' : 'Sync'),
        ]),
        el('div', { class: 'card-value', style: { fontSize: '18px' }, text: `${tempDisplay} · ${weather?.condition || 'Offline'}` }),
        el('div', {
          class: 'muted',
          style: { fontSize: '10.5px' },
          text: weather?.available
            ? `Wind ${weather.wind_kmh ?? '--'} km/h · Humidity ${weather.humidity_percent ?? '--'}%${weather.cached ? ' (cached)' : ''}`
            : (weather?.status_note || 'Offline cache inactive'),
        }),
      ]),
    ]),

    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: 'Stopwatch' }),
        el('span', { class: 'card-value', style: { fontFamily: 'var(--font-mono)' }, text: formatDuration(swSec) }),
      ]),
      el('div', { style: { display: 'flex', gap: '6px' } }, [
        el('button', {
          type: 'button',
          class: `pill-btn ${store.stopwatchRunning ? '' : 'primary'}`,
          onClick: () => api.timerAction(store.stopwatchRunning ? 'stopwatch_pause' : 'stopwatch_start'),
        }, store.stopwatchRunning ? 'Pause' : 'Start'),
        el('button', {
          type: 'button',
          class: 'pill-btn',
          disabled: swSec === 0 && !store.stopwatchRunning,
          onClick: () => api.timerAction('stopwatch_reset'),
        }, 'Reset'),
      ]),
    ]),

    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: 'Timers' }),
        el('div', { style: { display: 'flex', gap: '4px' } }, [1, 5, 15, 25].map((m) =>
          el('button', {
            class: 'pill-btn',
            style: { padding: '2px 7px', fontSize: '10.5px' },
            onClick: () => api.timerAction('add', null, `${m}m Timer`, m * 60),
          }, `${m}m`)
        )),
      ]),
      el('div', { style: { display: 'flex', gap: '6px', marginBottom: store.timers.length ? '8px' : '0' } }, [
        labelInput,
        minInput,
        el('button', {
          type: 'button',
          class: 'pill-btn primary',
          onClick: startCustomTimer,
        }, 'Start'),
      ]),
      store.timers.length > 0
        ? el('div', { class: 'stack', style: { gap: '5px' } }, store.timers.map((t) =>
            el('div', { class: 'list-item', style: { padding: '6px 9px' } }, [
              el('span', { style: { fontWeight: '600', fontSize: '12px' }, text: t.label }),
              el('div', { style: { display: 'flex', alignItems: 'center', gap: '6px' } }, [
                el('span', { class: 'card-value', text: t.completed ? 'Done!' : formatDuration(t.remaining_seconds) }),
                el('button', {
                  class: 'pill-btn',
                  style: { padding: '2px 8px', fontSize: '10px' },
                  onClick: () => api.timerAction('toggle', t.id),
                }, t.completed ? 'Restart' : (t.running ? 'Pause' : 'Resume')),
                el('button', {
                  class: 'icon-btn',
                  style: { width: '22px', height: '22px' },
                  onClick: () => api.timerAction('remove', t.id),
                }, icon('trash', 11)),
              ]),
            ])
          ))
        : null,
    ]),
  ]);

  return root;
}
