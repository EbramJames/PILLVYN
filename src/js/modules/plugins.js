import { el, icon } from '../utils.js';

export function renderPlugins(store, api) {
  const snap = store.plugins;
  const plugins = snap?.plugins || [];

  const root = el('div', { class: 'stack' }, [
    el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '6px' } }, [
      el('span', { class: 'muted', style: { fontSize: '10.5px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }, text: snap?.plugins_dir || 'Local Plugins' }),
      el('div', { style: { display: 'flex', gap: '5px', flexShrink: '0' } }, [
        el('button', { class: 'pill-btn', onClick: () => api.pluginAction('create_sample') }, 'Add Sample'),
        el('button', { class: 'pill-btn', onClick: () => api.pluginAction('refresh') }, 'Rescan'),
      ]),
    ]),
  ]);

  if (plugins.length === 0) {
    root.appendChild(
      el('div', { class: 'empty-state' }, [
        icon('plug', 22),
        el('div', { style: { marginTop: '6px', fontWeight: '600', color: 'var(--text-primary)' }, text: 'No Plugins Installed' }),
        el('div', { class: 'muted', style: { marginTop: '3px' }, text: 'Click "Add Sample" to create a valid manifest plugin in %APPDATA%\\PILLVYN\\plugins, or drop custom plugin.json folders there.' }),
      ])
    );
    return root;
  }

  for (const p of plugins) {
    const m = p.manifest;
    root.appendChild(
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-title-row' }, [
          el('span', { style: { fontWeight: '600', fontSize: '12.5px' }, text: `${m.name} v${m.version}` }),
          el('button', {
            class: `pill-btn ${m.enabled ? 'primary' : ''}`,
            style: { padding: '2px 9px', fontSize: '10.5px' },
            onClick: () => api.pluginAction('toggle', m.id),
          }, m.enabled ? 'Enabled' : 'Disabled'),
        ]),
        el('div', { class: 'muted', style: { fontSize: '11px' }, text: `${m.description} · by ${m.author}` }),
        m.enabled && m.actions && m.actions.length > 0
          ? el('div', { style: { display: 'flex', gap: '6px', marginTop: '8px', flexWrap: 'wrap' } }, m.actions.map((act) =>
              el('button', {
                class: 'pill-btn',
                onClick: () => {
                  if (act.kind === 'url') api.openUrl(act.target);
                  else api.pluginAction('trigger_activity', m.name, act.target);
                },
              }, act.label)
            ))
          : null,
      ])
    );
  }

  return root;
}

let aiDraftPrompt = '';

export function renderAi(store, api) {
  const aiCfg = store.config?.ai;
  if (!aiCfg?.enabled) {
    return el('div', { class: 'empty-state' }, [
      icon('sparkles', 22),
      el('div', { style: { marginTop: '6px', fontWeight: '600', color: 'var(--text-primary)' }, text: 'Optional AI Assistant Disabled' }),
      el('div', { class: 'muted', style: { marginTop: '4px' }, text: 'Enable local Ollama or a custom OpenAI-compatible endpoint in Modules & Tools inside Settings Studio.' }),
      el('button', {
        type: 'button',
        class: 'pill-btn primary',
        style: { marginTop: '10px' },
        onClick: () => api.openSettings(),
      }, 'Open Settings Studio'),
    ]);
  }

  const promptBox = el('textarea', {
    class: 'input-glass',
    rows: '2',
    placeholder: `Ask ${aiCfg.model} via ${aiCfg.provider}…`,
  });
  promptBox.value = aiDraftPrompt;

  const submitBtn = el('button', {
    type: 'button',
    class: 'pill-btn primary',
    disabled: store.aiLoading || !aiDraftPrompt.trim(),
    onClick: () => api.askAi(promptBox.value),
  }, store.aiLoading ? 'Thinking…' : 'Send Prompt');

  promptBox.addEventListener('input', (e) => {
    aiDraftPrompt = e.target.value;
    submitBtn.disabled = store.aiLoading || !aiDraftPrompt.trim();
  });
  promptBox.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && aiDraftPrompt.trim() && !store.aiLoading) {
      e.preventDefault();
      api.askAi(promptBox.value);
    }
  });

  const root = el('div', { class: 'stack' }, [
    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-title-row' }, [
        el('span', { class: 'card-label', text: `${aiCfg.provider} · ${aiCfg.model}` }),
        el('span', { class: 'badge', text: aiCfg.endpoint }),
      ]),
      promptBox,
      el('div', { style: { display: 'flex', justifyContent: 'flex-end', marginTop: '8px' } }, [
        submitBtn,
      ]),
    ]),
  ]);

  if (store.aiError) {
    root.appendChild(el('div', { class: 'notice-banner', style: { borderColor: 'rgba(248,113,113,0.4)', color: '#fecaca' }, text: store.aiError }));
  } else if (store.aiResponse) {
    root.appendChild(
      el('div', { class: 'glass-card' }, [
        el('div', { class: 'card-label', style: { marginBottom: '4px' }, text: 'Response' }),
        el('div', { style: { fontSize: '12px', whiteSpace: 'pre-wrap', userSelect: 'text' }, text: store.aiResponse.reply }),
      ])
    );
  }

  return root;
}
