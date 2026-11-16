// Reactive application store for the PILLVYN Dynamic Island.

export const store = {
  islandState: 'compact', // 'minimized' | 'compact' | 'expanded'
  activeTab: 'overview', // 'overview' | 'music' | 'system' | 'monitor' | 'notifications' | 'clipboard' | 'utilities'
  monitorSubTab: 'hardware', // 'hardware' | 'network' | 'downloads' | 'gaming'
  utilitiesSubTab: 'widgets', // 'widgets' | 'actions' | 'plugins' | 'ai'
  pinned: false,
  compactHovered: false,
  activityCycleIndex: 0,
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
  appVersion: '2.0.0',
  rangeDragging: false,
  seekingUntil: 0,
  weatherLoading: false,
  panelNotice: null,
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
