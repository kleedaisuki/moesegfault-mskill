/** Progressive enhancement: pages, navigation and commands remain usable without JavaScript. */
const theme = document.querySelector('#theme');
theme.value = document.documentElement.dataset.moeTheme || 'auto';
theme.addEventListener('change', () => {
  document.documentElement.dataset.moeTheme = theme.value;
  try { localStorage.setItem('moe-theme', theme.value); } catch { /* Session-only preference is valid. */ }
});
document.querySelector('#language').addEventListener('change', (event) => {
  const selected = event.target.value;
  if (['/', '/ja/', '/en/'].includes(selected)) location.assign(selected);
});
document.querySelectorAll('[data-copy]').forEach((button) => {
  button.addEventListener('click', async () => {
    const text = document.getElementById(button.dataset.copy).textContent;
    const original = button.textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = button.dataset.success;
      document.querySelector('#copy-status').textContent = button.dataset.success;
      setTimeout(() => { button.textContent = original; }, 1800);
    } catch {
      document.querySelector('#copy-status').textContent = button.dataset.failure;
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(document.getElementById(button.dataset.copy));
      selection.removeAllRanges();
      selection.addRange(range);
    }
  });
});
