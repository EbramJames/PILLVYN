import { activityEmoji, respectsShowEmojis, withEmoji } from '../emoji.js';
import { createRangeSlider, el, formatDuration, icon } from '../utils.js';

export function renderMusic(store, api) {
  const media = store.media;
  const showEmojis = respectsShowEmojis(store.config);
  if (!media || !media.active) {
    return el('div', { class: 'empty-state' }, [
      icon('music', 22),
      el('div', { style: { marginTop: '8px', fontWeight: '600', color: 'var(--text-primary)' }, text: withEmoji(showEmojis ? '🎧' : '', 'No Active Media Session') }),
      el('div', { class: 'muted', style: { marginTop: '4px' }, text: 'Start playback in Spotify, Chrome, Edge, Firefox, VLC, or Apple Music.' }),
    ]);
  }

  const dur = Math.max(0, media.duration_sec || 0);
  const pos = Math.min(dur || media.position_sec || 0, Math.max(0, media.position_sec || 0));
  const sessions = media.sessions || [];
  const root = el('div', { class: 'stack' });

  if (sessions.length > 1) {
    root.appendChild(
      el('div', { class: 'session-bar' }, sessions.map((s) =>
        el('button', {
          type: 'button',
          class: `session-chip ${s.is_active ? 'active' : ''}`,
          title: `${s.source_app}: ${s.title || 'Session'}`,
          onClick: () => api.mediaControl('select_session', null, s.source_id),
        }, `${s.source_app}${s.status === 'Playing' ? (showEmojis ? ' · 🎵 Live' : ' · Live') : ''}`)
      ))
    );
  }

  const posReadout = el('span', { text: formatDuration(pos) });
  const seekSlider = createRangeSlider({
    value: Math.round(pos),
    min: 0,
    max: Math.max(1, Math.round(dur)),
    disabled: !media.can_seek || dur <= 0,
    ariaLabel: 'Playback Position',
    title: media.can_seek && dur > 0 ? 'Drag to seek track position' : 'Seek unavailable for this media source',
    onDragState: (d) => api.setRangeDragging(d),
    onInputValue: (v) => {
      posReadout.textContent = formatDuration(v);
    },
    onCommitValue: (v) => {
      posReadout.textContent = formatDuration(v);
      api.mediaControl('seek', Number(v));
    },
  });

  const hasArt = Boolean(media.thumbnail_data_url);
  const statusEmojiChar = activityEmoji('media', { status: media.status }, showEmojis);

  root.appendChild(
    el('div', { class: 'glass-card' }, [
      el('div', {
        class: `music-hero ${hasArt ? 'has-art' : ''}`,
        style: hasArt ? { '--album-art-bg': `url("${media.thumbnail_data_url}")` } : undefined,
      }, [
        el('div', { class: 'album-art' }, [
          hasArt
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
            el('span', { text: withEmoji(statusEmojiChar, `${media.source_app || 'Media'} · ${media.status}`) }),
          ]),
        ]),
      ]),
      el('div', { style: { marginTop: '12px' } }, [
        seekSlider,
        el('div', { class: 'time-row' }, [
          posReadout,
          el('span', { text: dur > 0 ? formatDuration(dur) : '--:--' }),
        ]),
      ]),
      el('div', { class: 'media-controls' }, [
        el('button', {
          type: 'button',
          class: 'icon-btn',
          title: 'Previous Track',
          disabled: !media.can_previous,
          onClick: () => api.mediaControl('previous'),
        }, icon('prev', 14)),
        el('button', {
          type: 'button',
          class: 'media-btn-main',
          title: media.status === 'Playing' ? 'Pause' : 'Play',
          disabled: !media.can_play_pause,
          onClick: () => api.mediaControl('play_pause'),
        }, icon(media.status === 'Playing' ? 'pause' : 'play', 16)),
        el('button', {
          type: 'button',
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
