import { activityEmoji, respectsShowEmojis, statusEmoji, withEmoji } from '../emoji.js';
import { el, formatRelativeTime, icon } from '../utils.js';

export function renderNotifications(store, api) {
  const hub = store.notifications;
  const items = hub?.items || [];
  const dnd = Boolean(store.config?.privacy?.dnd_mode);
  const showEmojis = respectsShowEmojis(store.config);

  const root = el('div', { class: 'stack' }, [
    el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '6px', flexWrap: 'wrap' } }, [
      el('span', {
        class: `badge ${hub?.windows_listener_status === 'granted' ? 'ok' : 'warn'}`,
        text: hub?.windows_listener_status === 'granted'
          ? withEmoji(statusEmoji('online', showEmojis), 'Windows Action Center: Synced')
          : withEmoji(statusEmoji('alert', showEmojis), 'Local Hub Active (Unpackaged Win32)'),
      }),
      el('div', { style: { display: 'flex', gap: '5px', flexWrap: 'wrap' } }, [
        el('button', {
          type: 'button',
          class: 'pill-btn primary',
          id: 'btn-test-sms-ding',
          onClick: () => api.triggerTestSmsDing(),
        }, withEmoji(statusEmoji('sms', showEmojis), 'Test SMS Ding')),
        el('button', {
          type: 'button',
          class: 'pill-btn',
          onClick: () => api.pushNotification('PILLVYN', 'Alert Channel Verified', 'Notification pipeline & priority routing active.', 'normal'),
        }, 'Test Alert'),
        el('button', {
          type: 'button',
          class: 'pill-btn',
          disabled: items.length === 0 || (hub?.unread_count || 0) === 0,
          onClick: () => api.notificationAction('mark_all_read'),
        }, 'Read All'),
        el('button', {
          type: 'button',
          class: 'pill-btn danger',
          disabled: items.length === 0,
          onClick: () => api.notificationAction('clear_all'),
        }, 'Clear'),
      ]),
    ]),
  ]);

  if (hub?.windows_listener_detail && hub.windows_listener_status !== 'granted') {
    root.appendChild(el('div', { class: 'notice-banner', text: hub.windows_listener_detail }));
  }

  if (dnd) {
    root.appendChild(
      el('div', {
        class: 'notice-banner',
        text: withEmoji(
          statusEmoji('dnd', showEmojis),
          'Do Not Disturb is active — only Critical alerts trigger island banners; others are archived quietly.'
        ),
      })
    );
  }

  if (items.length === 0) {
    root.appendChild(
      el('div', { class: 'empty-state' }, [
        icon('bell', 22),
        el('div', {
          style: { marginTop: '6px', fontWeight: '600', color: 'var(--text-primary)' },
          text: withEmoji(statusEmoji('idle', showEmojis), 'All Caught Up'),
        }),
        el('div', {
          class: 'muted',
          style: { marginTop: '3px' },
          text: 'SMS messages, timer alarms, system alerts, and plugin notifications appear here.',
        }),
      ])
    );
    return root;
  }

  for (const n of items) {
    const isSms = Boolean(n.is_sms);
    const prioClass = isSms ? 'sms' : (n.priority === 'critical' ? 'danger' : n.priority === 'high' ? 'warn' : '');
    const itemEmoji = activityEmoji(isSms ? 'sms' : 'notification', { priority: n.priority }, showEmojis);
    root.appendChild(
      el('div', { class: `list-item ${isSms ? 'is-sms-item' : ''}`, style: { opacity: n.read ? '0.68' : '1' } }, [
        el('div', { style: { minWidth: '0', flex: '1', cursor: 'pointer' }, onClick: () => api.notificationAction('mark_read', n.id) }, [
          el('div', { style: { display: 'flex', alignItems: 'center', gap: '6px' } }, [
            el('span', { class: `badge ${prioClass}`, text: withEmoji(itemEmoji, n.app_name || (isSms ? 'SMS' : 'System')) }),
            el('span', { style: { fontWeight: '600', fontSize: '12px', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }, text: n.title }),
          ]),
          n.body
            ? el('div', { class: 'muted', style: { marginTop: '3px', fontSize: '11px' }, text: n.body })
            : null,
          el('div', { class: 'muted', style: { fontSize: '10px', marginTop: '2px' }, text: `${formatRelativeTime(n.timestamp_ms)}${isSms ? ' · SMS Ding' : ''}${n.suppressed_by_dnd ? ' · Muted by DND' : ''}` }),
        ]),
        el('button', {
          type: 'button',
          class: 'icon-btn',
          style: { width: '24px', height: '24px' },
          title: 'Dismiss',
          onClick: () => api.notificationAction('dismiss', n.id),
        }, icon('trash', 12)),
      ])
    );
  }

  return root;
}
