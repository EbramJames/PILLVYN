// Fluid micro-interactions, Web Audio preview chime, and SMS Arrival Ding choreography.

let dingResetTimer = null;
let sharedAudioCtx = null;

function prefersReducedMotion() {
  return document.body.classList.contains('reduced-motion');
}

export function pulseElement(node, className = 'is-pop', durationMs = 260) {
  if (!node || prefersReducedMotion()) return;
  node.classList.remove(className);
  // Trigger style recalc so consecutive clicks replay the spring bounce cleanly.
  void node.offsetWidth;
  node.classList.add(className);
  setTimeout(() => node.classList.remove(className), durationMs);
}

/**
 * Plays a soft, studio-tuned Web Audio crystalline two-note bell chime
 * for UI/preview contexts or browser tests without requiring winmm.
 */
export function playWebDing() {
  try {
    const AudioCtx = window.AudioContext || window.webkitAudioContext;
    if (!AudioCtx) return;
    if (!sharedAudioCtx) {
      sharedAudioCtx = new AudioCtx();
    }
    if (sharedAudioCtx.state === 'suspended') {
      sharedAudioCtx.resume().catch(() => {});
    }
    const now = sharedAudioCtx.currentTime;

    const chimeNote = (freq, startAt, peakGain, decaySec) => {
      const osc = sharedAudioCtx.createOscillator();
      const gain = sharedAudioCtx.createGain();
      osc.type = 'sine';
      osc.frequency.setValueAtTime(freq, startAt);
      gain.gain.setValueAtTime(0.0001, startAt);
      gain.gain.linearRampToValueAtTime(peakGain, startAt + 0.007);
      gain.gain.exponentialRampToValueAtTime(0.0001, startAt + decaySec);
      osc.connect(gain);
      gain.connect(sharedAudioCtx.destination);
      osc.start(startAt);
      osc.stop(startAt + decaySec + 0.02);
    };

    chimeNote(1318.51, now, 0.09, 0.28);       // E6 crystalline primary
    chimeNote(2637.02, now, 0.025, 0.16);      // Delicate octave shimmer
    chimeNote(1975.53, now + 0.09, 0.06, 0.26); // B6 resonant harmonic tail
  } catch {
    // Ignore when Web Audio is unavailable or gated by policy
  }
}

/**
 * Triggers the Apple-inspired Dynamic Island "ding" ripple + capsule bloom
 * upon SMS / high-priority arrival.
 */
export function triggerIslandDing(shell, payload = {}, durationMs = 1550) {
  if (!shell) return;
  clearTimeout(dingResetTimer);

  const compactView = document.getElementById('view-compact');
  const artWrap = document.getElementById('compact-artwork');
  const isSms = Boolean(payload?.is_sms ?? true);

  shell.classList.remove('is-ding-sms', 'is-ding-alert');
  void shell.offsetWidth;
  shell.classList.add(isSms ? 'is-ding-sms' : 'is-ding-alert');

  if (compactView) {
    compactView.classList.remove('is-ding-pulse');
    void compactView.offsetWidth;
    compactView.classList.add('is-ding-pulse');
  }

  if (artWrap && isSms) {
    artWrap.classList.add('is-sms-badge');
  }

  dingResetTimer = setTimeout(() => {
    shell.classList.remove('is-ding-sms', 'is-ding-alert');
    compactView?.classList.remove('is-ding-pulse');
  }, durationMs);
}
