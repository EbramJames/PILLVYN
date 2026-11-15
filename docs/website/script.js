const capsule = document.getElementById('demo-capsule');

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
