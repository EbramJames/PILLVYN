import { el, formatBytes } from '../utils.js';

export function renderHardware(store) {
  const hw = store.hardware;
  if (!hw) {
    return el('div', { class: 'empty-state', text: 'Collecting CPU, RAM, GPU, and storage telemetry…' });
  }

  const cpuPct = Math.round(hw.cpu_usage_percent || 0);
  const ramPct = Math.round(hw.ram_percent || 0);
  const ghz = hw.cpu_freq_mhz ? `${(hw.cpu_freq_mhz / 1000).toFixed(2)} GHz` : '--';

  const root = el('div', { class: 'stack' }, [
    el('div', { class: 'grid-2' }, [
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: 'CPU' }),
          el('span', { class: 'card-value', text: `${cpuPct}%` }),
        ]),
        el('div', { class: 'meter-track' }, [el('div', { class: 'meter-fill', style: { width: `${cpuPct}%` } })]),
        el('div', {
          class: 'muted',
          style: { marginTop: '6px', fontSize: '10.5px', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' },
          text: `${hw.cpu_brand} · ${hw.cpu_cores}T · ${ghz}${hw.cpu_temp_c ? ` · ${Math.round(hw.cpu_temp_c)}°C` : ''}`,
        }),
      ]),
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: 'Memory (RAM)' }),
          el('span', { class: 'card-value', text: `${ramPct}%` }),
        ]),
        el('div', { class: 'meter-track' }, [el('div', { class: 'meter-fill', style: { width: `${ramPct}%` } })]),
        el('div', {
          class: 'muted',
          style: { marginTop: '6px', fontSize: '10.5px' },
          text: `${formatBytes(hw.ram_used_bytes)} / ${formatBytes(hw.ram_total_bytes)}`,
        }),
      ]),
    ]),
  ]);

  if (hw.gpus && hw.gpus.length > 0) {
    for (const gpu of hw.gpus) {
      const util = gpu.utilization_percent;
      const vram = gpu.vram_used_mb != null && gpu.vram_total_mb != null
        ? `${gpu.vram_used_mb} / ${gpu.vram_total_mb} MB VRAM`
        : (gpu.vram_total_mb ? `${gpu.vram_total_mb} MB VRAM` : 'Shared Memory');
      const extras = [
        vram,
        gpu.temperature_c != null ? `${gpu.temperature_c}°C` : null,
        gpu.power_watts != null ? `${gpu.power_watts.toFixed(0)}W` : null,
        gpu.telemetry_source,
      ].filter(Boolean).join(' · ');

      root.appendChild(
        el('div', { class: 'glass-card' }, [
          el('div', { class: 'card-title-row' }, [
            el('span', { class: 'card-label', text: gpu.name }),
            el('span', { class: 'card-value', text: util != null ? `${util}%` : 'Active' }),
          ]),
          util != null
            ? el('div', { class: 'meter-track', style: { marginBottom: '6px' } }, [
                el('div', { class: 'meter-fill', style: { width: `${util}%` } }),
              ])
            : null,
          el('div', { class: 'muted', style: { fontSize: '10.5px' }, text: extras }),
        ])
      );
    }
  }

  if (hw.disks && hw.disks.length > 0) {
    root.appendChild(
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-label', style: { marginBottom: '6px' }, text: 'Storage Volumes' }),
        el('div', { class: 'stack', style: { gap: '6px' } }, hw.disks.map((d) => {
          const pct = Math.round(d.used_percent || 0);
          return el('div', {}, [
            el('div', { style: { display: 'flex', justifyContent: 'space-between', fontSize: '11px', marginBottom: '3px' } }, [
              el('span', { text: `${d.mount} ${d.name ? `(${d.name})` : ''}` }),
              el('span', { class: 'muted', text: `${formatBytes(d.available_bytes)} free of ${formatBytes(d.total_bytes)} (${pct}%)` }),
            ]),
            el('div', { class: 'meter-track', style: { height: '4px' } }, [
              el('div', { class: 'meter-fill', style: { width: `${pct}%` } }),
            ]),
          ]);
        })),
      ])
    );
  }

  return root;
}
