// Reactive application store for the PILLVYN Dynamic Island.

export const store = {
  islandState: 'compact', // 'minimized' | 'compact' | 'expanded'
  activeTab: 'overview',
  config: null,
  monitors: [],
  activities: [],
  media: null,
  system: null,
  hardware: null,
  gaming: null,
  downloads: null,
  notifications: null,
  clipboard: null,
  network: null,
  timers: [],
  stopwatchMs: 0,
  stopwatchRunning: false,
  weather: null,
  plugins: null,
  activeProfile: null,
  appVersion: '1.0.0',
  aiResponse: null,
  aiError: null,
  aiLoading: false,
};

const listeners = new Set();

export function subscribe(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function patchStore(partial, reason = 'update') {
  Object.assign(store, partial);
  for (const fn of listeners) {
    fn(store, reason);
  }
}
