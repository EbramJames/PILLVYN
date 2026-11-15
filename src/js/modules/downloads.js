import { el, formatBytes, formatDuration, formatSpeed, icon } from '../utils.js';

export function renderDownloads(store) {
  const dl = store.downloads;
  if (!dl || !dl.items || dl.items.length === 0) {
    return el('div', { class: 'stack' }, [
      el('div', { class: 'empty-state' }, [
        icon('download', 24),
        el('div', { style: { marginTop: '8px', fontWeight: '600', color: 'var(--text-primary)' }, text: 'No Active Downloads' }),
        el('div', { class: 'muted', style: { marginTop: '4px' }, text: 'Live progress automatically appears when Chrome, Edge, Firefox, Opera, or Steam downloads are active.' }),
      ]),
      dl?.watched_sources?.length
        ? el('div', { class: 'muted', style: { fontSize: '10.5px', padding: '0 4px' }, text: `Watching: ${dl.watched_sources.join(' · ')}` })
        : null,
    ]);
  }

  return el('div', { class: 'stack' }, dl.items.map((item) => {
    const pct = item.progress_percent != null ? Math.round(item.progress_percent) : null;
    const sizeText = item.total_bytes
      ? `${formatBytes(item.downloaded_bytes)} / ${formatBytes(item.total_bytes)}`
      : formatBytes(item.downloaded_bytes);
    const metaRight = item.status === 'completed'
      ? 'Completed'
      : `${formatSpeed(item.speed_bps)}${item.eta_seconds != null ? ` · ETA ${formatDuration(item.eta_seconds)}` : ''}`;

    return el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', {
          style: { fontWeight: '600', fontSize: '12.5px', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' },
          text: item.name,
        }),
        el('span', { class: `badge ${item.status === 'completed' ? 'ok' : 'warn'}`, text: pct != null ? `${pct}%` : item.source }),
      ]),
      pct != null
        ? el('div', { class: 'meter-track', style: { marginBottom: '6px' } }, [
            el('div', { class: 'meter-fill', style: { width: `${pct}%` } }),
          ])
        : null,
      el('div', { style: { display: 'flex', justifyContent: 'space-between', fontSize: '11px' } }, [
        el('span', { class: 'muted', text: `${item.source} · ${sizeText}` }),
        el('span', { class: 'muted', text: metaRight }),
      ]),
    ]);
  }));
}
