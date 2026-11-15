import { el, formatDuration, icon } from '../utils.js';

export function renderSystem(store, api) {
  const sys = store.system;
  if (!sys) {
    return el('div', { class: 'empty-state', text: 'Querying Windows audio & system controls…' });
  }

  const outPct = Math.round((sys.output_volume || 0) * 100);
  const micPct = Math.round((sys.mic_volume || 0) * 100);
  const brightPct = sys.brightness ?? 0;

  const outSelect = el('select', {
    class: 'select-glass',
    onChange: (e) => api.switchAudioDevice(e.target.value),
  }, sys.output_devices.map((d) =>
    el('option', { value: d.id, selected: d.is_default ? 'selected' : null, text: d.name })
  ));

  const inSelect = el('select', {
    class: 'select-glass',
    onChange: (e) => api.switchAudioDevice(e.target.value),
  }, sys.input_devices.map((d) =>
    el('option', { value: d.id, selected: d.is_default ? 'selected' : null, text: d.name })
  ));

  const batt = sys.battery;
  const battDesc = !batt.present
    ? (batt.ac_online ? 'Desktop / AC Power' : 'No battery detected')
    : `${batt.percent ?? '--'}% · ${batt.charging ? 'Charging' : (batt.ac_online ? 'Plugged in' : 'Discharging')}${batt.remaining_seconds ? ` · ${formatDuration(batt.remaining_seconds)} left` : ''}`;

  const outBg = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${outPct}%, rgba(255, 255, 255, 0.12) ${outPct}%)`;
  const micBg = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${micPct}%, rgba(255, 255, 255, 0.12) ${micPct}%)`;
  const brightBg = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${brightPct}%, rgba(255, 255, 255, 0.12) ${brightPct}%)`;

  return el('div', { class: 'stack' }, [
    el('div', { class: 'grid-2' }, [
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: 'Master Output' }),
          el('button', {
            class: `pill-btn ${sys.output_muted ? 'danger' : ''}`,
            style: { padding: '2px 8px', fontSize: '10.5px' },
            onClick: () => api.setMute(false, !sys.output_muted),
          }, [icon(sys.output_muted ? 'mute' : 'volume', 12), el('span', { text: sys.output_muted ? 'Muted' : `${outPct}%` })]),
        ]),
        el('input', {
          type: 'range',
          class: 'range-slider',
          min: '0',
          max: '100',
          value: String(outPct),
          style: { background: outBg },
          onInput: (e) => {
            const v = Number(e.target.value);
            e.target.style.background = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${v}%, rgba(255, 255, 255, 0.12) ${v}%)`;
            api.setVolume(false, v / 100);
          },
        }),
      ]),
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: 'Microphone' }),
          el('button', {
            class: `pill-btn ${sys.mic_muted ? 'danger' : ''}`,
            style: { padding: '2px 8px', fontSize: '10.5px' },
            disabled: !sys.mic_available,
            onClick: () => api.setMute(true, !sys.mic_muted),
          }, [icon('mic', 12), el('span', { text: !sys.mic_available ? 'None' : (sys.mic_muted ? 'Muted' : `${micPct}%`) })]),
        ]),
        el('input', {
          type: 'range',
          class: 'range-slider',
          min: '0',
          max: '100',
          value: String(micPct),
          disabled: !sys.mic_available,
          style: { background: micBg },
          onInput: (e) => {
            const v = Number(e.target.value);
            e.target.style.background = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${v}%, rgba(255, 255, 255, 0.12) ${v}%)`;
            api.setVolume(true, v / 100);
          },
        }),
      ]),
    ]),

    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: 'Display Brightness' }),
        el('span', { class: 'card-value', text: sys.brightness_supported ? `${brightPct}%` : 'Unsupported' }),
      ]),
      sys.brightness_supported
        ? el('input', {
            type: 'range',
            class: 'range-slider',
            min: '0',
            max: '100',
            value: String(brightPct),
            style: { background: brightBg },
            onInput: (e) => {
              const v = Number(e.target.value);
              e.target.style.background = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${v}%, rgba(255, 255, 255, 0.12) ${v}%)`;
            },
            onChange: (e) => api.setBrightness(Number(e.target.value)),
          })
        : el('div', { class: 'muted', text: 'Monitor does not expose WMI internal brightness or DDC/CI.' }),
    ]),

    el('div', { class: 'grid-2' }, [
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-label', style: { marginBottom: '6px' }, text: 'Output Device' }),
        sys.output_devices.length > 0 ? outSelect : el('div', { class: 'muted', text: 'No active output devices' }),
      ]),
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-label', style: { marginBottom: '6px' }, text: 'Input Device' }),
        sys.input_devices.length > 0 ? inSelect : el('div', { class: 'muted', text: 'No active input devices' }),
      ]),
    ]),

    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row', style: { marginBottom: '4px' } }, [
        el('span', { class: 'card-label', text: 'Power & Connectivity' }),
        el('span', { class: `badge ${sys.wifi_connected ? 'ok' : ''}`, text: sys.wifi_connected ? `Wi-Fi: ${sys.wifi_ssid || 'Connected'}` : 'Ethernet / Offline' }),
      ]),
      el('div', { class: 'muted', text: battDesc }),
      el('div', { style: { display: 'flex', gap: '6px', marginTop: '8px', flexWrap: 'wrap' } }, [
        el('button', { class: 'pill-btn', onClick: () => api.openUrl('ms-availablenetworks:') }, [icon('wifi', 12), el('span', { text: 'Wi-Fi Flyout' })]),
        el('button', { class: 'pill-btn', onClick: () => api.openUrl('ms-settings:bluetooth') }, el('span', { text: 'Bluetooth' })),
        el('button', { class: 'pill-btn', onClick: () => api.openUrl('ms-settings:sound') }, el('span', { text: 'Audio Settings' })),
      ]),
    ]),
  ]);
}
