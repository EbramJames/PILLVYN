// Unified, toggleable intuitive emoji helpers for PILLVYN v2.0 Dynamic Island, Settings, and Tray.

export function respectsShowEmojis(config) {
  return config?.appearance?.show_emojis !== false;
}

export function withEmoji(emoji, label) {
  const cleanLabel = label == null ? '' : String(label);
  if (!emoji) return cleanLabel;
  return cleanLabel ? `${emoji} ${cleanLabel}` : emoji;
}

export function weatherEmoji(condition = '', enabled = true) {
  if (!enabled) return '';
  const c = String(condition || '').toLowerCase();
  if (!c || c === 'offline') return '🌡️';
  if (c.includes('thunder') || c.includes('storm') || c.includes('lightning')) return '⛈️';
  if (c.includes('snow') || c.includes('sleet') || c.includes('ice') || c.includes('flurr') || c.includes('blizzard')) return '❄️';
  if (c.includes('rain') || c.includes('drizzle') || c.includes('shower')) return '🌧️';
  if (c.includes('fog') || c.includes('mist') || c.includes('haze') || c.includes('smoke')) return '🌫️';
  if (c.includes('partly') || c.includes('scattered') || c.includes('few clouds') || c.includes('broken')) return '🌤️';
  if (c.includes('cloud') || c.includes('overcast')) return '☁️';
  if (c.includes('wind') || c.includes('breez') || c.includes('gale')) return '💨';
  if (c.includes('night')) return '🌙';
  if (c.includes('clear') || c.includes('sun') || c.includes('fair') || c.includes('bright')) return '☀️';
  return '⛅';
}

export function batteryEmoji(percent = null, charging = false, present = true, enabled = true) {
  if (!enabled) return '';
  if (!present) return '🖥️';
  if (charging) return '⚡';
  if (percent != null && Number(percent) <= 20) return '🪫';
  return '🔋';
}

export function activityEmoji(kind, opts = {}, enabled = true) {
  if (!enabled) return '';
  switch (kind) {
    case 'sms':
      return '💬';
    case 'notification':
      return opts.priority === 'critical' ? '🚨' : '🔔';
    case 'media':
      return opts.status === 'Playing' ? '🎵' : '🎧';
    case 'timer':
      return opts.completed ? '⏰' : '⏱️';
    case 'download':
      return opts.completed ? '✅' : '📥';
    case 'gaming':
      return '🎮';
    case 'system_hud':
      if (opts.muted || opts.title === 'Muted') return '🔇';
      if (opts.brightness || opts.title === 'Brightness') return '☀️';
      return '🔊';
    case 'clipboard':
      return opts.pinned ? '📌' : '📋';
    case 'ai':
      return '✨';
    case 'plugin':
      return '🧩';
    case 'weather':
      return weatherEmoji(opts.condition || '', true);
    case 'battery':
      return batteryEmoji(opts.percent, opts.charging, opts.present, true);
    default:
      return '✨';
  }
}

export function statusEmoji(key, enabled = true) {
  if (!enabled) return '';
  const map = {
    online: '🟢',
    offline: '📡',
    dnd: '🔕',
    focus: '🌙',
    hidden: '🙈',
    playing: '🎵',
    paused: '⏸️',
    sms: '💬',
    alert: '🔔',
    idle: '✨',
    pin: '📌',
    wifi: '🌐',
    vpn: '🛡️',
    gaming: '🎮',
    download: '📥',
    completed: '✅',
    timer: '⏱️',
    stopwatch: '⏱️',
    clock: '🕒',
    weather: '🌤️',
    fire: '🔥',
    rocket: '🚀',
  };
  return map[key] || '';
}
