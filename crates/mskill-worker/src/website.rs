//! Public catalog and service notices served directly by the Rust Worker.
//!
//! Assets are same-origin and have no external dependencies. Package metadata is
//! rendered with DOM text nodes, never interpreted as HTML or executable code.

use worker::{Headers, Response, Result};

/// Return a public page or bundled asset, leaving other paths to the API router.
pub fn page(path: &str) -> Option<Result<Response>> {
    let (body, content_type) = match path {
        "/" => (HOME, "text/html; charset=utf-8"),
        "/privacy" => (PRIVACY, "text/html; charset=utf-8"),
        "/terms" => (TERMS, "text/html; charset=utf-8"),
        "/assets/app.js" => (SCRIPT, "text/javascript; charset=utf-8"),
        "/assets/app.css" => (STYLE, "text/css; charset=utf-8"),
        _ => return None,
    };
    Some(response(body, content_type))
}

/// Apply one security policy to pages and their bundled assets.
fn response(body: &str, content_type: &str) -> Result<Response> {
    let headers = Headers::new();
    headers.set("Content-Type", content_type)?;
    headers.set("Cache-Control", "no-cache")?;
    headers.set("X-Content-Type-Options", "nosniff")?;
    headers.set("Referrer-Policy", "no-referrer")?;
    headers.set(
        "Permissions-Policy",
        "camera=(), microphone=(), geolocation=()",
    )?;
    headers.set("Content-Security-Policy", "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")?;
    Ok(Response::ok(body)?.with_headers(headers))
}

/// Main catalog; all dynamic data and interaction live in the external asset.
const HOME: &str = r##"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>mskill · Skill 分发与复用</title><meta name="description" content="发现、下载和发布 Skill，用 mskill 在本地库与项目之间复用。">
<link rel="stylesheet" href="/assets/app.css"><script src="/assets/app.js" defer></script></head>
<body><a class="skip" href="#catalog">跳到 Skill 列表</a>
<header><a class="brand" href="/">mskill</a><nav aria-label="主要导航"><a href="#start">使用方法</a><a href="/privacy">隐私</a><a href="/terms">条款</a></nav></header>
<main><section class="intro"><p class="eyebrow">skills.moesegfault.dev</p><h1>让 Skill 在项目间复用。</h1>
<p>下载到本地库，复制或链接到项目。用 moeSegFault 账号发布，在自己的账号空间管理 Skill。</p>
<p class="detail">本地库 <code>~/.mskill</code> · 项目目录 <code>.agents/skills</code> · 分发包 <code>.skill</code></p></section>
<section id="catalog" aria-labelledby="catalog-title"><div class="section-heading"><h2 id="catalog-title">公开 Skill</h2><span id="count"></span></div>
<label for="filter">搜索已加载的 Skill</label><input id="filter" type="search" placeholder="名称、发布者或说明" autocomplete="off">
<p id="status" role="status" aria-live="polite">正在加载…</p><div id="packages" class="packages"></div>
<button id="more" type="button" hidden>加载更多</button><noscript><p>请启用 JavaScript 浏览目录，或用 <code>mskill list --cloud</code> 获取列表。</p></noscript></section>
<section id="start" aria-labelledby="start-title"><h2 id="start-title">从命令行开始</h2><div class="steps">
<article><h3>下载与安装</h3><p>用目录中的完整引用替换 <code>owner/name</code>。</p><pre><code>mskill pull owner/name
mskill clone owner/name --project .</code></pre><p>需要跟随本地库变化时，使用 <code>mskill link owner/name --project .</code>。</p></article>
<article><h3>发布与更新</h3><pre><code>mskill add ./my-skill
mskill login
mskill publish local/my-skill</code></pre><p>只保留最新包。再次发布会替换当前包；下载后的更新使用 <code>mskill update owner/name</code>。</p></article></div>
<p class="detail">首次使用？从 <a href="https://github.com/kleedaisuki/moesegfault-mskill">项目仓库</a>获取客户端和安装说明。使用前检查下载内容；不要发布密钥或无权分发的材料。</p></section></main>
<footer><span>mskill</span><a href="/privacy">隐私政策</a><a href="/terms">服务条款</a></footer></body></html>"##;

/// Privacy notice reflects registry data flows, without inventing retention periods.
const PRIVACY: &str = r##"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>隐私政策 · mskill</title><link rel="stylesheet" href="/assets/app.css"></head>
<body><header><a class="brand" href="/">mskill</a><nav aria-label="主要导航"><a href="/">Skill 目录</a><a href="/terms">条款</a></nav></header><main class="notice"><h1>隐私政策</h1>
<p>本政策适用于 skills.moesegfault.dev 的公开 Skill 分发服务。moeSegFault 账号服务的登录与账号处理适用该账号服务自身的隐私政策。</p>
<h2>我们处理哪些信息</h2><ul><li>发布时，服务校验 CLI 提供的账号访问令牌，并将账号签发方与账号标识映射为本服务的发布者标识。令牌不作为 Skill 包内容保存。</li>
<li>服务保存你发布的 Skill 包及其名称、说明、摘要、大小和更新时间。发布者标识、包内容和这些目录信息对公众开放，无需登录即可查看或下载。</li>
<li>Cloudflare 处理网络请求，并提供运行日志与链路追踪。应用日志记录请求标识、路由、响应状态、耗时和追踪标识，不主动记录访问令牌、邮箱或账号标识。</li></ul>
<h2>用途与服务提供方</h2><p>这些信息用于验证发布权限、展示和分发包、更新与删除内容，以及诊断故障和保护服务。托管、对象存储、数据库和运行监测由 Cloudflare 提供；使用服务会涉及其基础设施对请求及相关数据的处理。</p>
<h2>你的选择</h2><p>浏览器目录不设置应用登录 Cookie，不包含第三方分析脚本。登录和包管理通过 CLI 完成。不要在公开包中包含个人信息、凭证、私有项目资料或其他不应公开的内容。</p>
<p>你可以通过 CLI 删除自己发布的包。删除后，目录及后续下载不再提供该包；底层旧包清理由后台执行。删除不能撤回别人已下载或转存的副本，也不意味着相关运行日志或账号映射立即消失。</p>
<h2>隐私问题与联系</h2><p>通过 <a href="https://github.com/kleedaisuki/moesegfault-mskill/issues">项目问题跟踪器</a>联系维护者。请勿在公开问题中附上令牌、完整请求日志或其他敏感信息；需要私密沟通时，先请求私密联系渠道。</p>
<p><a href="/">返回 Skill 目录</a></p></main><footer><a href="/terms">服务条款</a></footer></body></html>"##;

/// Distribution terms distinguish software licensing from uploaded content rights.
const TERMS: &str = r##"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>服务条款 · mskill</title><link rel="stylesheet" href="/assets/app.css"></head>
<body><header><a class="brand" href="/">mskill</a><nav aria-label="主要导航"><a href="/">Skill 目录</a><a href="/privacy">隐私</a></nav></header><main class="notice"><h1>服务条款</h1>
<p>skills.moesegfault.dev 提供公开 Skill 包分发。请在发布或使用包前阅读以下规则。</p>
<h2>发布者责任</h2><p>仅发布你拥有或获准公开分发的材料，附上适用的许可证与必要署名。你保留自己的内容权利；上传表示你授权本服务为分发目的存储、展示和向公众提供该内容。mskill 软件的许可证不自动适用于用户上传的包。</p>
<p>不得上传违法内容、恶意软件、访问凭证、无权公开的个人信息或机密材料；不得冒充其他发布者、侵害他人权利、绕过权限或滥用服务资源。维护者可为安全、合规或处理滥用而限制请求、移除包或停止发布权限。</p>
<h2>最新版与删除</h2><p>每个账号空间内的名称只保留最新包，不提供版本历史或恢复保证。发布替换包前请自行备份。不同账号可以使用同一名称；以完整的 <code>owner/name</code> 区分发布者。</p>
<p>云端删除只影响本服务的目录和后续下载，不删除用户的本地库或项目副本。第三方使用与再分发须遵守包自身的许可证。</p>
<h2>使用包与服务</h2><p>Skill 可以包含指令、脚本和其他文件。下载或安装不等于授权执行；使用前应检查内容及其权限需求。SHA-256 摘要用于识别包字节变化，不是内容安全、作者身份或许可证的保证。</p>
<p>服务按当前可用状态提供，不承诺持续可用、数据永久保留或适合特定用途。请自行保留重要材料。在适用法律允许的范围内，维护者不承担因第三方包或服务中断导致的损失；本条款不排除法律规定不能排除的责任或权利。</p>
<h2>权利投诉与联系</h2><p>通过 <a href="https://github.com/kleedaisuki/moesegfault-mskill/issues">项目问题跟踪器</a>提供包的完整引用、问题说明和相关权利依据。公开提交时请勿附上个人敏感信息；需要私密沟通时，先请求私密联系渠道。</p>
<p>个人信息处理方式见<a href="/privacy">隐私政策</a>。维护者可能更新这些条款；发布前请查看当前内容。</p><p><a href="/">返回 Skill 目录</a></p></main><footer><a href="/privacy">隐私政策</a></footer></body></html>"##;

/// Same-origin catalog client. Filtering applies to fetched pages; pagination is explicit.
const SCRIPT: &str = r##"'use strict';
(() => {
  const list = document.getElementById('packages');
  const filter = document.getElementById('filter');
  const status = document.getElementById('status');
  const more = document.getElementById('more');
  const count = document.getElementById('count');
  const packages = new Map();
  let cursor = null;
  let busy = false;
  let loaded = false;

  /** Construct text-only elements; metadata must never become markup. */
  function element(tag, text, className) {
    const node = document.createElement(tag);
    if (text !== undefined) node.textContent = text;
    if (className) node.className = className;
    return node;
  }

  /** Validate only displayable identifiers and finite package size. */
  function valid(skill) {
    return skill && typeof skill.owner_id === 'string' && skill.owner_id.length > 0 &&
      typeof skill.name === 'string' && skill.name.length > 0 &&
      typeof skill.sha256 === 'string' && /^[a-f0-9]{64}$/i.test(skill.sha256) &&
      Number.isSafeInteger(skill.size_bytes) && skill.size_bytes >= 0;
  }

  /** Render the current filter without re-fetching or losing pagination state. */
  function render() {
    const query = filter.value.trim().toLocaleLowerCase();
    const fragment = document.createDocumentFragment();
    let visible = 0;
    for (const skill of packages.values()) {
      const ref = skill.owner_id + '/' + skill.name;
      const description = typeof skill.description === 'string' ? skill.description : '';
      if (!(`${ref} ${description}`.toLocaleLowerCase().includes(query))) continue;
      visible += 1;
      const card = element('article', undefined, 'package');
      card.append(element('h3', skill.name), element('p', skill.owner_id, 'owner'));
      if (description) card.append(element('p', description, 'description'));
      const details = element('p', Math.ceil(skill.size_bytes / 1024) + ' KiB', 'detail');
      if (skill.updated_at !== undefined && skill.updated_at !== null) {
        const raw = skill.updated_at;
        const date = new Date(typeof raw === 'number' ? raw * 1000 : raw);
        if (!Number.isNaN(date.valueOf())) details.append(document.createTextNode(' · 更新于 ' + date.toLocaleDateString('zh-CN')));
      }
      card.append(details, element('code', ref, 'reference'));
      const actions = element('div', undefined, 'actions');
      const copy = element('button', '复制引用');
      copy.type = 'button';
      copy.addEventListener('click', async () => {
        try {
          await navigator.clipboard.writeText(ref);
          status.textContent = '已复制 ' + ref;
        } catch (_) {
          status.textContent = '无法自动复制，请选中引用后复制。';
        }
      });
      const download = element('a', '下载 .skill');
      download.href = '/v1/skills/' + encodeURIComponent(skill.owner_id) + '/' + encodeURIComponent(skill.name) + '/archive';
      actions.append(copy, download);
      card.append(actions);
      fragment.append(card);
    }
    list.replaceChildren(fragment);
    count.textContent = '已加载 ' + packages.size + ' 个';
    if (!busy) status.textContent = visible ? '显示 ' + visible + ' 个 Skill' :
      (query ? '已加载的 Skill 中没有匹配项。' : '暂时没有公开 Skill。');
  }

  /** Fetch one bounded page; failed requests preserve the retry cursor. */
  async function load() {
    if (busy) return;
    busy = true;
    more.disabled = true;
    status.textContent = '正在加载…';
    try {
      const url = new URL('/v1/skills', window.location.origin);
      if (cursor) url.searchParams.set('cursor', cursor);
      const response = await fetch(url, {headers: {'Accept': 'application/json'}, credentials: 'omit'});
      if (!response.ok) throw new Error('http');
      const data = await response.json();
      if (!data || !Array.isArray(data.skills)) throw new Error('format');
      for (const skill of data.skills) {
        if (valid(skill)) packages.set(skill.owner_id + '/' + skill.name, skill);
      }
      cursor = typeof data.next_cursor === 'string' && data.next_cursor ? data.next_cursor : null;
      loaded = true;
      busy = false;
      render();
      more.hidden = !cursor;
      more.textContent = '加载更多';
    } catch (_) {
      status.textContent = '目录加载失败，请重试。已加载的内容仍可使用。';
      more.hidden = false;
      more.textContent = loaded ? '重试加载更多' : '重试';
    } finally {
      busy = false;
      more.disabled = false;
    }
  }
  filter.addEventListener('input', render);
  more.addEventListener('click', load);
  load();
})();"##;

/// Responsive layout with visible keyboard focus and no network font dependency.
const STYLE: &str = r##":root{color-scheme:light dark;--bg:#f8fafc;--fg:#172033;--muted:#526079;--line:#d8dfeb;--card:#fff;--accent:#2851b8;font-family:system-ui,-apple-system,"Segoe UI",sans-serif;line-height:1.65}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg)}a{color:var(--accent);text-underline-offset:3px}header,main,footer{max-width:1100px;margin:auto;padding:24px}header,footer{display:flex;gap:24px;align-items:center;flex-wrap:wrap}header{border-bottom:1px solid var(--line);justify-content:space-between}nav{display:flex;gap:20px}.brand{font-size:24px;font-weight:750;text-decoration:none;color:var(--fg)}.intro{padding:35px 0 30px;max-width:800px}h1{font-size:clamp(28px,5vw,44px);line-height:1.25;letter-spacing:-.03em}h2{font-size:24px}h3{font-size:19px;margin:0}p{margin:12px 0}.eyebrow,.detail,.owner,#status,#count{color:var(--muted)}.eyebrow{font-family:monospace}.section-heading{display:flex;align-items:center;gap:20px;flex-wrap:wrap}label{display:block;margin-bottom:6px}input{width:100%;max-width:600px;padding:12px;border:1px solid var(--line);border-radius:8px;background:var(--card);color:var(--fg);font:inherit}.packages{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,290px),1fr));gap:18px}.package{padding:22px;background:var(--card);border:1px solid var(--line);border-radius:10px;overflow-wrap:anywhere}.owner{margin-top:2px;font-size:14px}.description{white-space:pre-wrap}.reference{display:block;user-select:all;overflow-wrap:anywhere}.actions{display:flex;align-items:center;gap:18px;margin-top:18px;flex-wrap:wrap}button{font:inherit;padding:7px 14px;border:1px solid var(--line);border-radius:7px;background:var(--card);color:var(--accent);cursor:pointer}button:disabled{opacity:.6;cursor:wait}#more{margin-top:22px}#start{padding-top:40px}.steps{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr));gap:24px}.steps article{min-width:0}code,pre{font-family:ui-monospace,"Cascadia Code",Consolas,monospace}pre{padding:18px;background:var(--card);border:1px solid var(--line);border-radius:8px;overflow:auto;font-size:14px}footer{margin-top:30px;border-top:1px solid var(--line);font-size:14px}.notice{max-width:800px}.notice h2{margin-top:30px}.notice li{margin-bottom:10px}:focus-visible{outline:3px solid var(--accent);outline-offset:4px}.skip{position:absolute;left:16px;top:-100px;background:var(--card);padding:10px;z-index:1}.skip:focus{top:10px}[hidden]{display:none!important}@media(prefers-color-scheme:dark){:root{--bg:#101724;--fg:#e6ecf6;--muted:#afbbd0;--line:#344155;--card:#192334;--accent:#a3c0ff}}@media(max-width:480px){header,main,footer{padding:18px}nav{gap:14px;font-size:14px}.intro{padding-top:20px}}"##;
