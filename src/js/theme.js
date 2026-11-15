export const THEME_PRESETS = [
  {
    id: 'obsidian',
    name: 'Obsidian',
    desc: 'Pure dark glass',
    surface: '#090a0e',
    accent: '#5b8cff',
  },
  {
    id: 'frosted',
    name: 'Frosted Slate',
    desc: 'Elevated slate glass',
    surface: '#181c26',
    accent: '#7aa6ff',
  },
  {
    id: 'aurora',
    name: 'Aurora',
    desc: 'Cool teal mineral',
    surface: '#090f14',
    accent: '#38d9c0',
  },
  {
    id: 'midnight',
    name: 'Midnight',
    desc: 'Deep violet obsidian',
    surface: '#06070b',
    accent: '#8c78ff',
  },
  {
    id: 'crimson',
    name: 'Crimson',
    desc: 'Warm ruby obsidian',
    surface: '#0f090c',
    accent: '#f05a74',
  },
];

export const VALID_THEMES = new Set(THEME_PRESETS.map((t) => t.id));

export function normalizeHexColor(value, fallback = '#5b8cff') {
  if (typeof value !== 'string') return fallback;
  const trimmed = value.trim();
  return /^#[0-9a-fA-F]{6}$/.test(trimmed) ? trimmed.toLowerCase() : fallback;
}

export function hexToRgb(hex) {
  const clean = normalizeHexColor(hex).slice(1);
  const num = parseInt(clean, 16);
  return {
    r: (num >> 16) & 255,
    g: (num >> 8) & 255,
    b: num & 255,
  };
}

export function mixHexWithWhite(hex, amount = 0.28) {
  const { r, g, b } = hexToRgb(hex);
  const mr = Math.round(r + (255 - r) * amount);
  const mg = Math.round(g + (255 - g) * amount);
  const mb = Math.round(b + (255 - b) * amount);
  return `#${[mr, mg, mb].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

export function getThemePreset(themeId) {
  return THEME_PRESETS.find((t) => t.id === themeId) || THEME_PRESETS[0];
}

export function applyAppearanceVars(
  appearance = {},
  {
    themeTarget = document.body,
    varTarget = document.documentElement,
    applyOpacity = true,
    minOpacity = 0.45,
  } = {}
) {
  const themeId = VALID_THEMES.has(appearance.theme) ? appearance.theme : 'obsidian';
  const accent = normalizeHexColor(appearance.accent_color, getThemePreset(themeId).accent);
  const { r, g, b } = hexToRgb(accent);
  const accent2 = mixHexWithWhite(accent, 0.26);

  if (themeTarget) {
    themeTarget.dataset.theme = themeId;
    if (appearance.reduced_motion !== undefined) {
      themeTarget.dataset.reducedMotion = String(Boolean(appearance.reduced_motion));
      themeTarget.classList.toggle('reduced-motion', Boolean(appearance.reduced_motion));
    }
  }

  for (const el of new Set([varTarget, themeTarget].filter(Boolean))) {
    el.style.setProperty('--accent', accent);
    el.style.setProperty('--accent-2', accent2);
    el.style.setProperty('--accent-soft', `rgba(${r}, ${g}, ${b}, 0.15)`);
    el.style.setProperty('--accent-glow', `rgba(${r}, ${g}, ${b}, 0.26)`);

    if (applyOpacity && typeof appearance.opacity === 'number') {
      const clamped = Math.max(minOpacity, Math.min(0.98, appearance.opacity));
      el.style.setProperty('--island-opacity', clamped.toFixed(2));
    }
    if (typeof appearance.scale === 'number') {
      el.style.setProperty('--island-scale', appearance.scale.toFixed(2));
    }
  }
}

export function updateRangeTrackFill(input) {
  if (!input) return;
  const min = Number(input.min || 0);
  const max = Number(input.max || 100);
  const val = Number(input.value || 0);
  const span = max - min;
  const pct = span > 0 ? Math.max(0, Math.min(100, ((val - min) / span) * 100)) : 0;
  input.style.setProperty('--range-pct', `${pct.toFixed(2)}%`);
}
