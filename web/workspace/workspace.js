import { translate } from '/assets/i18n.js';
import { applyTheme } from '/assets/moe-style.js';

/** The page is readable before hydration; session credentials stay in HttpOnly cookies. */
const locale = document.documentElement.lang;
const t = (key, params) => translate(locale, key, params);
const $ = (selector, parent = document) => parent.querySelector(selector);
const state = { session: null, cursor: null, busy: false, preview: null, commentsCursor: null, comments: [], commentsLoaded: false, commentsBusy: false };
const boot = JSON.parse($('#workspace-data').textContent);
const prefix = locale === 'zh-CN' ? '' : `/${locale}`;
const skillUrl = (owner, name) => `${prefix}/skills/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`;
const profileUrl = owner => `${prefix}/people/${encodeURIComponent(owner)}`;
const apiPath = () => `/v1/skills/${encodeURIComponent(boot.owner)}/${encodeURIComponent(boot.name)}`;

/** Metadata always enters the document as text, never interpolated HTML. */
function el(tag, text, className) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}
function link(text, href, className) {
  const node = el('a', text, className); node.href = href; return node;
}
function button(text, action, variant = 'secondary') {
  const node = el('button', text, 'moe-button'); node.type = 'button';
  node.dataset.variant = variant; node.addEventListener('click', action); return node;
}
function status(text, error = false, target = $('#status')) {
  if (!target) return;
  target.textContent = text; target.toggleAttribute('data-error', error);
}

/** A revoked/expired session restores a visible sign-in path without discarding drafts. */
function signedOut() {
  state.session = null;
  document.querySelectorAll('[data-auth-only]').forEach(node => node.hidden = true);
  document.querySelectorAll('[data-guest-only]').forEach(node => node.hidden = false);
  if ($('#owner-actions')) $('#owner-actions').hidden = true;
  $('#account-controls')?.replaceChildren(link(t('login'), `/auth/login?return_to=${encodeURIComponent(location.pathname + location.hash)}`, 'moe-button'));
}

/** Translate stable problem codes without exposing backend exception details. */
function errorMessage(error) {
  const codes = { csrf_failed: 'csrfError', unauthenticated: 'authRequired', session_expired: 'sessionExpired', forbidden: 'permissionDenied', archive_too_large: 'uploadTooLarge', invalid_archive: 'invalidArchive', skill_name_mismatch: 'nameMismatch', publish_conflict: 'changed', archive_changed: 'changed' };
  if (error.status === 401) return t('sessionExpired');
  if (error.status === 403) return t(codes[error.code] || 'permissionDenied');
  if (error.status === 413) return t('uploadTooLarge', { limit: '16 MiB' });
  if (error.status === 422) return t('invalidArchive');
  return t(codes[error.code] || (error.network ? 'networkError' : 'error'));
}

/** Mutations carry the session-specific CSRF token; anonymous reads never require login. */
async function api(path, options = {}, retry = true) {
  const headers = new Headers(options.headers);
  if (options.method && !['GET', 'HEAD'].includes(options.method)) {
    if (!state.session?.user) throw { status: 401 };
    headers.set('X-CSRF-Token', state.session.csrf_token);
  }
  let response;
  try { response = await fetch(path, { ...options, headers, credentials: 'same-origin' }); }
  catch { throw { network: true }; }
  const data = response.status === 204 ? null : await response.json().catch(() => null);
  if (response.status === 503 && data?.error_code === 'session_refresh_busy' && retry) {
    await new Promise(resolve => setTimeout(resolve, 1000));
    return api(path, options, false);
  }
  if (response.status === 401) signedOut();
  if (!response.ok) throw { status: response.status, code: data?.error_code, requestId: data?.request_id };
  return data;
}
function jsonWrite(method, body) { return { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }; }

/** Theme changes reuse the actual platform API and remain independent of auth state. */
const themeSelect = $('#theme');
if (themeSelect) {
  themeSelect.value = document.documentElement.dataset.moeTheme || 'auto';
  themeSelect.addEventListener('change', () => {
    applyTheme(themeSelect.value);
    try { localStorage.setItem('moe-theme', themeSelect.value); } catch { /* Storage can be unavailable. */ }
  });
}
$('#language')?.addEventListener('change', event => {
  const next = event.target.value === 'zh-CN' ? '' : `/${event.target.value}`;
  const route = location.pathname.replace(/^\/(en|ja)(?=\/|$)/, '');
  location.assign(`${next}${route || '/'}${location.search}${location.hash}`);
});

/** Restore account controls without persisting tokens or identity in local storage. */
async function loadSession() {
  try {
    state.session = await api('/web/session');
    if (state.session?.user) {
      const account = $('#account-controls'); account.replaceChildren();
      account.append(link(state.session.user.display_name || state.session.user.owner_id, profileUrl(state.session.user.owner_id), 'account-name'));
      account.append(button(t('logout'), async () => {
        try {
          await api('/auth/logout', { method: 'POST' });
          location.assign(`${prefix}/`);
        } catch (error) { status(errorMessage(error), true); }
      }, 'ghost'));
      account.append(button(t('logoutIdentity'), async () => {
        try {
          const result = await api('/auth/logout?identity=1', { method: 'POST' });
          location.assign(result.redirect_to || `${prefix}/`);
        } catch (error) { status(errorMessage(error), true); }
      }, 'ghost'));
      document.querySelectorAll('[data-auth-only]').forEach(node => node.hidden = false);
      document.querySelectorAll('[data-guest-only]').forEach(node => node.hidden = true);
      if (boot.owner === state.session.user.owner_id && boot.kind === 'skill') {
        $('#owner-actions').hidden = false;
      }
    }
    // Comment permissions derive from the current session, even if guest reads finished first.
    if (state.commentsLoaded) renderComments();
    if (boot.kind === 'mine') {
      if (state.session?.user) await loadCatalog(true);
      else status(t('authRequired'));
    }
  } catch (error) { status(errorMessage(error), true); }
}

/** One row displays the publisher namespace, current metadata and community counters. */
function skillRow(skill) {
  const row = el('article', undefined, 'skill-row');
  const heading = el('div', undefined, 'skill-title');
  heading.append(link(skill.display_name || skill.owner_id, profileUrl(skill.owner_id), 'owner-link'), el('span', '/'), link(skill.name, skillUrl(skill.owner_id, skill.name)));
  row.append(heading, el('p', skill.description));
  const meta = el('div', undefined, 'meta-line');
  const time = el('time', new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(new Date(skill.updated_at)));
  time.dateTime = skill.updated_at;
  meta.append(time, el('span', bytes(skill.size_bytes)), link(t('commentsCount', { count: skill.comment_count || 0 }), `${skillUrl(skill.owner_id, skill.name)}#discussion`));
  row.append(meta);
  return row;
}
function bytes(size) { return size < 1024 ? `${size} B` : size < 1024 * 1024 ? `${(size / 1024).toFixed(1)} KiB` : `${(size / 1024 / 1024).toFixed(1)} MiB`; }

/** Server-backed search covers the whole registry, not only previously fetched pages. */
async function loadCatalog(reset = false) {
  if (state.busy || !$('#packages')) return;
  state.busy = true;
  const more = $('#more'); if (more) more.disabled = true;
  status(t('loading'));
  if (reset) state.cursor = null;
  const query = new URLSearchParams({ limit: '30' });
  const search = $('#search')?.value.trim(); if (search) query.set('q', search);
  if (boot.kind === 'profile') query.set('owner_id', boot.owner);
  if (boot.kind === 'mine' && state.session?.user) query.set('owner_id', state.session.user.owner_id);
  if (state.cursor) query.set('cursor', state.cursor);
  try {
    const result = await api(`/v1/community?${query}`);
    const items = result.skills || result.items || [];
    if (reset) $('#packages').replaceChildren();
    items.forEach(skill => $('#packages').append(skillRow(skill)));
    state.cursor = result.next_cursor || null;
    if (more) more.hidden = !state.cursor;
    status($('#packages').children.length ? '' : search ? t('noResults') : t('empty'));
  } catch (error) { status(errorMessage(error), true); }
  finally { state.busy = false; if (more) more.disabled = false; }
}
$('#search-form')?.addEventListener('submit', event => {
  event.preventDefault();
  const url = new URL(location.href); const q = $('#search').value.trim();
  if (q) url.searchParams.set('q', q); else url.searchParams.delete('q');
  history.replaceState(null, '', url);
  loadCatalog(true);
});
$('#more')?.addEventListener('click', () => loadCatalog(false));
state.cursor = boot.cursor || null;

/** Mature Markdown parser is loaded only for content pages; uploaded HTML stays inert. */
async function markdown(text) {
  const { default: MarkdownIt } = await import('/assets/markdown-it.mjs');
  const md = new MarkdownIt({ html: false, linkify: true, breaks: false, typographer: false });
  md.validateLink = href => /^https?:\/\//i.test(href) || (!/^[a-z][a-z0-9+.-]*:/i.test(href) && !href.startsWith('//'));
  // Images become text, never trackers or active SVG. The archive remains downloadable.
  md.renderer.rules.image = (tokens, index) => '<span class="image-description">' + md.utils.escapeHtml(tokens[index].content) + '</span>';
  const template = document.createElement('template');
  template.innerHTML = md.render(text.replace(/^---\r?\n[\s\S]*?\r?\n---(?:\r?\n|$)/, ''));
  const seen = new Map();
  template.content.querySelectorAll('h1,h2,h3,h4,h5,h6').forEach(heading => {
    const base = heading.textContent.trim().toLowerCase().replace(/[^\p{L}\p{N}_-]+/gu, '-').replace(/^-|-$/g, '') || 'section';
    const count = seen.get(base) || 0; seen.set(base, count + 1);
    heading.id = 'readme-' + base + (count ? '-' + count : '');
  });
  template.content.querySelectorAll('a').forEach(anchor => {
    const href = anchor.getAttribute('href') || '';
    if (/^https?:\/\//i.test(href)) { anchor.rel = 'nofollow noreferrer'; return; }
    if (href.startsWith('#')) {
      anchor.href = '#readme-' + href.slice(1);
      return;
    }
    anchor.href = '#files';
    anchor.addEventListener('click', event => {
      event.preventDefault(); selectTab('files');
      let path = href.split('#')[0].split('?')[0].replace(/^\.\//, '');
      try { path = decodeURIComponent(path); } catch { /* Leave malformed escapes inert. */ }
      loadFile(path);
    });
  });
  return template.content;
}

/** Detail pages pin previews to one latest digest to avoid presenting mixed content. */
async function loadPreview() {
  status(t('loading'), false, $('#preview-status'));
  try {
    const preview = await api(`${apiPath()}/preview`);
    if (preview.sha256 !== boot.sha256) throw { code: 'archive_changed' };
    state.preview = preview;
    $('#readme-content').replaceChildren(await markdown(preview.skill_md));
    const tree = $('#file-tree'); tree.replaceChildren();
    for (const file of preview.files) {
      const li = el('li');
      const item = button(file.path, () => loadFile(file.path), 'ghost');
      item.dataset.path = file.path; item.title = `${file.path} · ${bytes(file.size_bytes)}`;
      li.append(item); tree.append(li);
    }
    const first = preview.files.find(file => file.path === 'SKILL.md');
    if (first) await loadFile(first.path);
    status('', false, $('#preview-status'));
  } catch (error) {
    status(errorMessage(error), true, $('#preview-status'));
    status(errorMessage(error), true, $('#file-status'));
  }
}
async function loadFile(path) {
  status(t('loading'), false, $('#file-status'));
  try {
    const cached = state.preview?.files.find(file => file.path === path && file.text !== undefined);
    const result = cached ? state.preview : await api(`${apiPath()}/preview?path=${encodeURIComponent(path)}`);
    if (result.sha256 !== boot.sha256) throw { code: 'archive_changed' };
    const file = result.files.find(entry => entry.path === path);
    if (file?.text !== undefined && !cached) {
      const entry = state.preview?.files.find(entry => entry.path === path);
      if (entry) entry.text = file.text;
    }
    $('#file-name').textContent = path;
    $('#file-source').textContent = file?.text ?? (file ? t(file.size_bytes > 256 * 1024 ? 'fileTooLarge' : 'fileBinary') : t('fileMissing'));
    document.querySelectorAll('[data-path]').forEach(item => item.toggleAttribute('aria-current', item.dataset.path === path));
    status('', false, $('#file-status'));
  } catch (error) { status(errorMessage(error), true, $('#file-status')); }
}
/** Ready tabs immediately show their panel; pending content retains a localized status. */
function selectTab(name) {
  document.querySelectorAll('[data-tab]').forEach(button => {
    const selected = button.dataset.tab === name;
    button.setAttribute('aria-selected', String(selected)); button.tabIndex = selected ? 0 : -1;
  });
  document.querySelectorAll('[data-panel]').forEach(panel => panel.hidden = panel.dataset.panel !== name);
  if (name === 'discussion' && !state.commentsLoaded) loadComments(true);
  history.replaceState(null, '', `${location.pathname}${location.search}#${name}`);
}
const tabs = [...document.querySelectorAll('[data-tab]')];
tabs.forEach((tab, index) => {
  tab.addEventListener('click', () => selectTab(tab.dataset.tab));
  tab.addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const target = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length;
    tabs[target].focus(); selectTab(tabs[target].dataset.tab);
  });
  // SSR disables JS-only actions; enable only after both interaction handlers exist.
  tab.disabled = false;
  tab.removeAttribute('aria-disabled');
});
$('#copy-command')?.addEventListener('click', async event => {
  try { await navigator.clipboard.writeText($('#install-command').textContent); event.target.textContent = t('copied'); }
  catch { const range = document.createRange(); range.selectNodeContents($('#install-command')); getSelection().removeAllRanges(); getSelection().addRange(range); status(t('copyCommand')); }
});

/** Comments are public text; write/edit/delete authorization is enforced server-side. */
async function loadComments(reset = false) {
  if (state.commentsBusy) {
    // A completed write must not lose its requested refresh behind an older read.
    state.commentsReload = state.commentsReload || reset;
    return;
  }
  state.commentsBusy = true;
  const query = new URLSearchParams({ limit: '30' });
  if (!reset && state.commentsCursor) query.set('cursor', state.commentsCursor);
  status(t('loading'), false, $('#comments-status'));
  try {
    const result = await api(`${apiPath()}/comments?${query}`);
    if (reset) state.comments = [];
    state.comments.push(...result.comments);
    renderComments();
    state.commentsLoaded = true; state.commentsCursor = result.next_cursor || null;
    $('#comments-more').hidden = !state.commentsCursor;
    status($('#comments').children.length ? '' : t('noComments'), false, $('#comments-status'));
  } catch (error) { status(errorMessage(error), true, $('#comments-status')); }
  finally {
    state.commentsBusy = false;
    if (state.commentsReload) {
      state.commentsReload = false;
      loadComments(true);
    }
  }
}
/** Re-render already fetched pages when authentication changes, without another network read. */
function renderComments() {
  $('#comments').replaceChildren(...state.comments.map(commentNode));
}
function commentNode(comment) {
  const article = el('article', undefined, 'comment');
  const head = el('div', undefined, 'comment-header');
  head.append(link(comment.display_name || comment.owner_id, profileUrl(comment.owner_id)));
  const time = el('time', new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(comment.updated_at || comment.created_at)));
  time.dateTime = comment.updated_at || comment.created_at; head.append(time);
  const actions = el('div', undefined, 'comment-actions');
  const own = state.session?.user?.owner_id === comment.owner_id;
  const moderator = state.session?.user?.owner_id === boot.owner;
  if (own) actions.append(button(t('edit'), () => {
    const form = el('form', undefined, 'comment-form'); const field = el('textarea');
    field.value = comment.body; field.maxLength = 4096; field.required = true; field.setAttribute('aria-label', t('commentPlaceholder'));
    form.append(field, button(t('save'), async () => {
      try { await api(`${apiPath()}/comments/${encodeURIComponent(comment.id)}`, jsonWrite('PATCH', { body: field.value })); await loadComments(true); }
      catch (error) { status(errorMessage(error), true, $('#comments-status')); }
    }), button(t('cancel'), () => loadComments(true), 'ghost'));
    article.replaceChildren(form); field.focus();
  }, 'ghost'));
  if (own || moderator) actions.append(button(t('deleteComment'), async () => {
    if (!confirm(t('deleteCommentConfirm'))) return;
    try { await api(`${apiPath()}/comments/${encodeURIComponent(comment.id)}`, { method: 'DELETE' }); await loadComments(true); }
    catch (error) { status(errorMessage(error), true, $('#comments-status')); }
  }, 'ghost'));
  head.append(actions); article.append(head, el('div', comment.body, 'comment-body')); return article;
}
$('#comment-form')?.addEventListener('submit', async event => {
  event.preventDefault(); const submit = $('#comment-submit'); submit.disabled = true;
  try { await api(`${apiPath()}/comments`, jsonWrite('POST', { body: $('#comment-body').value })); $('#comment-body').value = ''; await loadComments(true); }
  catch (error) { status(errorMessage(error), true, $('#comments-status')); }
  finally { submit.disabled = false; }
});
$('#comments-more')?.addEventListener('click', () => loadComments(false));

/** Publishing sends the portable archive once; Rust derives and validates its name. */
const upload = $('#publish-dialog');
function showPublish(replaceName = null) {
  if (!state.session?.user) { location.assign(`/auth/login?return_to=${encodeURIComponent(location.pathname)}`); return; }
  state.replaceName = replaceName;
  $('#publish-title').textContent = t(replaceName ? 'replace' : 'publishTitle');
  $('#archive-file').value = ''; $('#archive-name').textContent = ''; $('#publish-submit').disabled = true;
  status('', false, $('#publish-status')); upload.showModal();
}
document.querySelectorAll('[data-publish]').forEach(node => node.addEventListener('click', () => showPublish()));
$('#replace-skill')?.addEventListener('click', () => showPublish(boot.name));
$('#publish-close')?.addEventListener('click', () => upload.close());
$('#publish-cancel')?.addEventListener('click', () => upload.close());
$('#archive-file')?.addEventListener('change', () => {
  const file = $('#archive-file').files[0]; $('#publish-submit').disabled = true; if (!file) return;
  if (file.size > 16 * 1024 * 1024) { status(t('uploadTooLarge', { limit: '16 MiB' }), true, $('#publish-status')); return; }
  $('#archive-name').textContent = file.name;
  $('#publish-submit').disabled = false; status('', false, $('#publish-status'));
});
$('#publish-form')?.addEventListener('submit', async event => {
  event.preventDefault(); const file = $('#archive-file').files[0]; if (!file) return;
  $('#publish-submit').disabled = true; status(t('uploadProgress'), false, $('#publish-status'));
  try {
    const owner = state.session.user.owner_id;
    const headers = { 'Content-Type': 'application/vnd.mskill.skill' };
    let published;
    if (state.replaceName) {
      if (!confirm(t('replaceConfirm', { name: state.replaceName }))) { $('#publish-submit').disabled = false; status('', false, $('#publish-status')); return; }
      headers['If-Match'] = `"${boot.sha256}"`;
      published = await api(`/v1/skills/${encodeURIComponent(owner)}/${encodeURIComponent(state.replaceName)}`, { method: 'PUT', headers, body: file });
    } else {
      try { published = await api('/web/publish', { method: 'POST', headers: { ...headers, 'If-None-Match': '*' }, body: file }); }
      catch (error) {
        if (error.code !== 'skill_exists') throw error;
        if (!confirm(t('replaceExistingConfirm'))) { $('#publish-submit').disabled = false; status('', false, $('#publish-status')); return; }
        published = await api('/web/publish', { method: 'POST', headers, body: file });
      }
    }
    location.assign(skillUrl(published.owner_id, published.name));
  } catch (error) { status(errorMessage(error), true, $('#publish-status')); $('#publish-submit').disabled = false; }
});
$('#delete-skill')?.addEventListener('click', async () => {
  if (!confirm(t('deleteConfirm', { name: boot.name }))) return;
  try { await api(apiPath(), { method: 'DELETE', headers: { 'If-Match': `"${boot.sha256}"` } }); location.assign(`${prefix}/mine`); }
  catch (error) { status(errorMessage(error), true); }
});

// Anonymous preview must not wait for an unrelated provider session refresh.
loadSession();
if (boot.kind === 'skill') {
  loadPreview();
  if (['files', 'discussion'].includes(location.hash.slice(1))) selectTab(location.hash.slice(1));
}
if (boot.kind === 'catalog' || boot.kind === 'profile') {
  // SSR rows are retained until a complete replacement is available.
  if (!boot.hasItems || boot.query) loadCatalog(true);
}
