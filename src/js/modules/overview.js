import { clampPercent, createRangeSlider, el, formatBytes, formatDuration, formatSpeed, icon } from '../utils.js';

export function renderOverview(store, api) {
  const root = el('div', { class: 'stack' });

  // 1. Live Now Playing or Focus Summary Card
  const media = store.media;
  const activeTimer = (store.timers || []).find((t) => t.running && !t.completed);
  if (media && media.active) {
    const thumb = el('div', { class: 'album-art', style: { width: '46px', height: '46px', borderRadius: '11px' } }, [
      media.thumbnail_data_url
        ? el('img', { src: media.thumbnail_data_url, alt: media.title || 'Album art' })
        : icon('music', 18),
    ]);
    const playIcon = media.status === 'Playing' ? icon('pause', 14) : icon('play', 14);
    const controls = el('div', { style: { display: 'flex', gap: '4px', alignItems: 'center' } }, [
      el('button', { type: 'button', class: 'icon-btn ghost', title: 'Previous', disabled: media.can_previous === false, onClick: () => api.mediaControl('previous') }, icon('prev', 13)),
      el('button', { type: 'button', class: 'icon-btn active', title: 'Play / Pause', disabled: media.can_play_pause === false, onClick: () => api.mediaControl('play_pause') }, playIcon),
      el('button', { type: 'button', class: 'icon-btn ghost', title: 'Next', disabled: media.can_next === false, onClick: () => api.mediaControl('next') }, icon('next', 13)),
    ]);

    const pct = media.duration_sec > 0 ? clampPercent((media.position_sec / media.duration_sec) * 100) : 0;
    root.appendChild(
      el('div', { class: 'glass-card' }, [
        el('div', { style: { display: 'flex', alignItems: 'center', gap: '10px' } }, [
          thumb,
          el('div', { class: 'track-meta' }, [
            el('div', { class: 'track-title', style: { fontSize: '13px' }, text: media.title || 'Media Playing' }),
            el('div', { class: 'track-artist', text: `${media.artist || media.source_app || 'Media'} · ${formatDuration(media.position_sec)} / ${formatDuration(media.duration_sec)}` }),
          ]),
          controls,
        ]),
        el('div', { class: 'meter-track', style: { marginTop: '8px', height: '3px' } }, [
          el('div', { class: 'meter-fill', style: { width: `${pct}%` } }),
        ]),
      ])
    );
  } else if (activeTimer) {
    const pct = activeTimer.total_seconds > 0
      ? clampPercent((1 - activeTimer.remaining_seconds / activeTimer.total_seconds) * 100)
      : 0;
    root.appendChild(
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row', style: { marginBottom: '4px' } }, [
          el('span', { class: 'card-label', text: activeTimer.label }),
          el('div', { style: { display: 'flex', alignItems: 'center', gap: '8px' } }, [
            el('span', { class: 'card-value', text: formatDuration(activeTimer.remaining_seconds) }),
            el('button', { type: 'button', class: 'pill-btn', style: { padding: '2px 8px', fontSize: '10px' }, onClick: () => api.timerAction('toggle', activeTimer.id) }, 'Pause'),
          ]),
        ]),
        el('div', { class: 'meter-track', style: { height: '3px' } }, [
          el('div', { class: 'meter-fill', style: { width: `${pct}%` } }),
        ]),
      ])
    );
  }

  // 2. Quick Audio & Brightness Controls (hide brightness card when display lacks DDC/CI or WMI)
  const sys = store.system;
  if (sys) {
    const volPct = Math.round(clampPercent((sys.output_volume || 0) * 100));
    const brightPct = Math.round(clampPercent(sys.brightness ?? 0));

    const volReadout = el('span', { text: sys.output_muted ? 'Muted' : `${volPct}%` });
    const volSlider = createRangeSlider({
      value: volPct,
      min: 0,
      max: 100,
      ariaLabel: 'Master Volume',
      onDragState: (d) => api.setRangeDragging(d),
      onInputValue: (v) => {
        volReadout.textContent = sys.output_muted ? `Muted · ${Math.round(v)}%` : `${Math.round(v)}%`;
        api.setVolume(false, v / 100, false);
      },
      onCommitValue: (v) => {
        volReadout.textContent = sys.output_muted ? 'Muted' : `${Math.round(v)}%`;
        api.setVolume(false, v / 100, true);
      },
    });

    const volCard = el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: 'Volume' }),
        el('button', {
          type: 'button',
          class: `pill-btn ${sys.output_muted ? 'danger' : ''}`,
          style: { padding: '1px 7px', fontSize: '10.5px' },
          onClick: () => api.setMute(false, !sys.output_muted),
        }, [icon(sys.output_muted ? 'mute' : 'volume', 11), volReadout]),
      ]),
      volSlider,
    ]);

    if (sys.brightness_supported) {
      const brightReadout = el('span', { class: 'card-value', text: `${brightPct}%` });
      const brightSlider = createRangeSlider({
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
      });
      const brightCard = el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: 'Brightness' }),
          brightReadout,
        ]),
        brightSlider,
      ]);
      root.appendChild(el('div', { class: 'grid-2' }, [volCard, brightCard]));
    } else {
      root.appendChild(volCard);
    }
  }

  // 3. Telemetry Pulse Row (CPU/RAM, Network, Battery)
  const mods = store.config?.modules || {};
  const hw = store.hardware;
  const net = store.network;
  const batt = sys?.battery;
  const hwEnabled = mods.hardware !== false;
  const netEnabled = mods.network !== false;
  const sysEnabled = mods.system !== false;
  const makeNavCard = (enabled, tabId, children) => {
    const attrs = { class: `glass-card ${enabled ? 'is-clickable' : ''}` };
    if (enabled) {
      attrs.role = 'button';
      attrs.tabindex = '0';
      attrs.onClick = () => api.switchTab(tabId);
      attrs.onKeydown = (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          api.switchTab(tabId);
        }
      };
    }
    return el('div', attrs, children);
  };

  root.appendChild(
    el('div', { class: 'grid-3' }, [
      makeNavCard(hwEnabled, 'hardware', [
        el('div', { class: 'card-label', text: 'CPU / RAM' }),
        el('div', {
          class: 'card-value',
          style: { marginTop: '4px' },
          text: !hwEnabled ? 'Off' : (hw ? `${Math.round(hw.cpu_usage_percent)}% · ${Math.round(hw.ram_percent)}%` : 'Sampling…'),
        }),
        el('div', {
          class: 'muted',
          text: !hwEnabled ? 'Disabled in Settings' : (hw ? `${formatBytes(hw.ram_used_bytes)} RAM` : 'Hardware'),
        }),
      ]),
      makeNavCard(netEnabled, 'network', [
        el('div', { class: 'card-label', text: net?.vpn_active ? 'Network · VPN' : 'Network' }),
        el('div', {
          class: 'card-value',
          style: { marginTop: '4px' },
          text: !netEnabled ? 'Off' : (net ? (net.connected ? `↓ ${formatSpeed(net.download_bps)}` : 'Offline') : 'Probing…'),
        }),
        el('div', {
          class: 'muted',
          text: !netEnabled ? 'Disabled in Settings' : (net?.ping_ms != null ? `Ping ${net.ping_ms} ms` : (net?.adapter_name || 'Adapter')),
        }),
      ]),
      makeNavCard(sysEnabled, 'system', [
        el('div', { class: 'card-label', text: 'Power' }),
        el('div', {
          class: 'card-value',
          style: { marginTop: '4px' },
          text: batt?.present
            ? `${batt.percent ?? '--'}%${batt.charging ? ' · Chg' : ''}`
            : (batt?.ac_online ? 'AC Power' : 'Desktop'),
        }),
        el('div', {
          class: 'muted',
          text: batt?.battery_saver ? 'Battery Saver On' : (batt?.ac_online ? 'Plugged In' : 'On Battery'),
        }),
      ]),
    ])
  );

  // 4. Top Quick Actions Row (hidden when the Shortcuts / Actions module is disabled)
  const actions = store.config?.modules?.actions === false
    ? []
    : (store.config?.quick_actions || []).slice(0, 3);
  if (actions.length > 0) {
    root.appendChild(
      el('div', { class: 'grid-3' }, actions.map((item) =>
        el('button', { type: 'button', class: 'action-tile', onClick: () => api.runQuickAction(item) }, [
          icon(item.icon || 'zap', 15),
          el('span', { style: { fontSize: '11.5px', fontWeight: '600' }, text: item.label }),
        ])
      ))
    );
  }

  return root;
}
