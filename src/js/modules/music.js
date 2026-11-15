import { el, formatDuration, icon } from '../utils.js';

export function renderMusic(store, api) {
  const media = store.media;
  if (!media || !media.active) {
    return el('div', { class: 'empty-state' }, [
      icon('music', 24),
      el('div', { style: { marginTop: '8px', fontWeight: '600', color: 'var(--text-primary)' }, text: 'No Active Windows Media Session' }),
      el('div', { class: 'muted', style: { marginTop: '4px' }, text: 'Play audio or video in Spotify, Edge, Chrome, Firefox, VLC, or Apple Music to control playback from PILLVYN.' }),
    ]);
  }

  const dur = Math.max(0, media.duration_sec || 0);
  const pos = Math.min(dur || media.position_sec || 0, Math.max(0, media.position_sec || 0));

  return el('div', { class: 'stack' }, [
    el('div', { class: 'glass-card' }, [
      el('div', { class: 'music-hero' }, [
        el('div', { class: 'album-art' }, [
          media.thumbnail_data_url
            ? el('img', { src: media.thumbnail_data_url, alt: media.title || 'Album cover' })
            : icon('music', 28),
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
          disabled: !media.can_seek,
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
        }, icon('prev', 15)),
        el('button', {
          class: 'media-btn-main',
          title: media.status === 'Playing' ? 'Pause' : 'Play',
          disabled: !media.can_play_pause,
          onClick: () => api.mediaControl('play_pause'),
        }, icon(media.status === 'Playing' ? 'pause' : 'play', 17)),
        el('button', {
          class: 'icon-btn',
          title: 'Next Track',
          disabled: !media.can_next,
          onClick: () => api.mediaControl('next'),
        }, icon('next', 15)),
      ]),
    ]),
  ]);
}
