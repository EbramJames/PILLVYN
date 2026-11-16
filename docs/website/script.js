const capsule = document.getElementById('demo-capsule');
const dingBtn = document.getElementById('btn-demo-ding');
const dcArt = document.getElementById('dc-art');
const dcTitle = document.getElementById('dc-title');
const dcSub = document.getElementById('dc-sub');
const dcMiniBadge = document.getElementById('dc-mini-badge');
let dingReset = null;

document.querySelectorAll('[data-demo-state]').forEach((btn) => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('[data-demo-state]').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    capsule.setAttribute('data-state', btn.getAttribute('data-demo-state'));
  });
});

document.querySelectorAll('[data-demo-theme]').forEach((btn) => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('[data-demo-theme]').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    capsule.setAttribute('data-theme', btn.getAttribute('data-demo-theme'));
  });
});

function playPreviewDing() {
  try {
    const AudioCtx = window.AudioContext || window.webkitAudioContext;
    if (!AudioCtx) return;
    const ctx = new AudioCtx();
    const now = ctx.currentTime;
    const chime = (freq, start, peak, dur) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = 'sine';
      osc.frequency.setValueAtTime(freq, start);
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.linearRampToValueAtTime(peak, start + 0.007);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + dur);
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start(start);
      osc.stop(start + dur + 0.02);
    };
    chime(1318.51, now, 0.09, 0.28);
    chime(1975.53, now + 0.09, 0.06, 0.26);
  } catch {
    // Ignore when Web Audio is restricted
  }
}

dingBtn?.addEventListener('click', () => {
  clearTimeout(dingReset);
  capsule.classList.remove('is-ding-sms');
  void capsule.offsetWidth;
  capsule.classList.add('is-ding-sms');
  playPreviewDing();

  if (dcArt) dcArt.textContent = '💬';
  if (dcTitle) dcTitle.textContent = '💬 New SMS · Alex';
  if (dcSub) dcSub.textContent = 'Messages · "Hey! PILLVYN v2.0.0 feels super smooth."';
  if (dcMiniBadge) dcMiniBadge.textContent = '💬 SMS';

  dingReset = setTimeout(() => {
    capsule.classList.remove('is-ding-sms');
    if (dcArt) dcArt.textContent = '🎵';
    if (dcTitle) dcTitle.textContent = '🎵 Windows GSMTC Media';
    if (dcSub) dcSub.textContent = 'Click Trigger SMS Ding or Expanded Hub';
    if (dcMiniBadge) dcMiniBadge.textContent = '🎵 LIVE';
  }, 3200);
});
