import { batteryEmoji, respectsShowEmojis, withEmoji } from '../emoji.js';
import { clampPercent, createRangeSlider, el, formatDuration, icon } from '../utils.js';

export function renderSystem(store, api) {
  const sys = store.system;
  if (!sys) {
    return el('div', { class: 'empty-state', text: 'Querying Windows audio & system controls…' });
  }
  const showEmojis = respectsShowEmojis(store.config);

  const outPct = Math.round(clampPercent((sys.output_volume || 0) * 100));
  const micPct = Math.round(clampPercent((sys.mic_volume || 0) * 100));
  const brightPct = Math.round(clampPercent(sys.brightness ?? 0));

  const outSelect = el('select', {
    class: 'select-glass',
    onChange: (e) => api.switchAudioDevice(e.target.value),
  }, sys.output_devices.map((d) =>
    el('option', { value: d.id, selected: Boolean(d.is_default), text: d.name })
  ));

  const inSelect = el('select', {
    class: 'select-glass',
    onChange: (e) => api.switchAudioDevice(e.target.value),
  }, sys.input_devices.map((d) =>
    el('option', { value: d.id, selected: Boolean(d.is_default), text: d.name })
  ));

  const batt = sys.battery;
  const battIcon = batteryEmoji(batt.percent, batt.charging, batt.present, showEmojis);
  const battDesc = withEmoji(
    battIcon,
    !batt.present
      ? (batt.ac_online ? 'Desktop / AC Power' : 'No battery detected')
      : `${batt.percent ?? '--'}% · ${batt.charging ? 'Charging' : (batt.ac_online ? 'Plugged in' : 'Discharging')}${batt.remaining_seconds ? ` · ${formatDuration(batt.remaining_seconds)} left` : ''}`
  );

  const outReadout = el('span', { text: sys.output_muted ? 'Muted' : `${outPct}%` });
  const outSlider = createRangeSlider({
    value: outPct,
    min: 0,
    max: 100,
    ariaLabel: 'Master Output Volume',
    onDragState: (d) => api.setRangeDragging(d),
    onInputValue: (v) => {
      outReadout.textContent = sys.output_muted ? `Muted · ${Math.round(v)}%` : `${Math.round(v)}%`;
      api.setVolume(false, v / 100, false);
    },
    onCommitValue: (v) => {
      outReadout.textContent = sys.output_muted ? 'Muted' : `${Math.round(v)}%`;
      api.setVolume(false, v / 100, true);
    },
  });

  const micReadout = el('span', { text: !sys.mic_available ? 'None' : (sys.mic_muted ? 'Muted' : `${micPct}%`) });
  const micSlider = createRangeSlider({
    value: micPct,
    min: 0,
    max: 100,
    disabled: !sys.mic_available,
    ariaLabel: 'Microphone Level',
    onDragState: (d) => api.setRangeDragging(d),
    onInputValue: (v) => {
      micReadout.textContent = sys.mic_muted ? `Muted · ${Math.round(v)}%` : `${Math.round(v)}%`;
      api.setVolume(true, v / 100, false);
    },
    onCommitValue: (v) => {
      micReadout.textContent = sys.mic_muted ? 'Muted' : `${Math.round(v)}%`;
      api.setVolume(true, v / 100, true);
    },
  });

  const brightReadout = el('span', { class: 'card-value', text: sys.brightness_supported ? `${brightPct}%` : 'Unsupported' });
  const brightSlider = sys.brightness_supported
    ? createRangeSlider({
        value: brightPct,
        min: 0,
        max: 100,
        ariaLabel: 'Display Brightness',
        onDragState: (d) => api.setRangeDragging(d),
        onInputValue: (v) => {
          const rounded = Math.round(v);
          brightReadout.textContent = `${rounded}%`;
          api.setBrightness(rounded, false);
        },
        onCommitValue: (v) => {
          const rounded = Math.round(v);
          brightReadout.textContent = `${rounded}%`;
          api.setBrightness(rounded, true);
        },
      })
    : null;

  const wifiBadgeText = sys.wifi_connected
    ? (sys.wifi_ssid && sys.wifi_ssid.toLowerCase() !== 'wi-fi' ? `Wi-Fi · ${sys.wifi_ssid}` : 'Wi-Fi · Connected')
    : 'Ethernet / Offline';

  return el('div', { class: 'stack' }, [
    el('div', { class: 'grid-2' }, [
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: 'Master Output' }),
          el('button', {
            type: 'button',
            class: `pill-btn ${sys.output_muted ? 'danger' : ''}`,
            style: { padding: '2px 8px', fontSize: '10.5px' },
            onClick: () => api.setMute(false, !sys.output_muted),
          }, [icon(sys.output_muted ? 'mute' : 'volume', 12), outReadout]),
        ]),
        outSlider,
      ]),
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: 'Microphone' }),
          el('button', {
            type: 'button',
            class: `pill-btn ${sys.mic_muted ? 'danger' : ''}`,
            style: { padding: '2px 8px', fontSize: '10.5px' },
            disabled: !sys.mic_available,
            onClick: () => api.setMute(true, !sys.mic_muted),
          }, [icon('mic', 12), micReadout]),
        ]),
        micSlider,
      ]),
    ]),

    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: 'Display Brightness' }),
        brightReadout,
      ]),
      sys.brightness_supported
        ? brightSlider
        : el('div', { class: 'muted', text: 'No DDC/CI or internal backlight exposed by display' }),
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
        el('span', { class: `badge ${sys.wifi_connected ? 'ok' : ''}`, text: wifiBadgeText }),
      ]),
      el('div', { class: 'muted', text: battDesc }),
      el('div', { style: { display: 'flex', gap: '6px', marginTop: '8px', flexWrap: 'wrap' } }, [
        el('button', { type: 'button', class: 'pill-btn', onClick: () => api.openUrl('ms-availablenetworks:') }, [icon('wifi', 12), el('span', { text: 'Wi-Fi Flyout' })]),
        el('button', { type: 'button', class: 'pill-btn', onClick: () => api.openUrl('ms-settings:bluetooth') }, el('span', { text: 'Bluetooth' })),
        el('button', { type: 'button', class: 'pill-btn', onClick: () => api.openUrl('ms-settings:sound') }, el('span', { text: 'Audio Settings' })),
      ]),
    ]),
  ]);
}
