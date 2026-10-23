'use strict';
(() => {
  const root = document.documentElement;
  const system = window.matchMedia('(prefers-color-scheme: dark)');
  let saved = null;
  try { saved = localStorage.getItem('paperdesk:theme'); } catch { /* Private mode can deny storage. */ }
  const apply = theme => {
    root.dataset.theme = theme==='auto'?(system.matches?'dark':'light'):theme;
    const select = document.getElementById('theme-selector');
    if (select) select.value=theme;
  };
  if(!['auto','light','sepia','dark'].includes(saved))saved='auto';
  apply(saved);
  system.addEventListener('change', () => { if (saved==='auto') apply('auto'); });
  document.addEventListener('DOMContentLoaded', () => {
    apply(saved);
    document.getElementById('theme-selector').addEventListener('change', e => {
      saved = e.target.value;
      apply(saved);
      try { localStorage.setItem('paperdesk:theme', saved); } catch { /* Theme still works for this session. */ }
    });
  });
})();
