/* PILLVYN AI Voice Assistant frontend engine:
 * - RNNoise WASM (48 kHz real-time neural denoising)
 * - Silero VAD ONNX (16 kHz stateful neural voice activity detection & sentence endpointing)
 * - Wake Word ("Hello PILLVYN" / "سلام پیلوین") + Active Conversation state machine
 * - whisper.cpp auto Persian/English STT + OpenRouter Ling-3.2-Flash + Edge TTS MP3 player
 */
(() => {
'use strict';

const $ = (s) => document.querySelector(s);
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

const state = {
  status: {
    rnnoise: false, silero: false, whisperBin: false, whisperModel: false,
    voiceReady: false, hasKey: false, keyMasked: '', installing: false,
    assetUrl: '', model: 'inclusionai/ling-3.2-flash:free', modelId: 'base',
  },
  install: null,          // { step, pct, done, error }
  mode: 'idle',           // 'idle' | 'standby' | 'listening' | 'transcribing' | 'thinking' | 'speaking'
  active: false,          // whether the pill is currently displaying the AI view
  micActive: false,
  micError: '',
  dspReady: false,
  dspLoading: false,
  dspError: '',
  energy: 0,
  vadProb: 0,
  pillText: 'Say “Hello PILLVYN” or ask anything…',
  messages: [],           // [{ role: 'user'|'assistant'|'error', text, tools?, lang?, ts }]
  audio: null,
  listenTimeout: 0,
  busy: false,
};

// ------------------------------------------------------------ DSP: RNNoise WASM + Silero VAD ONNX
let rnnMod = null, rnnCtx = 0, rnnPtr = 0;
let ortSession = null, vadState = null, vadCtx = new Float32Array(64);
let vadInputNames = [];
let pcm48Queue = [];
let pcm16Queue = [];
let vadQueue = [];
let vadProcessing = false;

// Utterance endpointing buffers (16 kHz mono float32)
const PREROLL_FRAMES = 15;       // 15 * 32 ms = 480 ms pre-roll so word starts are never clipped
const preRoll = [];
let inSpeech = false;
let speechChunks = [];
let speechFrames = 0;
let silenceFrames = 0;

function resetVadEndpoint() {
  inSpeech = false;
  speechChunks = [];
  speechFrames = 0;
  silenceFrames = 0;
  preRoll.length = 0;
  if (vadState) vadState.fill(0);
  vadCtx.fill(0);
}

function loadScriptOnce(src) {
  return new Promise((resolve, reject) => {
    if (document.querySelector(`script[src="${src}"]`)) { resolve(); return; }
    const s = document.createElement('script');
    s.src = src;
    s.async = true;
    s.onload = () => resolve();
    s.onerror = () => reject(new Error('Failed to load ' + src));
    document.head.appendChild(s);
  });
}

async function ensureDsp() {
  if (state.dspReady) return true;
  if (state.dspLoading) return false;
  const base = state.status.assetUrl;
  if (!base || !state.status.rnnoise || !state.status.silero) return false;

  state.dspLoading = true;
  state.dspError = '';
  try {
    // 1. Load official Xiph RNNoise WASM
    const rnnExport = (await import(`${base}/rnnoise.js`)).default;
    rnnMod = await rnnExport({ locateFile: (f) => `${base}/${f}` });
    rnnCtx = rnnMod._rnnoise_create();
    rnnPtr = rnnMod._malloc(480 * 4);

    // 2. Load ONNX Runtime Web (single-threaded SIMD WASM — zero COOP/COEP headers required)
    await loadScriptOnce(`${base}/ort.wasm.min.js`);
    const ort = window.ort;
    if (!ort) throw new Error('ONNX Runtime Web did not initialize');
    ort.env.wasm.wasmPaths = `${base}/`;
    ort.env.wasm.numThreads = 1;
    ort.env.wasm.proxy = false;

    // 3. Load official Silero VAD ONNX model
    ortSession = await ort.InferenceSession.create(`${base}/silero_vad.onnx`, {
      executionProviders: ['wasm'],
    });
    vadInputNames = ortSession.inputNames || [];
    vadState = new Float32Array(2 * 1 * 128);
    vadCtx = new Float32Array(64);
    state.dspReady = true;
    notifyUi();
    return true;
  } catch (e) {
    state.dspError = String(e && e.message || e);
    state.dspReady = false;
    notifyUi();
    return false;
  } finally {
    state.dspLoading = false;
  }
}

function b64ToInt16(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new Int16Array(bytes.buffer);
}

function uint8ToB64(bytes) {
  let bin = '';
  const step = 0x8000;
  for (let i = 0; i < bytes.length; i += step) {
    bin += String.fromCharCode.apply(null, bytes.subarray(i, i + step));
  }
  return btoa(bin);
}

function encodeWav16k(chunks) {
  let total = 0;
  for (const c of chunks) total += c.length;
  const buf = new ArrayBuffer(44 + total * 2);
  const dv = new DataView(buf);
  const str = (off, s) => { for (let i = 0; i < s.length; i++) dv.setUint8(off + i, s.charCodeAt(i)); };
  str(0, 'RIFF'); dv.setUint32(4, 36 + total * 2, true);
  str(8, 'WAVE'); str(12, 'fmt '); dv.setUint32(16, 16, true);
  dv.setUint16(20, 1, true); dv.setUint16(22, 1, true);
  dv.setUint32(24, 16000, true); dv.setUint32(28, 32000, true);
  dv.setUint16(32, 2, true); dv.setUint16(34, 16, true);
  str(36, 'data'); dv.setUint32(40, total * 2, true);
  let pos = 44;
  for (const c of chunks) {
    for (let i = 0; i < c.length; i++) {
      const s = clamp(c[i], -1, 1);
      dv.setInt16(pos, s < 0 ? Math.round(s * 32768) : Math.round(s * 32767), true);
      pos += 2;
    }
  }
  return uint8ToB64(new Uint8Array(buf));
}

// Feed raw 48 kHz Int16 PCM from native Rust WASAPI mic -> RNNoise -> 16 kHz -> Silero VAD
function onRawPcm(b64) {
  // Acoustic echo guard: never process mic while Edge TTS is speaking or whisper/LLM is busy
  if (state.mode === 'speaking' || state.mode === 'transcribing' || state.mode === 'thinking' || state.busy) return;
  if (state.mode !== 'listening' && state.mode !== 'standby') return;
  if (!state.dspReady || !rnnMod || !ortSession) return;

  const samples = b64ToInt16(b64);
  for (let i = 0; i < samples.length; i++) pcm48Queue.push(samples[i]);

  // Process exact 480-sample (10ms @ 48 kHz) frames through RNNoise WASM
  while (pcm48Queue.length >= 480) {
    const f32 = new Float32Array(480);
    for (let i = 0; i < 480; i++) f32[i] = pcm48Queue[i]; // RNNoise expects [-32768, 32767]
    pcm48Queue.splice(0, 480);

    const heapOff = rnnPtr >> 2;
    rnnMod.HEAPF32.set(f32, heapOff);
    rnnMod._rnnoise_process_frame(rnnCtx, rnnPtr, rnnPtr);
    const clean48 = rnnMod.HEAPF32.subarray(heapOff, heapOff + 480);

    // 3-tap low-pass decimation 48 kHz -> 16 kHz (160 samples per 10ms) + live energy meter
    let sq = 0;
    for (let k = 0; k < 160; k++) {
      const j = k * 3;
      const v = (clean48[j] + clean48[j + 1] + clean48[j + 2]) / (3 * 32768);
      pcm16Queue.push(v);
      sq += v * v;
    }
    const rms = Math.sqrt(sq / 160);
    state.energy = state.energy * 0.65 + clamp(rms * 14, 0, 1) * 0.35;
    if (state.active) updateOrbEnergy();
  }

  // Pack 512-sample (32 ms @ 16 kHz) frames for Silero VAD ONNX
  while (pcm16Queue.length >= 512) {
    const frame512 = new Float32Array(512);
    for (let i = 0; i < 512; i++) frame512[i] = pcm16Queue[i];
    pcm16Queue.splice(0, 512);
    vadQueue.push(frame512);
  }
  if (!vadProcessing && vadQueue.length) drainVadQueue();
}

async function runSileroFrame(frame512) {
  const ort = window.ort;
  const usesV5 = vadInputNames.includes('state');
  let feeds;
  if (usesV5) {
    // Official Silero VAD v5 expects 64-sample previous context + 512 samples = 576
    const withCtx = new Float32Array(576);
    withCtx.set(vadCtx, 0);
    withCtx.set(frame512, 64);
    vadCtx.set(frame512.subarray(512 - 64), 0);
    feeds = {
      input: new ort.Tensor('float32', withCtx, [1, 576]),
      state: new ort.Tensor('float32', vadState, [2, 1, 128]),
      sr: new ort.Tensor('int64', BigInt64Array.from([16000n]), []),
    };
  } else {
    feeds = {
      input: new ort.Tensor('float32', frame512, [1, 512]),
      sr: new ort.Tensor('int64', BigInt64Array.from([16000n]), [1]),
      h: new ort.Tensor('float32', vadState.subarray(0, 128), [2, 1, 64]),
      c: new ort.Tensor('float32', vadState.subarray(128, 256), [2, 1, 64]),
    };
  }
  const out = await ortSession.run(feeds);
  if (usesV5 && out.stateN) {
    vadState.set(out.stateN.data);
  } else if (out.hn && out.cn) {
    vadState.set(out.hn.data, 0);
    vadState.set(out.cn.data, 128);
  }
  return out.output ? Number(out.output.data[0]) || 0 : 0;
}

async function drainVadQueue() {
  vadProcessing = true;
  try {
    while (vadQueue.length) {
      if (state.mode !== 'listening' && state.mode !== 'standby') {
        vadQueue.length = 0;
        break;
      }
      const frame = vadQueue.shift();
      const prob = await runSileroFrame(frame);
      state.vadProb = prob;

      const isStandby = state.mode === 'standby';
      const endSilFrames = isStandby ? 18 : 28; // ~576ms standby / ~896ms conversation sentence end
      const maxFrames = isStandby ? 190 : 560;  // ~6s wake burst / ~18s full question

      if (!inSpeech) {
        preRoll.push(frame);
        if (preRoll.length > PREROLL_FRAMES) preRoll.shift();
        if (prob >= 0.55) {
          inSpeech = true;
          speechChunks = preRoll.slice();
          speechFrames = 1;
          silenceFrames = 0;
          if (!isStandby) {
            clearTimeout(state.listenTimeout);
            setPillText('Hearing you…');
          }
        }
      } else {
        speechChunks.push(frame);
        if (prob >= 0.35) {
          speechFrames++;
          silenceFrames = 0;
        } else {
          silenceFrames++;
        }
        if (silenceFrames >= endSilFrames || speechChunks.length >= maxFrames) {
          const captured = speechChunks;
          const valid = speechFrames >= 8; // at least ~256 ms of real speech
          resetVadEndpoint();
          vadQueue.length = 0;
          if (valid) {
            await handleFinishedUtterance(captured, isStandby);
          }
          break;
        }
      }
    }
  } catch (e) {
    state.dspError = String(e && e.message || e);
  } finally {
    vadProcessing = false;
  }
}

// ------------------------------------------------------------ STT -> Wake Word -> OpenRouter -> Edge TTS
async function handleFinishedUtterance(chunks, wasStandby) {
  state.busy = true;
  const wavB64 = encodeWav16k(chunks);
  try {
    if (!wasStandby) {
      setMode('transcribing', 'Transcribing…');
    }
    const stt = await window.pillvyn.aiTranscribe(wavB64);
    if (!stt || !stt.ok || !stt.text) {
      if (wasStandby) {
        state.busy = false;
        return;
      }
      setMode('listening', stt && stt.error ? stt.error : 'Didn’t catch that — try again');
      armListenTimeout();
      return;
    }

    const spoken = stt.text.trim();
    if (wasStandby) {
      if (!stt.wakeMatched) return; // Ignore background speech that didn't say "Hello PILLVYN"
      const pv = window.__pillvyn;
      if (pv) { pv.triggerSheen(); pv.chime('notif'); }
      const cmd = (stt.wakeCommand || '').trim();
      state.active = true;
      if (pv) { pv.st.holdUntil = Date.now() + 12000; pv.render(); }
      if (cmd.length >= 2) {
        await sendText(cmd, { fromVoice: true });
      } else {
        setMode('listening', 'Listening…');
        armListenTimeout();
      }
      return;
    }

    // Active listening mode: strip accidental repeated wake prefix if present
    const finalText = stt.wakeMatched && stt.wakeCommand ? stt.wakeCommand.trim() : spoken;
    if (!finalText) {
      setMode('listening', 'Listening…');
      armListenTimeout();
      return;
    }
    await sendText(finalText, { fromVoice: true });
  } finally {
    state.busy = false;
    syncMicWant();
  }
}

function liveClientContext() {
  const pv = window.__pillvyn;
  const d = new Date();
  const time = String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
  const date = $('#t-date') ? $('#t-date').textContent : d.toDateString();
  const m = pv && pv.st && pv.st.media;
  const media = m && m.active ? `${m.title || ''}${m.artist ? ' - ' + m.artist : ''}` : '';
  return { time, date, media };
}

async function sendText(raw, opts = {}) {
  const text = String(raw || '').trim();
  if (!text) return null;
  stopSpeaking();
  clearTimeout(state.listenTimeout);
  state.active = true;

  const pv = window.__pillvyn;
  if (pv) {
    pv.st.holdUntil = Date.now() + 20000;
    pv.render();
  }

  state.messages.push({ role: 'user', text, ts: Date.now() });
  if (state.messages.length > 40) state.messages.shift();
  setMode('thinking', text);
  notifyUi();

  const res = await window.pillvyn.aiChat(text, liveClientContext());
  if (!res || !res.ok) {
    const err = (res && res.error) || 'Could not reach OpenRouter';
    state.messages.push({ role: 'error', text: err, ts: Date.now() });
    setMode('idle', err);
    notifyUi();
    return res;
  }

  const reply = (res.reply || '').trim();
  state.messages.push({
    role: 'assistant',
    text: reply,
    tools: Array.isArray(res.tools) ? res.tools : [],
    lang: res.lang || 'en',
    ts: Date.now(),
  });
  if (state.messages.length > 40) state.messages.shift();
  setPillText(reply || 'Done');
  notifyUi();

  const autoSpeak = !pv || pv.st.settings.aiTtsAuto !== false;
  if (reply && (opts.speak || autoSpeak)) {
    await speak(reply);
  } else {
    setMode(wantStandby() ? 'standby' : 'idle', reply);
  }
  return res;
}

// ------------------------------------------------------------ Edge TTS Playback
function stopSpeaking() {
  if (state.audio) {
    try { state.audio.pause(); state.audio.src = ''; } catch { /* ignore */ }
    state.audio = null;
  }
  if (state.mode === 'speaking') {
    setMode(wantStandby() ? 'standby' : 'idle', state.pillText);
  }
  syncMicWant();
}

async function speak(text, opts = {}) {
  const clean = String(text || '').trim();
  if (!clean) return { ok: false, error: 'empty text' };
  stopSpeaking();
  setMode('speaking', clean);
  const pv = window.__pillvyn;
  const s = (pv && pv.st && pv.st.settings) || {};
  const vol = opts.volume !== undefined ? opts.volume : (Number.isFinite(Number(s.aiTtsVolume)) ? Number(s.aiTtsVolume) : 90);
  const rate = opts.rate !== undefined ? opts.rate : (Number.isFinite(Number(s.aiTtsRate)) ? Number(s.aiTtsRate) : 0);
  const voice = opts.voice || null;

  const res = await window.pillvyn.aiTts(clean, voice, rate, vol);
  if (!res || !res.ok || !res.audioB64) {
    setMode(wantStandby() ? 'standby' : 'idle', clean);
    return res || { ok: false, error: 'TTS failed' };
  }

  return new Promise((resolve) => {
    try {
      const audio = new Audio(`data:${res.mime || 'audio/mpeg'};base64,${res.audioB64}`);
      audio.volume = clamp(vol / 100, 0, 1);
      state.audio = audio;
      const done = () => {
        if (state.audio === audio) state.audio = null;
        if (state.mode === 'speaking') {
          setMode(wantStandby() ? 'standby' : 'idle', clean);
        }
        syncMicWant();
        resolve(res);
      };
      audio.onended = done;
      audio.onerror = done;
      const p = audio.play();
      if (p && p.catch) p.catch(done);
    } catch {
      setMode(wantStandby() ? 'standby' : 'idle', clean);
      resolve(res);
    }
  });
}

// ------------------------------------------------------------ Mode, Mic, and UI sync
function wantStandby() {
  const pv = window.__pillvyn;
  return !!(pv && pv.st && pv.st.settings && pv.st.settings.aiWakeWord && state.status.voiceReady);
}

function syncMicWant() {
  const needMic = (state.mode === 'listening' || state.mode === 'standby') && state.status.voiceReady;
  if (needMic && !state.dspReady && !state.dspLoading) {
    ensureDsp().then((ok) => {
      if (ok) window.pillvyn.aiSetMic(true);
    });
  } else {
    window.pillvyn.aiSetMic(needMic && state.dspReady);
  }
}

function armListenTimeout() {
  clearTimeout(state.listenTimeout);
  state.listenTimeout = setTimeout(() => {
    if (state.mode === 'listening' && !inSpeech) {
      setMode(wantStandby() ? 'standby' : 'idle', 'Say “Hello PILLVYN” or click Mic');
      syncMicWant();
    }
  }, 10000);
}

function setPillText(txt) {
  state.pillText = txt || '';
  const el = $('#ai-pill-text');
  if (el && el.firstElementChild) {
    el.firstElementChild.textContent = state.pillText;
  }
}

function setMode(next, text) {
  state.mode = next;
  if (text !== undefined) setPillText(text);
  const orb = $('#ai-orb');
  const lbl = $('#ai-pill-state');
  const stopBtn = $('#ai-pill-stop');
  const micBtn = $('#ai-pill-mic');
  if (orb) orb.className = next;
  if (lbl) {
    lbl.textContent =
      next === 'listening' ? 'Listening'
      : next === 'transcribing' ? 'Whisper'
      : next === 'thinking' ? 'Thinking'
      : next === 'speaking' ? 'Speaking'
      : 'PILLVYN AI';
  }
  if (stopBtn) stopBtn.classList.toggle('hidden', next !== 'speaking');
  if (micBtn) micBtn.classList.toggle('active', next === 'listening');
  const tBtn = $('#t-ai');
  if (tBtn) tBtn.classList.toggle('ai-live', next === 'standby' || next === 'listening' || next === 'speaking');
  syncMicWant();
  notifyUi();
}

function updateOrbEnergy() {
  const orb = $('#ai-orb');
  if (!orb) return;
  const sc = (1 + state.energy * 0.45).toFixed(3);
  orb.style.setProperty('--e', sc);
}

function notifyUi() {
  const pv = window.__pillvyn;
  if (!pv) return;
  pv.render();
  if (pv.st.stackOpen && (pv.st.stackKind === 'ai' || (pv.st.stackKind === 'settings' && pv.st.settingsTab === 'ai'))) {
    if (pv.refreshAiView) pv.refreshAiView();
  }
}

async function startListening() {
  stopSpeaking();
  await refreshStatus();
  state.active = true;
  const pv = window.__pillvyn;
  if (pv) {
    pv.st.holdUntil = Date.now() + 15000;
    pv.render();
  }
  if (!state.status.voiceReady) {
    setMode('idle', 'Set up local voice engine to speak');
    if (pv) pv.setStack(true, 'ai');
    return;
  }
  const ok = await ensureDsp();
  if (!ok) {
    setMode('idle', state.dspError || 'DSP engine failed to load');
    return;
  }
  resetVadEndpoint();
  setMode('listening', 'Listening… speak Persian or English');
  if (pv) { pv.triggerSheen(); pv.chime('tick'); }
  armListenTimeout();
}

function toggleMic() {
  if (state.mode === 'listening') {
    clearTimeout(state.listenTimeout);
    setMode(wantStandby() ? 'standby' : 'idle', 'Paused listening');
  } else {
    startListening();
  }
}

function openPanel(listenNow) {
  state.active = true;
  const pv = window.__pillvyn;
  if (pv) {
    pv.st.holdUntil = Date.now() + 20000;
    pv.setStack(true, 'ai');
    pv.render();
  }
  refreshStatus().then(() => {
    if (listenNow && state.status.voiceReady) startListening();
  });
}

function closeAll() {
  clearTimeout(state.listenTimeout);
  stopSpeaking();
  state.active = false;
  setMode(wantStandby() ? 'standby' : 'idle', 'Say “Hello PILLVYN” or ask anything…');
  const pv = window.__pillvyn;
  if (pv) {
    if (pv.st.stackOpen && pv.st.stackKind === 'ai') pv.setStack(false);
    pv.render();
  }
}

function clearChat() {
  state.messages = [];
  window.pillvyn.aiClear();
  setPillText('Conversation cleared');
  notifyUi();
}

async function refreshStatus() {
  const s = await window.pillvyn.aiStatus();
  if (s && typeof s === 'object') {
    state.status = { ...state.status, ...s };
    if (wantStandby() && state.mode === 'idle') {
      setMode('standby', state.pillText);
    } else if (!wantStandby() && state.mode === 'standby') {
      setMode('idle', state.pillText);
    }
    notifyUi();
  }
  return state.status;
}

async function install(modelId) {
  state.install = { step: 'Starting download…', pct: 2, done: false, error: null };
  notifyUi();
  return window.pillvyn.aiInstall(modelId || state.status.modelId || 'base');
}

// ------------------------------------------------------------ Wire Pill Controls & Backend Events
$('#ai-pill-mic').addEventListener('click', toggleMic);
$('#ai-pill-stop').addEventListener('click', stopSpeaking);
$('#ai-pill-chat').addEventListener('click', () => {
  const pv = window.__pillvyn;
  if (!pv) return;
  pv.st.holdUntil = Date.now() + 15000;
  pv.setStack(!(pv.st.stackOpen && pv.st.stackKind === 'ai'), 'ai');
});
$('#ai-pill-close').addEventListener('click', closeAll);

window.pillvyn.onAiPcm(onRawPcm);
window.pillvyn.onAiMic((m) => {
  state.micActive = !!(m && m.active);
  state.micError = (m && m.error) || '';
  if (state.micError && state.mode === 'listening') {
    setMode('idle', 'Mic error: ' + state.micError);
  }
  notifyUi();
});
window.pillvyn.onAiInstall((p) => {
  state.install = p;
  if (p && p.done) {
    refreshStatus();
  } else {
    notifyUi();
  }
});
window.pillvyn.onAiStatus((s) => {
  if (s && typeof s === 'object') {
    state.status = { ...state.status, ...s };
    if (wantStandby() && state.mode === 'idle') setMode('standby', state.pillText);
    notifyUi();
  }
});
window.pillvyn.onAiAction((act) => {
  const pv = window.__pillvyn;
  if (!pv || !act) return;
  if (act.type === 'set_timer') {
    const mins = clamp(Number(act.minutes) || 5, 0.5, 180);
    pv.timer.left = Math.round(mins * 60000);
    pv.timer.total = pv.timer.left;
    pv.timer.done = false;
    pv.timer.endAt = Date.now() + pv.timer.left;
    pv.timer.running = true;
    pv.tick();
    pv.render();
  } else if (act.type === 'control_timer') {
    if (act.action === 'pause' && pv.timer.running) {
      pv.timer.left = Math.max(0, pv.timer.endAt - Date.now());
      pv.timer.running = false;
    } else if (act.action === 'resume' && !pv.timer.running) {
      if (pv.timer.left <= 0) pv.timer.left = 25 * 60000;
      pv.timer.endAt = Date.now() + pv.timer.left;
      pv.timer.running = true;
    } else if (act.action === 'reset') {
      pv.timer.running = false;
      pv.timer.done = false;
      pv.timer.left = 25 * 60000;
      pv.timer.total = pv.timer.left;
    }
    pv.tick();
    pv.render();
  }
});

// Boot status sync
setTimeout(() => { refreshStatus(); }, 400);

window.pillvynAi = {
  state,
  refreshStatus,
  ensureDsp,
  install,
  openPanel,
  closeAll,
  startListening,
  toggleMic,
  sendText,
  speak,
  stopSpeaking,
  clearChat,
  syncStandby() {
    if (wantStandby() && state.mode === 'idle') setMode('standby', state.pillText);
    else if (!wantStandby() && state.mode === 'standby') setMode('idle', state.pillText);
    syncMicWant();
  },
  encodeWav16k,
};
})();
