/* PILLVYN v4.0.0 Ultra Voice AI Engine:
 * - Zero-Wait Mic Pipeline: Software AGC + RNNoise WASM + Silero VAD ONNX (with instant RMS fallback VAD!)
 * - Broadcast-Clean Audio Trimming & Peak Normalization before STT for top Persian & English accuracy
 * - Hybrid STT: Local hot whisper.cpp (Small-Q5 / Base) + instant GapGPT Cloud fallback
 * - Real-Time SSE Streaming from GapGPT (`gapgpt-gemma-26b-a4b`, fast `reasoning_effort=none`)
 * - Full Dynamic Island coordination (32 tools: Windows + Panels + Settings + Shelf + Notes + Notifications + Vault)
 * - Streaming sentence-by-sentence Microsoft Edge Neural TTS with concurrent prefetching & barge-in
 */
(() => {
'use strict';

const $ = (s) => document.querySelector(s);
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

const state = {
  status: {
    rnnoise: false, silero: false, whisperBin: false, whisperModel: false,
    dspReady: false, localReady: false, localWhisperReady: false, cloudSttReady: true,
    voiceReady: true, hasKey: true, customKey: false, keyMasked: 'Built-in GapGPT (Active)',
    installing: false, assetUrl: '', model: 'gapgpt-gemma-26b-a4b', modelId: 'small-q5',
    sttEngine: 'auto', fullAccess: true, reasoning: 'fast', sttLang: 'auto',
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
  agcGain: 1,
  activeTool: '',         // live tool name while Gemma 4 executes tools
  streamingText: '',      // live SSE token buffer while Gemma 4 streams response
  pillText: 'Say “Hello PILLVYN” or ask anything…',
  messages: [],           // [{ role: 'user'|'assistant'|'error', text, tools?, lang?, latencyMs?, ts }]
  audio: null,
  listenTimeout: 0,
  busy: false,
};

// ------------------------------------------------------------ DSP: AGC + RNNoise WASM + Silero VAD ONNX (+ Instant Fallback VAD)
let rnnMod = null, rnnCtx = 0, rnnPtr = 0;
let ortSession = null, vadState = null, vadCtx = new Float32Array(64);
let vadInputNames = [];
let pcm48Queue = [];
let pcm16Queue = [];
let vadQueue = [];
let vadProcessing = false;
let agcPeak = 4800; // Smoothed mic peak tracker (Int16 scale)
let noiseFloor = 0.015; // Adaptive RMS floor for instant pure-JS fallback VAD
let speakGen = 0;   // Cancellation token for streaming sentence-by-sentence TTS

// Utterance endpointing buffers (16 kHz mono float32)
const PREROLL_FRAMES = 16;       // 16 * 32 ms = 512 ms pre-roll so "Hello" / "سلام" is never clipped
const VAD_START_THRESH = 0.40;   // Responsive speech onset after RNNoise denoising
const VAD_CONT_THRESH = 0.24;    // Smooth speech continuation through soft syllables
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

let dspPromise = null;

function ensureDsp() {
  if (state.dspReady) return Promise.resolve(true);
  if (dspPromise) return dspPromise;
  const base = state.status.assetUrl;
  if (!base || !state.status.rnnoise || !state.status.silero) return Promise.resolve(false);

  state.dspLoading = true;
  state.dspError = '';
  dspPromise = (async () => {
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
      dspPromise = null;
    }
  })();
  return dspPromise;
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

// Trim excess silence (preserving 160 ms clean padding) & peak-normalize to -2.8 dBFS (0.72)
// so Whisper receives crisp, broadcast-level audio regardless of microphone distance.
function prepareSpeechChunks(chunks) {
  if (!chunks || !chunks.length) return [];
  let first = 0, last = chunks.length - 1;
  const rmsOf = (arr) => {
    let s = 0;
    for (let i = 0; i < arr.length; i++) s += arr[i] * arr[i];
    return Math.sqrt(s / (arr.length || 1));
  };
  while (first < last && rmsOf(chunks[first]) < 0.012) first++;
  while (last > first && rmsOf(chunks[last]) < 0.012) last--;
  const pad = 5; // 5 * 32ms = 160ms padding so initial/final consonants stay intact
  const slice = chunks.slice(Math.max(0, first - pad), Math.min(chunks.length, last + pad + 1));

  let peak = 0;
  for (const c of slice) {
    for (let i = 0; i < c.length; i++) {
      const a = Math.abs(c[i]);
      if (a > peak) peak = a;
    }
  }
  if (peak < 0.008) return slice;
  const normGain = clamp(0.72 / peak, 0.8, 4.0);
  return slice.map((c) => {
    const out = new Float32Array(c.length);
    for (let i = 0; i < c.length; i++) out[i] = clamp(c[i] * normGain, -0.98, 0.98);
    return out;
  });
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

function updateLiveMeters() {
  const micFill = $('#ai-mic-meter-fill');
  if (micFill) micFill.style.width = Math.round(clamp(state.energy * 100, 0, 100)) + '%';
  const vadFill = $('#ai-vad-meter-fill');
  if (vadFill) {
    const pct = Math.round(clamp(state.vadProb * 100, 0, 100));
    vadFill.style.width = pct + '%';
    vadFill.classList.toggle('hot', state.vadProb >= VAD_START_THRESH);
  }
  const stEl = $('#ai-meter-state');
  if (stEl) {
    stEl.textContent = !state.micActive
      ? 'Mic idle'
      : state.vadProb >= VAD_START_THRESH
        ? `Speech (${Math.round(state.vadProb * 100)}%)`
        : state.mode === 'standby'
          ? `Wake ready (${state.agcGain.toFixed(1)}×${state.dspReady ? '' : ' · RMS'})`
          : `Listening (${Math.round(state.vadProb * 100)}%)`;
  }
}

// Feed raw 48 kHz Int16 PCM from native Rust WASAPI mic -> AGC -> (RNNoise if ready) -> 16 kHz -> (Silero VAD or instant RMS VAD)
function onRawPcm(b64) {
  // Acoustic echo guard: never process mic while Edge TTS is speaking or whisper/LLM is busy
  if (state.mode === 'speaking' || state.mode === 'transcribing' || state.mode === 'thinking' || state.busy) return;
  if (state.mode !== 'listening' && state.mode !== 'standby') return;

  const samples = b64ToInt16(b64);
  for (let i = 0; i < samples.length; i++) pcm48Queue.push(samples[i]);

  // Process exact 480-sample (10ms @ 48 kHz) frames through AGC + optional RNNoise WASM
  while (pcm48Queue.length >= 480) {
    let framePeak = 0;
    for (let i = 0; i < 480; i++) {
      const a = Math.abs(pcm48Queue[i]);
      if (a > framePeak) framePeak = a;
    }
    if (framePeak > agcPeak) agcPeak = agcPeak * 0.65 + framePeak * 0.35;
    else agcPeak = agcPeak * 0.988 + framePeak * 0.012;
    const gain = clamp(12000 / Math.max(agcPeak, 2800), 1.0, 3.5);
    state.agcGain = gain;

    const f32 = new Float32Array(480);
    for (let i = 0; i < 480; i++) {
      f32[i] = clamp(pcm48Queue[i] * gain, -32700, 32700);
    }
    pcm48Queue.splice(0, 480);

    let clean48 = f32;
    if (state.dspReady && rnnMod) {
      const heapOff = rnnPtr >> 2;
      rnnMod.HEAPF32.set(f32, heapOff);
      rnnMod._rnnoise_process_frame(rnnCtx, rnnPtr, rnnPtr);
      clean48 = rnnMod.HEAPF32.subarray(heapOff, heapOff + 480);
    }

    // 3-tap low-pass decimation 48 kHz -> 16 kHz (160 samples per 10ms) + live energy meter
    let sq = 0;
    for (let k = 0; k < 160; k++) {
      const j = k * 3;
      const v = (clean48[j] + clean48[j + 1] + clean48[j + 2]) / (3 * 32768);
      pcm16Queue.push(v);
      sq += v * v;
    }
    const rms = Math.sqrt(sq / 160);
    state.energy = state.energy * 0.60 + clamp(rms * 14, 0, 1) * 0.40;
    if (state.active) updateOrbEnergy();
    updateLiveMeters();
  }

  // Pack 512-sample (32 ms @ 16 kHz) frames for Silero VAD ONNX (or instant adaptive RMS VAD fallback)
  while (pcm16Queue.length >= 512) {
    const frame512 = new Float32Array(512);
    for (let i = 0; i < 512; i++) frame512[i] = pcm16Queue[i];
    pcm16Queue.splice(0, 512);
    vadQueue.push(frame512);
  }
  if (!vadProcessing && vadQueue.length) drainVadQueue();
}

async function runSileroFrame(frame512) {
  if (!state.dspReady || !ortSession) {
    // Pure-JS adaptive RMS energy fallback VAD so Mic works immediately on fresh installs
    let sq = 0;
    for (let i = 0; i < frame512.length; i++) sq += frame512[i] * frame512[i];
    const rms = Math.sqrt(sq / frame512.length);
    if (rms < noiseFloor * 1.4) noiseFloor = noiseFloor * 0.96 + Math.max(0.006, rms) * 0.04;
    const ratio = rms / Math.max(noiseFloor, 0.012);
    return clamp((ratio - 1.4) / 3.2, 0, 1);
  }
  const ort = window.ort;
  const usesV5 = vadInputNames.includes('state');
  let feeds;
  if (usesV5) {
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
      updateLiveMeters();

      const isStandby = state.mode === 'standby';
      const endSilFrames = isStandby ? 14 : 22; // ~448 ms snappy wake / ~704 ms full sentence end
      const maxFrames = isStandby ? 210 : 620;  // ~6.7s wake + command / ~20s full prompt

      if (!inSpeech) {
        preRoll.push(frame);
        if (preRoll.length > PREROLL_FRAMES) preRoll.shift();
        if (prob >= VAD_START_THRESH) {
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
        if (prob >= VAD_CONT_THRESH) {
          speechFrames++;
          silenceFrames = 0;
        } else {
          silenceFrames++;
        }
        if (silenceFrames >= endSilFrames || speechChunks.length >= maxFrames) {
          const captured = speechChunks;
          const valid = speechFrames >= 6; // at least ~192 ms of real voiced speech
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

// ------------------------------------------------------------ STT -> Wake Word -> GapGPT Gemma 4 (SSE) -> Edge TTS
async function handleFinishedUtterance(chunks, wasStandby) {
  state.busy = true;
  const prepared = prepareSpeechChunks(chunks);
  const wavB64 = encodeWav16k(prepared);
  try {
    if (!wasStandby) {
      setMode('transcribing', 'Transcribing…');
    }
    const stt = await window.pillvyn.aiTranscribe(wavB64);
    if (!stt || !stt.ok || !stt.text) {
      if (wasStandby) return;
      setMode('listening', stt && stt.error ? stt.error : 'Didn’t catch that — try again');
      armListenTimeout(8000);
      return;
    }

    const spoken = stt.text.trim();
    if (wasStandby) {
      if (!stt.wakeMatched) return; // Ignore background speech that didn't call PILLVYN
      const pv = window.__pillvyn;
      if (pv) { pv.triggerSheen(); pv.chime('notif'); }
      const cmd = (stt.wakeCommand || '').trim();
      state.active = true;
      if (pv) {
        pv.st.holdUntil = Date.now() + 20000;
        pv.setStack(true, 'ai');
        pv.render();
      }
      if (cmd.length >= 2) {
        await sendText(cmd, { fromVoice: true });
      } else {
        setMode('listening', 'Yes? Speak Persian or English…');
        armListenTimeout(10000);
      }
      return;
    }

    const finalText = stt.wakeMatched && stt.wakeCommand ? stt.wakeCommand.trim() : spoken;
    if (!finalText) {
      setMode('listening', 'Listening…');
      armListenTimeout(9000);
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
  const media = m && m.active ? `${m.title || ''}${m.artist ? ' - ' + m.artist : ''}${m.playing ? ' (playing)' : ' (paused)'}` : '';
  let timer = 'idle';
  if (pv && pv.timer) {
    const leftMs = pv.timer.running ? Math.max(0, pv.timer.endAt - Date.now()) : pv.timer.left;
    const mins = Math.floor(leftMs / 60000), secs = Math.floor((leftMs % 60000) / 1000);
    if (pv.timer.running) timer = `running (${mins}m ${secs}s left)`;
    else if (pv.timer.done) timer = 'finished';
  }
  const n = pv && pv.st && pv.st.net;
  const net = n ? (!n.online ? 'offline' : n.vpn ? 'VPN: ' + n.vpn : n.proxy ? 'proxy' : 'online') : 'online';
  const hist = (pv && typeof pv.recentNotifs === 'function') ? pv.recentNotifs() : '';
  return { time, date, media, timer, net, notifs: hist };
}

async function sendText(raw, opts = {}) {
  const text = String(raw || '').trim();
  if (!text) return null;
  stopSpeaking();
  clearTimeout(state.listenTimeout);
  state.active = true;
  state.activeTool = '';
  state.streamingText = '';

  const pv = window.__pillvyn;
  if (pv) {
    pv.st.holdUntil = Date.now() + 28000;
    pv.render();
  }

  state.messages.push({ role: 'user', text, ts: Date.now() });
  if (state.messages.length > 48) state.messages.shift();
  setMode('thinking', text);
  notifyUi();

  const t0 = performance.now();
  const res = await window.pillvyn.aiChat(text, liveClientContext());
  const latencyMs = Number(res && res.ms) || Math.round(performance.now() - t0);
  state.activeTool = '';
  state.streamingText = '';

  if (!res || !res.ok) {
    const err = (res && res.error) || 'Could not reach GapGPT';
    state.messages.push({ role: 'error', text: err, ts: Date.now() });
    setMode(wantStandby() ? 'standby' : 'idle', err);
    notifyUi();
    return res;
  }

  const reply = (res.reply || '').trim();
  state.messages.push({
    role: 'assistant',
    text: reply,
    tools: Array.isArray(res.tools) ? res.tools : [],
    lang: res.lang || 'en',
    latencyMs,
    ts: Date.now(),
  });
  if (state.messages.length > 48) state.messages.shift();
  setPillText(reply || 'Done');
  notifyUi();

  const autoSpeak = !pv || pv.st.settings.aiTtsAuto !== false;
  let spokeCleanly = false;
  if (reply && (opts.speak || autoSpeak)) {
    const tRes = await speak(reply);
    spokeCleanly = !!(tRes && tRes.ok && !tRes.cancelled);
  } else {
    setMode(wantStandby() ? 'standby' : 'idle', reply);
  }

  // Natural conversational follow-up: after answering a voice request, stay listening for 7s
  if (opts.fromVoice && spokeCleanly && state.active && state.status.voiceReady) {
    resetVadEndpoint();
    setMode('listening', 'Listening for follow-up…');
    armListenTimeout(7000);
  }
  return res;
}

// ------------------------------------------------------------ Sentence-by-Sentence Streaming Edge TTS
function splitSentences(raw) {
  const cleaned = String(raw || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/https?:\/\/\S+/g, ' ')
    .replace(/[*_`~#>•▪◦]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  if (!cleaned) return [];

  const isFa = (s) => /[\u0600-\u06FF]/.test(s);
  const parts = cleaned.split(/(?<=[.!?؟])\s+|\n+/).map((s) => s.trim()).filter(Boolean);
  const chunks = [];
  for (const p of parts) {
    const prev = chunks.length ? chunks[chunks.length - 1] : '';
    const sameScript = chunks.length && (isFa(prev) === isFa(p));
    if (sameScript && (prev.length < 24 || p.length < 22) && prev.length + p.length < 240) {
      chunks[chunks.length - 1] += ' ' + p;
    } else if (p.length > 260) {
      const subs = p.split(/(?<=[,،;؛:])\s+/);
      let acc = '';
      for (const sub of subs) {
        if (acc && acc.length + sub.length > 220) { chunks.push(acc.trim()); acc = sub; }
        else acc = acc ? acc + ' ' + sub : sub;
      }
      if (acc.trim()) chunks.push(acc.trim());
    } else {
      chunks.push(p);
    }
  }
  return chunks.length ? chunks : [cleaned];
}

function stopSpeaking() {
  speakGen++;
  if (state.audio) {
    try {
      state.audio.onended = null;
      state.audio.onerror = null;
      state.audio.pause();
      state.audio.src = '';
    } catch { /* ignore */ }
    state.audio = null;
  }
  if (state.mode === 'speaking') {
    setMode(wantStandby() ? 'standby' : 'idle', state.pillText);
  }
  syncMicWant();
}

function playB64Audio(audioB64, mime, vol, myGen) {
  return new Promise((resolve) => {
    if (myGen !== speakGen) { resolve(false); return; }
    try {
      const audio = new Audio(`data:${mime || 'audio/mpeg'};base64,${audioB64}`);
      audio.volume = clamp(vol / 100, 0, 1);
      state.audio = audio;
      const done = (ok) => {
        if (state.audio === audio) state.audio = null;
        resolve(ok && myGen === speakGen);
      };
      audio.onended = () => done(true);
      audio.onerror = () => done(false);
      const p = audio.play();
      if (p && p.catch) p.catch(() => done(false));
    } catch {
      resolve(false);
    }
  });
}

async function speak(text, opts = {}) {
  const clean = String(text || '').trim();
  if (!clean) return { ok: false, error: 'empty text' };
  stopSpeaking();
  const myGen = speakGen;
  setMode('speaking', clean);

  const pv = window.__pillvyn;
  const s = (pv && pv.st && pv.st.settings) || {};
  const vol = opts.volume !== undefined ? opts.volume : (Number.isFinite(Number(s.aiTtsVolume)) ? Number(s.aiTtsVolume) : 90);
  const rate = opts.rate !== undefined ? opts.rate : (Number.isFinite(Number(s.aiTtsRate)) ? Number(s.aiTtsRate) : 8);
  const voice = opts.voice || null;

  const chunks = opts.single ? [clean] : splitSentences(clean);
  if (!chunks.length) {
    setMode(wantStandby() ? 'standby' : 'idle', clean);
    return { ok: false, error: 'empty text' };
  }

  let firstRes = null;
  let nextPromise = window.pillvyn.aiTts(chunks[0], voice, rate, vol);

  for (let i = 0; i < chunks.length; i++) {
    if (myGen !== speakGen) return { ok: false, cancelled: true };
    const curRes = await nextPromise;
    if (myGen !== speakGen) return { ok: false, cancelled: true };

    if (i + 1 < chunks.length) {
      nextPromise = window.pillvyn.aiTts(chunks[i + 1], voice, rate, vol);
    }
    if (!firstRes) firstRes = curRes;
    if (!curRes || !curRes.ok || !curRes.audioB64) continue;

    await playB64Audio(curRes.audioB64, curRes.mime, vol, myGen);
  }

  if (myGen === speakGen && state.mode === 'speaking') {
    setMode(wantStandby() ? 'standby' : 'idle', clean);
  }
  syncMicWant();
  if (!firstRes) return { ok: false, error: 'TTS failed' };
  return { ...firstRes, chunks: chunks.length, cancelled: myGen !== speakGen };
}

// ------------------------------------------------------------ Mode, Mic, and UI sync
function wantStandby() {
  const pv = window.__pillvyn;
  const wakeOn = !pv || !pv.st || !pv.st.settings ? true : pv.st.settings.aiWakeWord !== false;
  // Standby wake word runs continuously when neural DSP (RNNoise + Silero) is installed
  return !!(wakeOn && state.status.rnnoise && state.status.silero);
}

function syncMicWant() {
  const wantListen = state.mode === 'listening' && state.status.voiceReady;
  const wantWake = state.mode === 'standby' && state.status.rnnoise && state.status.silero;
  const needMic = wantListen || wantWake;
  if (needMic && state.status.rnnoise && state.status.silero && !state.dspReady && !state.dspLoading) {
    ensureDsp().then(() => {
      if (state.mode === 'listening' || state.mode === 'standby') {
        window.pillvyn.aiSetMic(true);
      }
    });
    if (wantListen) window.pillvyn.aiSetMic(true); // instant RMS VAD fallback while DSP initializes
  } else {
    window.pillvyn.aiSetMic(needMic && (state.dspReady || wantListen));
  }
}

function armListenTimeout(ms = 10000) {
  clearTimeout(state.listenTimeout);
  state.listenTimeout = setTimeout(() => {
    if (state.mode === 'listening' && !inSpeech) {
      setMode(wantStandby() ? 'standby' : 'idle', 'Say “Hello PILLVYN” or click Mic');
      syncMicWant();
    }
  }, ms);
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
  const wave = $('#ai-wave');
  const lbl = $('#ai-pill-state');
  const stopBtn = $('#ai-pill-stop');
  const micBtn = $('#ai-pill-mic');
  const wakeDot = $('#i-wake');
  if (orb) orb.className = next;
  if (wave) wave.className = 'ai-wave ' + next;
  if (lbl) {
    lbl.textContent =
      next === 'listening' ? 'Listening'
      : next === 'transcribing' ? 'Voice'
      : next === 'thinking' ? (state.activeTool ? 'Action' : state.streamingText ? 'Live' : 'Gemma 4')
      : next === 'speaking' ? 'Speaking'
      : next === 'standby' ? 'Wake Ready'
      : 'PILLVYN AI';
  }
  if (stopBtn) stopBtn.classList.toggle('hidden', next !== 'speaking');
  if (micBtn) micBtn.classList.toggle('active', next === 'listening');
  if (wakeDot) wakeDot.classList.toggle('on', next === 'standby' || next === 'listening');
  const tBtn = $('#t-ai');
  if (tBtn) tBtn.classList.toggle('ai-live', next === 'standby' || next === 'listening' || next === 'speaking');
  syncMicWant();
  notifyUi();
}

function updateOrbEnergy() {
  const orb = $('#ai-orb');
  const sc = (1 + state.energy * 0.48).toFixed(3);
  if (orb) orb.style.setProperty('--e', sc);
  const wave = $('#ai-wave');
  if (wave) wave.style.setProperty('--lvl', clamp(state.energy, 0.08, 1).toFixed(3));
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
    pv.st.holdUntil = Date.now() + 20000;
    pv.render();
  }
  // Load neural RNNoise + Silero DSP if installed; otherwise instant RMS fallback VAD still captures speech!
  if (state.status.rnnoise && state.status.silero) {
    await ensureDsp();
  }
  resetVadEndpoint();
  setMode('listening', 'Listening… speak Persian or English');
  if (pv) { pv.triggerSheen(); pv.chime('tick'); }
  armListenTimeout(10000);
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
    pv.st.holdUntil = Date.now() + 24000;
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
  state.streamingText = '';
  setMode(wantStandby() ? 'standby' : 'idle', 'Say “Hello PILLVYN” or ask anything…');
  const pv = window.__pillvyn;
  if (pv) {
    if (pv.st.stackOpen && pv.st.stackKind === 'ai') pv.setStack(false);
    pv.render();
  }
}

function clearChat() {
  state.messages = [];
  state.streamingText = '';
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
  return window.pillvyn.aiInstall(modelId || state.status.modelId || 'small-q5');
}

// ------------------------------------------------------------ Wire Pill Controls & Backend Events
$('#ai-pill-mic').addEventListener('click', toggleMic);
$('#ai-pill-stop').addEventListener('click', stopSpeaking);
$('#ai-pill-chat').addEventListener('click', () => {
  const pv = window.__pillvyn;
  if (!pv) return;
  pv.st.holdUntil = Date.now() + 20000;
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
  updateLiveMeters();
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
window.pillvyn.onAiTool((t) => {
  if (!t || !t.name) return;
  if (t.status === 'running') {
    state.activeTool = t.name;
    if (state.mode === 'thinking') {
      setPillText('Running ' + t.name + '…');
      const lbl = $('#ai-pill-state');
      if (lbl) lbl.textContent = 'Action';
    }
    notifyUi();
  }
});
window.pillvyn.onAiStream((ev) => {
  if (!ev || typeof ev.text !== 'string') return;
  state.streamingText = ev.text;
  state.activeTool = '';
  const preview = ev.text.replace(/\s+/g, ' ').trim();
  if (preview) setPillText(preview);
  const lbl = $('#ai-pill-state');
  if (lbl && state.mode === 'thinking') lbl.textContent = 'Live';
  const liveEl = $('#ai-stream-txt');
  if (liveEl) {
    liveEl.textContent = ev.text;
    const list = $('#stack-list');
    if (list) list.scrollTop = list.scrollHeight;
  } else {
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
  } else if (act.type === 'ui') {
    if (act.hudTitle && typeof pv.showHud === 'function') {
      pv.showHud('ai', act.hudTitle, act.hudText || '', 3000, 'pop');
    }
    if (act.settingsTab) {
      pv.st.settingsTab = act.settingsTab;
      pv.st.holdUntil = Date.now() + 12000;
      pv.setStack(true, 'settings');
    } else if (act.panel === 'close') {
      pv.setStack(false);
    } else if (act.panel === 'timer') {
      pv.st.manual = 'timer';
      pv.st.holdUntil = Date.now() + 10000;
      pv.render();
    } else if (act.panel) {
      const mapped = act.panel === 'inbox' ? 'notifs' : act.panel === 'shelf' ? 'files' : act.panel;
      pv.st.holdUntil = Date.now() + 12000;
      pv.setStack(true, mapped);
    }
  } else if (act.type === 'clear_notifs' && typeof pv.clearNotifs === 'function') {
    pv.clearNotifs();
  }
});

// Boot status sync -> immediately arms RNNoise + Silero VAD wake-word standby if ready
setTimeout(() => { refreshStatus(); }, 450);

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
  splitSentences,
  prepareSpeechChunks,
  syncStandby() {
    if (wantStandby() && state.mode === 'idle') setMode('standby', state.pillText);
    else if (!wantStandby() && state.mode === 'standby') setMode('idle', state.pillText);
    syncMicWant();
  },
  encodeWav16k,
};
})();
