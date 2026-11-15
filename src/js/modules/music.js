import { el, formatDuration, icon } from '../utils.js';

export function renderMusic(store, api) {
  const media = store.media;
  if (!media || !media.active) {
    return el('div', { class: 'empty-state' }, [
      icon('music', 22),
      el('div', { style: { marginTop: '8px', fontWeight: '600', color: 'var(--text-primary)' }, text: 'No Active Media Session' }),
      el('div', { class: 'muted', style: { marginTop: '4px' }, text: 'Start playback in Spotify, Chrome, Edge, Firefox, VLC, or Apple Music.' }),
    ]);
  }

  const dur = Math.max(0, media.duration_sec || 0);
  const pos = Math.min(dur || media.position_sec || 0, Math.max(0, media.position_sec || 0));
  const sessions = media.sessions || [];
  const sliderPct = dur > 0 ? Math.min(100, (pos / dur) * 100) : 0;
  const sliderBg = `linear-gradient(90deg, var(--accent) 0%, var(--accent) ${sliderPct}%, rgba(255, 255, 255, 0.14) ${sliderPct}%)`;

  const root = el('div', { class: 'stack' });

  if (sessions.length > 1) {
    root.appendChild(
      el('div', { class: 'session-bar' }, sessions.map((s) =>
        el('button', {
          class: `session-chip ${s.is_active ? 'active' : ''}`,
          title: `${s.source_app}: ${s.title || 'Session'}`,
          onClick: () => api.mediaControl('select_session', null, s.source_id),
        }, `${s.source_app}${s.status === 'Playing' ? ' · Live' : ''}`)
      ))
    );
  }

  root.appendChild(
    el('div', { class: 'glass-card' }, [
      el('div', { class: 'music-hero' }, [
        el('div', { class: 'album-art' }, [
          media.thumbnail_data_url
            ? el('img', { src: media.thumbnail_data_url, alt: media.title || 'Album cover' })
            : icon('music', 24),
        ]),
        el('div', { class: 'track-meta' }, [
          el('div', { class: 'track-title', text: media.title || 'Unknown Track' }),
          el('div', { class: 'track-artist', text: media.artist || 'Unknown Artist' }),
          media.album
            ? el('div', { class: 'muted', style: { fontSize: '11px', marginTop: '2px' }, text: media.album })
            : null,
          el('span', { class: 'track-source' }, [
            el('span', { text: `${media.source_app || 'Media'} · ${media.status}` }),
          ]),
        ]),
      ]),
      el('div', { style: { marginTop: '12px' } }, [
        el('input', {
          type: 'range',
          class: 'range-slider',
          min: '0',
          max: String(Math.max(1, Math.round(dur))),
          value: String(Math.round(pos)),
          style: { background: sliderBg },
          disabled: !media.can_seek,
          onInput: (e) => {
            const p = dur > 0 ? Math.min(100, (Number(e.target.value) / dur) * 100) : 0;
            e.target.style.background = `linear-gradient(90deg, var(--accent) 0%, var(--accent) ${p}%, rgba(255, 255, 255, 0.14) ${p}%)`;
          },
          onChange: (e) => api.mediaControl('seek', Number(e.target.value)),
        }),
        el('div', { class: 'time-row' }, [
          el('span', { text: formatDuration(pos) }),
          el('span', { text: dur > 0 ? formatDuration(dur) : '--:--' }),
        ]),
      ]),
      el('div', { class: 'media-controls' }, [
        el('button', {
          class: 'icon-btn',
          title: 'Previous Track',
          disabled: !media.can_previous,
          onClick: () => api.mediaControl('previous'),
        }, icon('prev', 14)),
        el('button', {
          class: 'media-btn-main',
          title: media.status === 'Playing' ? 'Pause' : 'Play',
          disabled: !media.can_play_pause,
          onClick: () => api.mediaControl('play_pause'),
        }, icon(media.status === 'Playing' ? 'pause' : 'play', 16)),
        el('button', {
          class: 'icon-btn',
          title: 'Next Track',
          disabled: !media.can_next,
          onClick: () => api.mediaControl('next'),
        }, icon('next', 14)),
      ]),
    ])
  );

  return root;
}
