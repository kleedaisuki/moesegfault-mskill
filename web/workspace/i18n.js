/** Supported locales in the order presented by the language selector. */
export const locales = Object.freeze(['zh-CN', 'ja', 'en']);

/** User-facing workspace copy. Placeholders are plain text, never HTML. */
export const messages = Object.freeze({
  'zh-CN': Object.freeze({
    brand: 'Skills', explore: '发现', mySkills: '我的 Skills', publish: '发布',
    search: '搜索', searchPlaceholder: '搜索 Skill 名称或描述', searchButton: '搜索',
    latest: '最近更新', login: '登录', logout: '退出登录', loading: '正在加载…',
    empty: '还没有发布的 Skill。', noResults: '没有找到匹配的 Skill。', loadMore: '加载更多',
    download: '下载 .skill', copyCommand: '复制安装命令', copied: '已复制',
    readme: '说明', files: '文件', discussion: '讨论', author: '作者', updated: '更新时间',
    size: '大小', sha: 'SHA-256', viewProfile: '查看作者', install: '安装',
    replace: '替换归档', delete: '删除 Skill', deleteConfirm: '删除 {name} 及其所有评论？此操作无法撤销，已下载的本地副本不受影响。',
    deleteSuccess: 'Skill 已删除。', publishTitle: '发布 Skill',
    publishHint: '运行 mskill pack ./my-skill -o my-skill.skill，再上传生成的 .skill 文件。同名 Skill 将替换你已发布的内容，只保留最新版。',
    replaceExistingConfirm: '你的账号已发布相同名称的 Skill。替换其当前归档？旧归档不会保留。',
    logoutIdentity: '退出 moeSegFault 账号', previewPlainText: '纯文本预览',
    chooseArchive: '选择 .skill 文件', publishButton: '发布 Skill', publishSuccess: 'Skill 已发布。',
    save: '保存', cancel: '取消', commentPlaceholder: '分享使用体验、提出问题或建议',
    commentSubmit: '发表评论', commentLogin: '登录后参与讨论', noComments: '还没有评论。',
    edit: '编辑', deleteComment: '删除评论', deleteCommentConfirm: '删除这条评论？此操作无法撤销。',
    theme: '外观', light: '浅色', dark: '深色', auto: '跟随系统', language: '语言',
    privacy: '隐私', terms: '条款', product: 'mskill', skip: '跳到主要内容', back: '返回',
    notFound: '找不到这个 Skill，或它已被删除。', error: '操作未完成。', retry: '重试',
    networkError: '连接失败，请检查网络后重试。', authRequired: '请先登录 moeSegFault 账号。',
    fileBinary: '此文件无法以文本预览。请下载归档查看。',
    fileTooLarge: '此文件超过预览大小限制。请下载归档查看。',
    fileMissing: '找不到此文件。它可能已在更新中移除。',
    changed: 'Skill 已更新。请刷新后查看最新内容。', commentsCount: '{count} 条评论',
    skillsCount: '{count} 个 Skills', close: '关闭', sessionExpired: '登录已过期，请重新登录。',
    csrfError: '页面已过期，请刷新后重试。', validationError: '请检查填写内容后重试。',
    permissionDenied: '你没有执行此操作的权限。', uploadTooLarge: '归档超过大小限制（{limit}）。',
    invalidArchive: '这个文件不是有效的 Skill 归档。请使用 mskill pack 重新打包。',
    nameMismatch: 'Skill 名称与归档内容不匹配。', unpublished: '尚未发布', draft: '草稿',
    bytes: '{count} 字节', previous: '上一页', next: '下一页', preview: '预览',
    profile: '作者主页', publishedSkills: '发布的 Skills', commentUpdated: '评论已更新。',
    commentDeleted: '评论已删除。', uploadProgress: '正在上传…',
    replaceConfirm: '替换 {name} 的已发布内容？旧归档不会保留。', confirm: '确认',
    source: '源文件', raw: '原始文本', refresh: '刷新', allSkills: '全部 Skills',
    mySkillsEmpty: '你还没有发布 Skill。', profileEmpty: '这位作者还没有发布 Skill。',
    description: '描述', name: '名称', owner: '发布者', anonymous: '访客',
    commentLength: '评论最多 {limit} 个字符。', commentRequired: '请先填写评论。',
    archiveRequired: '请选择 .skill 文件。', signInHint: '使用 moeSegFault 账号发布 Skill、管理内容和参与讨论。',
    currentArchive: '当前归档', noReadme: '没有可预览的说明文件。',
    unavailable: '暂时无法加载，请稍后重试。', copyFailed: '未能复制，请手动选择并复制命令。',
    loginFailed: '登录未完成，请重试。', loggingOut: '正在退出…',
    fileCount: '{count} 个文件', showFiles: '显示文件', hideFiles: '隐藏文件',
    community: '社区', published: '已发布', downloadHint: '访客也可以下载，不需要登录。',
    latestOnly: '只保留最新版', deleteAccountHint: '这里只管理已发布的 Skill，不会删除你的 moeSegFault 账号。',
  }),
  ja: Object.freeze({
    brand: 'Skills', explore: '見つける', mySkills: '自分の Skills', publish: '公開',
    search: '検索', searchPlaceholder: 'Skill の名前や説明で検索', searchButton: '検索',
    latest: '最近の更新', login: 'ログイン', logout: 'ログアウト', loading: '読み込み中…',
    empty: '公開された Skill はまだありません。', noResults: '一致する Skill が見つかりません。', loadMore: 'さらに表示',
    download: '.skill をダウンロード', copyCommand: 'インストールコマンドをコピー', copied: 'コピーしました',
    readme: '説明', files: 'ファイル', discussion: 'ディスカッション', author: '作者', updated: '更新日時',
    size: 'サイズ', sha: 'SHA-256', viewProfile: '作者を見る', install: 'インストール',
    replace: 'アーカイブを置換', delete: 'Skill を削除', deleteConfirm: '{name} とすべてのコメントを削除しますか？この操作は取り消せません。ダウンロード済みのローカルコピーには影響しません。',
    deleteSuccess: 'Skill を削除しました。', publishTitle: 'Skill を公開',
    publishHint: 'mskill pack ./my-skill -o my-skill.skill を実行し、生成した .skill ファイルをアップロードしてください。同名の Skill は自分の公開済み内容を置き換え、最新版のみを保持します。',
    replaceExistingConfirm: 'この名前の Skill は自分のアカウントですでに公開されています。現在のアーカイブを置き換えますか？以前のアーカイブは保持されません。',
    logoutIdentity: 'moeSegFault アカウントからログアウト', previewPlainText: 'プレーンテキストのプレビュー',
    chooseArchive: '.skill ファイルを選択', publishButton: 'Skill を公開', publishSuccess: 'Skill を公開しました。',
    save: '保存', cancel: 'キャンセル', commentPlaceholder: '使用した感想、質問や提案を共有しましょう',
    commentSubmit: 'コメントを投稿', commentLogin: 'ログインして参加', noComments: 'コメントはまだありません。',
    edit: '編集', deleteComment: 'コメントを削除', deleteCommentConfirm: 'このコメントを削除しますか？この操作は取り消せません。',
    theme: '外観', light: 'ライト', dark: 'ダーク', auto: 'システム設定', language: '言語',
    privacy: 'プライバシー', terms: '利用規約', product: 'mskill', skip: 'メインコンテンツへ移動', back: '戻る',
    notFound: 'この Skill は見つからないか、削除されています。', error: '操作を完了できませんでした。', retry: '再試行',
    networkError: '接続できませんでした。ネットワークを確認して再試行してください。', authRequired: 'moeSegFault アカウントでログインしてください。',
    fileBinary: 'このファイルはテキストでプレビューできません。アーカイブをダウンロードしてください。',
    fileTooLarge: 'このファイルはプレビューのサイズ制限を超えています。アーカイブをダウンロードしてください。',
    fileMissing: 'ファイルが見つかりません。更新で削除された可能性があります。',
    changed: 'Skill が更新されました。再読み込みして最新の内容を確認してください。', commentsCount: 'コメント {count} 件',
    skillsCount: 'Skills {count} 件', close: '閉じる', sessionExpired: 'ログインの有効期限が切れました。再度ログインしてください。',
    csrfError: 'ページの有効期限が切れました。再読み込みして再試行してください。', validationError: '入力内容を確認して再試行してください。',
    permissionDenied: 'この操作を行う権限がありません。', uploadTooLarge: 'アーカイブがサイズ制限（{limit}）を超えています。',
    invalidArchive: '有効な Skill アーカイブではありません。mskill pack で再作成してください。',
    nameMismatch: 'Skill の名前とアーカイブの内容が一致しません。', unpublished: '未公開', draft: '下書き',
    bytes: '{count} バイト', previous: '前のページ', next: '次のページ', preview: 'プレビュー',
    profile: '作者のプロフィール', publishedSkills: '公開した Skills', commentUpdated: 'コメントを更新しました。',
    commentDeleted: 'コメントを削除しました。', uploadProgress: 'アップロード中…',
    replaceConfirm: '{name} の公開済み内容を置き換えますか？以前のアーカイブは保持されません。', confirm: '確認',
    source: 'ソースファイル', raw: '元のテキスト', refresh: '再読み込み', allSkills: 'すべての Skills',
    mySkillsEmpty: 'まだ Skill を公開していません。', profileEmpty: 'この作者はまだ Skill を公開していません。',
    description: '説明', name: '名前', owner: '公開者', anonymous: 'ゲスト',
    commentLength: 'コメントは {limit} 文字以内で入力してください。', commentRequired: 'コメントを入力してください。',
    archiveRequired: '.skill ファイルを選択してください。', signInHint: 'moeSegFault アカウントで Skill の公開、管理、ディスカッションに参加できます。',
    currentArchive: '現在のアーカイブ', noReadme: 'プレビューできる説明ファイルがありません。',
    unavailable: '現在読み込めません。しばらくしてから再試行してください。', copyFailed: 'コピーできませんでした。コマンドを選択して手動でコピーしてください。',
    loginFailed: 'ログインを完了できませんでした。再試行してください。', loggingOut: 'ログアウト中…',
    fileCount: 'ファイル {count} 件', showFiles: 'ファイルを表示', hideFiles: 'ファイルを隠す',
    community: 'コミュニティ', published: '公開済み', downloadHint: 'ログインせずにダウンロードできます。',
    latestOnly: '最新版のみを保持', deleteAccountHint: 'ここでは公開した Skill のみを管理します。moeSegFault アカウントは削除されません。',
  }),
  en: Object.freeze({
    brand: 'Skills', explore: 'Explore', mySkills: 'My Skills', publish: 'Publish',
    search: 'Search', searchPlaceholder: 'Search Skill names or descriptions', searchButton: 'Search',
    latest: 'Recently updated', login: 'Sign in', logout: 'Sign out', loading: 'Loading…',
    empty: 'No Skills have been published yet.', noResults: 'No matching Skills found.', loadMore: 'Load more',
    download: 'Download .skill', copyCommand: 'Copy install command', copied: 'Copied',
    readme: 'Overview', files: 'Files', discussion: 'Discussion', author: 'Author', updated: 'Updated',
    size: 'Size', sha: 'SHA-256', viewProfile: 'View author', install: 'Install',
    replace: 'Replace archive', delete: 'Delete Skill', deleteConfirm: 'Delete {name} and all its comments? This cannot be undone. Downloaded local copies are not affected.',
    deleteSuccess: 'Skill deleted.', publishTitle: 'Publish a Skill',
    publishHint: 'Run mskill pack ./my-skill -o my-skill.skill, then upload the generated .skill file. A Skill with the same name replaces your published content. Only the latest archive is kept.',
    replaceExistingConfirm: 'Your account already publishes a Skill with this manifest name. Replace its current archive? The previous archive will not be kept.',
    logoutIdentity: 'Sign out of your moeSegFault account', previewPlainText: 'Plain-text preview',
    chooseArchive: 'Choose a .skill file', publishButton: 'Publish Skill', publishSuccess: 'Skill published.',
    save: 'Save', cancel: 'Cancel', commentPlaceholder: 'Share your experience, ask a question, or suggest an improvement',
    commentSubmit: 'Post comment', commentLogin: 'Sign in to join the discussion', noComments: 'No comments yet.',
    edit: 'Edit', deleteComment: 'Delete comment', deleteCommentConfirm: 'Delete this comment? This cannot be undone.',
    theme: 'Appearance', light: 'Light', dark: 'Dark', auto: 'System', language: 'Language',
    privacy: 'Privacy', terms: 'Terms', product: 'mskill', skip: 'Skip to content', back: 'Back',
    notFound: 'This Skill was not found or has been deleted.', error: 'The action could not be completed.', retry: 'Try again',
    networkError: 'Connection failed. Check your network and try again.', authRequired: 'Sign in with your moeSegFault account first.',
    fileBinary: 'This file cannot be previewed as text. Download the archive to view it.',
    fileTooLarge: 'This file exceeds the preview size limit. Download the archive to view it.',
    fileMissing: 'This file was not found. It may have been removed in an update.',
    changed: 'This Skill has changed. Refresh to see the latest content.', commentsCount: '{count} comments',
    skillsCount: '{count} Skills', close: 'Close', sessionExpired: 'Your session has expired. Sign in again.',
    csrfError: 'This page has expired. Refresh and try again.', validationError: 'Check your input and try again.',
    permissionDenied: 'You do not have permission to do this.', uploadTooLarge: 'The archive exceeds the size limit ({limit}).',
    invalidArchive: 'This is not a valid Skill archive. Rebuild it with mskill pack.',
    nameMismatch: 'The Skill name does not match the archive contents.', unpublished: 'Not published', draft: 'Draft',
    bytes: '{count} bytes', previous: 'Previous', next: 'Next', preview: 'Preview',
    profile: 'Author profile', publishedSkills: 'Published Skills', commentUpdated: 'Comment updated.',
    commentDeleted: 'Comment deleted.', uploadProgress: 'Uploading…',
    replaceConfirm: 'Replace the published content of {name}? The previous archive will not be kept.', confirm: 'Confirm',
    source: 'Source file', raw: 'Raw text', refresh: 'Refresh', allSkills: 'All Skills',
    mySkillsEmpty: 'You have not published a Skill yet.', profileEmpty: 'This author has not published a Skill yet.',
    description: 'Description', name: 'Name', owner: 'Publisher', anonymous: 'Guest',
    commentLength: 'Comments may contain up to {limit} characters.', commentRequired: 'Enter a comment first.',
    archiveRequired: 'Choose a .skill file first.', signInHint: 'Use your moeSegFault account to publish and manage Skills or join discussions.',
    currentArchive: 'Current archive', noReadme: 'No overview file is available to preview.',
    unavailable: 'Unable to load right now. Try again later.', copyFailed: 'Could not copy. Select the command and copy it manually.',
    loginFailed: 'Sign-in could not be completed. Try again.', loggingOut: 'Signing out…',
    fileCount: '{count} files', showFiles: 'Show files', hideFiles: 'Hide files',
    community: 'Community', published: 'Published', downloadHint: 'Downloads are available without signing in.',
    latestOnly: 'Latest archive only', deleteAccountHint: 'This only manages published Skills. Your moeSegFault account will not be deleted.',
  }),
});

/** Normalize browser language tags to one supported locale, defaulting to English. */
export function normalizeLocale(value) {
  const tag = String(value || '').toLowerCase().replaceAll('_', '-');
  if (tag === 'zh' || tag.startsWith('zh-')) return 'zh-CN';
  if (tag === 'ja' || tag.startsWith('ja-')) return 'ja';
  return 'en';
}

/** Resolve an explicit preference first, then the first supported browser language. */
export function detectLocale(preference, languages = globalThis.navigator?.languages || []) {
  if (preference && locales.includes(preference)) return preference;
  for (const language of languages) {
    if (/^(zh|ja|en)(-|_|$)/i.test(language)) return normalizeLocale(language);
  }
  return 'en';
}

/** Interpolate text-only placeholders; callers must use textContent, not innerHTML. */
export function translate(locale, key, parameters = {}) {
  const template = messages[normalizeLocale(locale)]?.[key] ?? messages.en[key] ?? key;
  return template.replace(/\{([a-zA-Z][a-zA-Z0-9_]*)\}/g, (match, name) =>
    Object.hasOwn(parameters, name) ? String(parameters[name]) : match);
}

/** Keep document language metadata synchronized with the selected locale. */
export function setDocumentLocale(locale, document = globalThis.document) {
  const resolved = normalizeLocale(locale);
  if (document?.documentElement) document.documentElement.lang = resolved;
  return resolved;
}
