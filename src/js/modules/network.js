import { respectsShowEmojis, statusEmoji, withEmoji } from '../emoji.js';
import { el, formatBytes, formatSpeed } from '../utils.js';

export function renderNetwork(store) {
  const net = store.network;
  if (!net) {
    return el('div', { class: 'empty-state', text: 'Sampling Windows network adapters and ICMP latency…' });
  }
  const showEmojis = respectsShowEmojis(store.config);

  return el('div', { class: 'stack' }, [
    el('div', { class: 'grid-2' }, [
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: withEmoji(showEmojis ? '⬇️' : '', 'Download') }),
          el('span', { class: 'badge ok', text: withEmoji(statusEmoji('online', showEmojis), 'LIVE') }),
        ]),
        el('div', { class: 'card-value', style: { fontSize: '17px' }, text: formatSpeed(net.download_bps) }),
        el('div', { class: 'muted', style: { marginTop: '4px' }, text: `Session Rx: ${formatBytes(net.total_rx_bytes)}` }),
      ]),
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { class: 'card-label', text: withEmoji(showEmojis ? '⬆️' : '', 'Upload') }),
          el('span', { class: 'badge', text: net.ping_ms != null ? `${net.ping_ms} ms` : 'Ping --' }),
        ]),
        el('div', { class: 'card-value', style: { fontSize: '17px' }, text: formatSpeed(net.upload_bps) }),
        el('div', { class: 'muted', style: { marginTop: '4px' }, text: `Session Tx: ${formatBytes(net.total_tx_bytes)}` }),
      ]),
    ]),

    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: 'Connectivity & Tunnel' }),
        el('span', {
          class: `badge ${net.connected ? 'ok' : 'danger'}`,
          text: net.connected
            ? withEmoji(statusEmoji('wifi', showEmojis), 'Online')
            : withEmoji(statusEmoji('offline', showEmojis), 'Offline'),
        }),
      ]),
      el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' } }, [
        el('div', {}, [
          el('div', { style: { fontWeight: '600', fontSize: '12.5px' }, text: net.adapter_name || 'No Active Adapter' }),
          el('div', { class: 'muted', text: net.vpn_active ? `VPN Active: ${net.vpn_name || 'Encrypted Tunnel'}` : 'No active VPN adapter detected' }),
        ]),
        el('span', {
          class: `badge ${net.vpn_active ? 'ok' : ''}`,
          text: net.vpn_active ? withEmoji(statusEmoji('vpn', showEmojis), 'VPN PROTECTED') : 'DIRECT',
        }),
      ]),
    ]),
  ]);
}
