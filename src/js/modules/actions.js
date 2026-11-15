import { el, icon } from '../utils.js';

export function renderActions(store, api) {
  const actions = store.config?.quick_actions || [];
  const root = el('div', { class: 'stack' });

  const grid = el('div', { class: 'grid-3' }, actions.map((item) =>
    el('div', {
      class: 'action-tile',
      role: 'button',
      tabindex: '0',
      onClick: () => api.runQuickAction(item),
      onKeydown: (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          api.runQuickAction(item);
        }
      },
    }, [
      el('div', { style: { display: 'flex', justifyContent: 'space-between', width: '100%', alignItems: 'center', gap: '4px' } }, [
        icon(item.icon || 'zap', 15),
        el('button', {
          class: 'icon-btn',
          style: { width: '20px', height: '20px', opacity: '0.72' },
          title: `Remove ${item.label}`,
          onClick: (e) => {
            e.stopPropagation();
            api.removeQuickAction(item.id);
          },
        }, icon('close', 11)),
      ]),
      el('span', { style: { fontWeight: '600', fontSize: '12px' }, text: item.label }),
      el('span', { class: 'muted', style: { fontSize: '9.5px', textTransform: 'uppercase' }, text: item.kind === 'system_tool' ? 'Tool' : item.kind }),
    ])
  ));
  root.appendChild(grid);

  const labelInput = el('input', { class: 'input-glass', placeholder: 'Label (e.g. GitHub)', style: { flex: '1' } });
  const targetInput = el('input', { class: 'input-glass', placeholder: 'https://..., ms-settings:..., or wt', style: { flex: '1.4' } });
  const feedbackEl = el('div', { class: 'muted', style: { fontSize: '11px', marginTop: '6px', display: 'none' } });

  const submitAction = async () => {
    const label = labelInput.value.trim();
    const target = targetInput.value.trim();
    if (!label || !target) {
      feedbackEl.textContent = 'Enter both a label and a valid shortcut target.';
      feedbackEl.style.color = '#fbbf24';
      feedbackEl.style.display = 'block';
      return;
    }

    const kind = target.startsWith('http://') || target.startsWith('https://') || target.startsWith('ms-')
      ? 'uri'
      : (target.toLowerCase().endsWith('.exe') && (target.includes('\\') || target.includes('/')))
        ? 'app'
        : 'system_tool';

    try {
      await api.validateQuickAction(kind, target);
      const saved = await api.addQuickAction({
        id: `qa-${Date.now()}`,
        label,
        icon: 'zap',
        kind,
        target,
      });
      if (saved) {
        labelInput.value = '';
        targetInput.value = '';
        feedbackEl.style.display = 'none';
      }
    } catch (err) {
      feedbackEl.textContent = String(err);
      feedbackEl.style.color = '#f87171';
      feedbackEl.style.display = 'block';
    }
  };

  root.appendChild(
    el('div', { class: 'glass-card' }, [
      el('div', { class: 'card-label', style: { marginBottom: '6px' }, text: 'Add Quick Shortcut' }),
      el('div', { style: { display: 'flex', gap: '6px' } }, [
        labelInput,
        targetInput,
        el('button', {
          class: 'pill-btn primary',
          onClick: submitAction,
        }, 'Add'),
      ]),
      feedbackEl,
    ])
  );

  return root;
}
