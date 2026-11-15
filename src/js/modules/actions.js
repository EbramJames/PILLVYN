import { el, icon } from '../utils.js';

export function renderActions(store, api) {
  const actions = store.config?.quick_actions || [];
  const root = el('div', { class: 'stack' });

  const grid = el('div', { class: 'grid-3' }, actions.map((item) =>
    el('button', { class: 'action-tile', onClick: () => api.runQuickAction(item) }, [
      el('div', { style: { display: 'flex', justifyContent: 'space-between', width: '100%', alignItems: 'center' } }, [
        icon(item.icon || 'zap', 15),
        el('span', { class: 'muted', style: { fontSize: '9.5px', textTransform: 'uppercase' }, text: item.kind === 'system_tool' ? 'Tool' : item.kind }),
      ]),
      el('span', { style: { fontWeight: '600', fontSize: '12px' }, text: item.label }),
    ])
  ));
  root.appendChild(grid);

  const labelInput = el('input', { class: 'input-glass', placeholder: 'Label (e.g. GitHub)', style: { flex: '1' } });
  const targetInput = el('input', { class: 'input-glass', placeholder: 'https://... or ms-settings:...', style: { flex: '1.4' } });

  root.appendChild(
    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-label', style: { marginBottom: '6px' }, text: 'Add Quick Shortcut' }),
      el('div', { style: { display: 'flex', gap: '6px' } }, [
        labelInput,
        targetInput,
        el('button', {
          class: 'pill-btn primary',
          onClick: () => {
            const label = labelInput.value.trim();
            const target = targetInput.value.trim();
            if (!label || !target) return;
            const kind = target.startsWith('http://') || target.startsWith('https://') || target.startsWith('ms-')
              ? 'uri'
              : target.endsWith('.exe') && target.includes('\\')
                ? 'app'
                : 'system_tool';
            api.addQuickAction({
              id: `qa-${Date.now()}`,
              label,
              icon: 'zap',
              kind,
              target,
            });
          },
        }, 'Add'),
      ]),
    ])
  );

  return root;
}
