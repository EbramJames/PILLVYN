import { el, formatBytes, formatDuration, formatSpeed, icon } from '../utils.js';

export function renderOverview(store, api) {
  const root = el('div', { class: 'stack' });

  // 1. Live Now Playing or Focus Summary Card
  const media = store.media;
  if (media && media.active) {
    const thumb = el('div', { class: 'album-art', style: { width: '48px', height: '48px', borderRadius: '11px' } }, [
      media.thumbnail_data_url
        ? el('img', { src: media.thumbnail_data_url, alt: media.title || 'Album art' })
        : icon('music', 20),
    ]);
    const playIcon = media.status === 'Playing' ? icon('pause', 14) : icon('play', 14);
    const controls = el('div', { style: { display: 'flex', gap: '5px', alignItems: 'center' } }, [
      el('button', { class: 'icon-btn', title: 'Previous', onClick: () => api.mediaControl('previous') }, icon('prev', 13)),
      el('button', { class: 'icon-btn active', title: 'Play / Pause', onClick: () => api.mediaControl('play_pause') }, playIcon),
      el('button', { class: 'icon-btn', title: 'Next', onClick: () => api.mediaControl('next') }, icon('next', 13)),
    ]);

    const pct = media.duration_sec > 0 ? Math.min(100, (media.position_sec / media.duration_sec) * 100) : 0;
    root.appendChild(
      el('div', { class: 'glass-card' }, [
        el('div', { style: { display: 'flex', alignItems: 'center', gap: '10px' } }, [
          thumb,
          el('div', { class: 'track-meta' }, [
            el('div', { class: 'track-title', style: { fontSize: '13px' }, text: media.title || 'Media Playing' }),
            el('div', { class: 'track-artist', text: `${media.artist || media.source_app || 'Windows Media'} · ${formatDuration(media.position_sec)} / ${formatDuration(media.duration_sec)}` }),
          ]),
          controls,
        ]),
        el('div', { class: 'meter-track', style: { marginTop: '8px', height: '4px' } }, [
          el('div', { class: 'meter-fill', style: { width: `${pct}%` } }),
        ]),
      ])
    );
  }

  // 2. Quick System Sliders (Volume + Brightness)
  const sys = store.system;
  if (sys) {
    const volPct = Math.round((sys.output_volume || 0) * 100);
    const brightPct = sys.brightness ?? 0;

    const volBg = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${volPct}%, rgba(255, 255, 255, 0.12) ${volPct}%)`;
    const brightBg = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${brightPct}%, rgba(255, 255, 255, 0.12) ${brightPct}%)`;

    root.appendChild(
      el('div', { class: 'grid-2' }, [
        el('div', { class: 'glass-card' }, [
          el('div', { class: 'card-title-row' }, [
            el('span', { class: 'card-label', text: 'Volume' }),
            el('span', { class: 'card-value', text: sys.output_muted ? 'Muted' : `${volPct}%` }),
          ]),
          el('input', {
            type: 'range',
            class: 'range-slider',
            min: '0',
            max: '100',
            value: String(volPct),
            style: { background: volBg },
            onInput: (e) => {
              const v = Number(e.target.value);
              e.target.style.background = `linear-gradient(90deg, var(--accent) 0%, var(--accent-2) ${v}%, rgba(255, 255, 255, 0.12) ${v}%)`;
              api.setVolume(false, v / 100);
            },
          }),
        ]),
        el('div', { class: 'glass-card' }, [
          el('div', { class: 'card-title-row' }, [
            el('span', { class: 'card-label', text: 'Brightness' }),
            el('span', {
              class: 'card-value',
              text: sys.brightness_supported ? `${brightPct}%` : 'N/A',
            }),
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
            : el('div', { class: 'muted', text: 'DDC/CI or internal panel unavailable' }),
        ]),
      ])
    );
  }

  // 3. Telemetry Pulse Row (CPU/RAM, Network, Battery)
  const hw = store.hardware;
  const net = store.network;
  const batt = sys?.battery;
  root.appendChild(
    el('div', { class: 'grid-3' }, [
      el('div', { class: 'glass-card', style: { cursor: 'pointer' }, onClick: () => api.switchTab('hardware') }, [
        el('div', { class: 'card-label', text: 'CPU / RAM' }),
        el('div', {
          class: 'card-value',
          style: { marginTop: '4px' },
          text: hw ? `${Math.round(hw.cpu_usage_percent)}% · ${Math.round(hw.ram_percent)}%` : 'Sampling…',
        }),
        el('div', {
          class: 'muted',
          text: hw ? `${formatBytes(hw.ram_used_bytes)} RAM` : 'Hardware',
        }),
      ]),
      el('div', { class: 'glass-card', style: { cursor: 'pointer' }, onClick: () => api.switchTab('network') }, [
        el('div', { class: 'card-label', text: net?.vpn_active ? 'Network · VPN' : 'Network' }),
        el('div', {
          class: 'card-value',
          style: { marginTop: '4px' },
          text: net ? (net.connected ? `↓ ${formatSpeed(net.download_bps)}` : 'Offline') : 'Probing…',
        }),
        el('div', {
          class: 'muted',
          text: net?.ping_ms != null ? `Ping ${net.ping_ms} ms` : (net?.adapter_name || 'Adapter'),
        }),
      ]),
      el('div', { class: 'glass-card', style: { cursor: 'pointer' }, onClick: () => api.switchTab('system') }, [
        el('div', { class: 'card-label', text: 'Power' }),
        el('div', {
          class: 'card-value',
          style: { marginTop: '4px' },
          text: batt?.present
            ? `${batt.percent ?? '--'}%${batt.charging ? ' ⚡' : ''}`
            : (batt?.ac_online ? 'AC Power' : 'Desktop'),
        }),
        el('div', {
          class: 'muted',
          text: batt?.battery_saver ? 'Battery Saver On' : (batt?.ac_online ? 'Plugged In' : 'On Battery'),
        }),
      ]),
    ])
  );

  // 4. Top Quick Actions Row
  const actions = (store.config?.quick_actions || []).slice(0, 3);
  if (actions.length > 0) {
    root.appendChild(
      el('div', { class: 'grid-3' }, actions.map((item) =>
        el('button', { class: 'action-tile', onClick: () => api.runQuickAction(item) }, [
          icon(item.icon || 'zap', 15),
          el('span', { style: { fontSize: '11.5px', fontWeight: '600' }, text: item.label }),
        ])
      ))
    );
  }

  return root;
}
