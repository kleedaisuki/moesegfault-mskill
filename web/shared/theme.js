/** First-paint theme bootstrap uses the platform's documented persistence contract. */
try {
  const theme = localStorage.getItem('moe-theme');
  document.documentElement.dataset.moeTheme = ['light', 'dark', 'auto'].includes(theme) ? theme : 'auto';
} catch { document.documentElement.dataset.moeTheme = 'auto'; }
