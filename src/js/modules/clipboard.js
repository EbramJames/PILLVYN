import { el, formatRelativeTime, icon } from '../utils.js';

let searchTerm = '';

export function renderClipboard(store, api, rerender) {
  const cb = store.clipboard;
  const enabled = cb?.enabled ?? true;
  const allItems = cb?.items || [];
  const filtered = searchTerm
    ? allItems.filter((it) => it.preview.toLowerCase().includes(searchTerm.toLowerCase()))
    : allItems;

  const root = el('div', { class: 'stack' }, [
    el('div', { style: { display: 'flex', gap: '6px', alignItems: 'center' } }, [
      el('input', {
        type: 'search',
        class: 'input-glass',
        placeholder: 'Filter clipboard history…',
        value: searchTerm,
        onInput: (e) => {
          searchTerm = e.target.value;
          rerender();
        },
      }),
      el('button', {
        class: `pill-btn ${enabled ? 'primary' : ''}`,
        onClick: () => api.clipboardAction('toggle_enabled'),
      }, enabled ? 'Active' : 'Paused'),
      el('button', {
        class: 'pill-btn danger',
        disabled: allItems.length === 0,
        onClick: () => api.clipboardAction('clear'),
      }, 'Clear'),
    ]),
  ]);

  if (!enabled) {
    root.appendChild(
      el('div', { class: 'notice-banner', text: 'Clipboard capture is paused for privacy. Click Active to resume listening to clipboard events.' })
    );
  }

  if (filtered.length === 0) {
    root.appendChild(
      el('div', { class: 'empty-state' }, [
        icon('clipboard', 22),
        el('div', { style: { marginTop: '6px', fontWeight: '600', color: 'var(--text-primary)' }, text: searchTerm ? 'No Matching Entries' : 'Clipboard History Empty' }),
        el('div', { class: 'muted', style: { marginTop: '3px' }, text: 'Copy text or images anywhere in Windows to capture them here automatically.' }),
      ])
    );
    return root;
  }

  for (const item of filtered) {
    root.appendChild(
      el('div', { class: 'list-item' }, [
        item.kind === 'image' && item.image_data_url
          ? el('img', { class: 'clip-thumb', src: item.image_data_url, alt: 'Clipboard image' })
          : null,
        el('div', { style: { minWidth: '0', flex: '1', cursor: item.kind === 'text' ? 'pointer' : 'default' }, onClick: () => {
          if (item.kind === 'text') api.clipboardAction('restore', item.id);
        } }, [
          el('div', {
            style: { fontSize: '12px', fontWeight: '500', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' },
            text: item.preview,
          }),
          el('div', {
            class: 'muted',
            style: { fontSize: '10px' },
            text: `${item.kind === 'image' ? 'Image' : `${item.char_count || 0} chars`} · ${formatRelativeTime(item.timestamp_ms)}${item.pinned ? ' · Pinned' : ''}`,
          }),
        ]),
        el('div', { style: { display: 'flex', gap: '4px', flexShrink: '0' } }, [
          item.kind === 'text'
            ? el('button', { class: 'pill-btn', style: { padding: '3px 8px', fontSize: '10.5px' }, onClick: () => api.clipboardAction('restore', item.id) }, 'Copy')
            : null,
          el('button', {
            class: `icon-btn ${item.pinned ? 'active' : ''}`,
            style: { width: '24px', height: '24px' },
            title: item.pinned ? 'Unpin' : 'Pin',
            onClick: () => api.clipboardAction('pin', item.id),
          }, icon('pin', 12)),
          el('button', {
            class: 'icon-btn',
            style: { width: '24px', height: '24px' },
            title: 'Delete',
            onClick: () => api.clipboardAction('delete', item.id),
          }, icon('trash', 12)),
        ]),
      ])
    );
  }

  return root;
}
