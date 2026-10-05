/**
 * Run real browser acceptance against deployed hosts with an isolated QA profile.
 * Read-only mode never signs in or mutates data. Staging-focus requires explicit
 * disposable fixtures and uses genuine provider UI; no cookies/tokens are injected.
 *
 * @example
 * node scripts/web-browser-journey.mjs --mode read-only --origin https://skills.moesegfault.dev --product https://mskill.moesegfault.dev
 * @example
 * node scripts/web-browser-journey.mjs --mode staging-focus --origin https://skills-staging.moesegfault.dev --product https://mskill-staging.moesegfault.dev --actor .temp/qa/primary.json --second-actor .temp/qa/second.json --archive .temp/qa/disposable.skill
 */
import { chromium } from 'playwright-core';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

const option = name => { const index = process.argv.indexOf(`--${name}`); return index < 0 ? undefined : process.argv[index + 1]; };
const mode = option('mode') || 'read-only';
assert.ok(['read-only', 'staging-focus'].includes(mode), 'Unknown journey mode');
const origin = new URL(option('origin') || 'https://skills.moesegfault.dev').origin;
const product = new URL(option('product') || 'https://mskill.moesegfault.dev').origin;
const executablePath = option('browser') || process.env.MSKILL_QA_BROWSER || (process.platform === 'win32' ? 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe' : undefined);
assert.ok(executablePath, 'Specify an existing official browser executable with --browser');
const base = path.resolve(option('output') || `.temp/web-journeys/${mode}-${Date.now()}`);
const relative = path.relative(process.cwd(), base);
assert.ok(relative && !relative.startsWith('..') && !path.isAbsolute(relative) && ['.temp', '.cache'].includes(relative.split(path.sep)[0]), 'Artifacts must stay under project .temp or .cache');
await mkdir(path.join(base, 'browser-temp'), { recursive: true });
await mkdir(path.join(base, 'screenshots'), { recursive: true });
process.env.TEMP = path.join(base, 'browser-temp'); process.env.TMP = process.env.TEMP;
const browser = await chromium.launch({ executablePath, headless: true, downloadsPath: path.join(base, 'downloads') });
const events = [], sensitive = new Set();

/** Keep all diagnostics free of credentials and authorization-query material. */
function sanitize(value) {
  let text = String(value).replace(/https?:\/\/[^\s]+\?[^\s]+/g, '[URL query omitted]');
  for (const secret of sensitive) if (secret) text = text.replaceAll(secret, '[private fixture value omitted]');
  return text;
}
function record(action) { events.push({ action, at: new Date().toISOString() }); console.log(action); }
async function screenshot(page, name) { await page.screenshot({ path: path.join(base, 'screenshots', `${name}.png`), fullPage: true }); }
async function context(mobile = false) {
  const result = await browser.newContext({ viewport: mobile ? { width: 390, height: 844 } : { width: 1280, height: 900 }, locale: 'zh-CN', acceptDownloads: true });
  result.setDefaultTimeout(15000); return result;
}

/** Account fixtures contain only an authorized QA username/password, never session injection. */
async function signIn(context, page, file) {
  const account = JSON.parse(await readFile(file, 'utf8'));
  assert.ok(account.username && account.password, 'A dedicated QA account fixture is required');
  sensitive.add(account.username); sensitive.add(account.password);
  await page.goto(`${origin}/mine`, { waitUntil: 'networkidle' });
  await page.locator('a[href^="/auth/login"]').first().click();
  assert.equal(new URL(page.url()).origin, 'https://login-staging.moesegfault.dev', 'QA credentials may only enter the first-party staging Login origin');
  await page.locator('input[name=login]').fill(account.username);
  await page.locator('input[name=password]').fill(account.password);
  await page.getByRole('button', { name: '使用密码登录', exact: true }).click();
  await page.waitForURL(`${origin}/mine`, { timeout: 30000 });
  await page.locator('[data-publish]').first().waitFor({ state: 'visible' });
  const session = await (await context.request.get(`${origin}/web/session`)).json();
  assert.ok(session.user?.owner_id); return session.user.owner_id;
}

/** Check public localized/theme/mobile rendering without any account or mutations. */
async function readOnly() {
  const guest = await context(), page = await guest.newPage();
  for (const [locale, prefix] of [['zh-CN', ''], ['ja', '/ja'], ['en', '/en']]) {
    for (const [host, kind] of [[origin, 'workspace'], [product, 'product']]) {
      await page.goto(`${host}${prefix}/`, { waitUntil: 'networkidle' });
      assert.equal(await page.locator('html').getAttribute('lang'), locale);
      for (const theme of ['light', 'dark']) {
        await page.locator('#theme').selectOption(theme);
        assert.equal(await page.locator('html').getAttribute('data-moe-theme'), theme);
        await screenshot(page, `${kind}-${locale}-${theme}-desktop`);
      }
      await page.setViewportSize({ width: 390, height: 844 });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), 'Mobile horizontal overflow');
      await screenshot(page, `${kind}-${locale}-mobile`);
      await page.setViewportSize({ width: 1280, height: 900 }); record(`Public ${kind} ${locale} themes/mobile`);
    }
  }
  for (const [host, kind] of [[origin, 'workspace'], [product, 'product']]) {
    await page.goto(host, { waitUntil: 'networkidle' });
    await page.locator('#theme').selectOption('auto');
    await page.emulateMedia({ colorScheme: 'light' });
    const light = await page.evaluate(() => getComputedStyle(document.body).color);
    await page.emulateMedia({ colorScheme: 'dark' });
    await page.waitForFunction(previous => getComputedStyle(document.body).color !== previous, light, { timeout: 5000 });
    await screenshot(page, `${kind}-system-auto-dark`);
    await page.emulateMedia({ colorScheme: 'light' });
    await page.waitForFunction(expected => getComputedStyle(document.body).color === expected, light, { timeout: 5000 });
    await screenshot(page, `${kind}-system-auto-light`); record(`Public ${kind} system auto follows OS media`);
  }
  await page.goto(product, { waitUntil: 'networkidle' });
  await page.locator('.hero .actions a[href="#downloads"]').click();
  assert.equal(new URL(page.url()).hash, '#downloads');
  const archive = page.locator('.download-card').first();
  const archiveUrl = await archive.getAttribute('href');
  assert.ok(archiveUrl?.startsWith('https://github.com/kleedaisuki/moesegfault-mskill/releases/'));
  const downloaded = page.waitForEvent('download', { timeout: 30000 }); await archive.click();
  const item = await downloaded;
  assert.equal(await item.failure(), null);
  const destination = path.join(base, 'downloaded-client.tar.gz'); await item.saveAs(destination);
  const digest = createHash('sha256').update(await readFile(destination)).digest('hex');
  const checksum = await guest.request.get(`${archiveUrl}.sha256`); assert.equal(checksum.status(), 200);
  assert.equal((await checksum.text()).trim().split(/\s+/)[0], digest);
  record('Product actual download CTA obtains client matching published SHA-256');
  await page.goto(product, { waitUntil: 'networkidle' });
  await page.locator('.hero .actions a[href^="https://skills"]').click();
  await page.waitForURL(url => url.origin === origin);
  record('Product actual workspace CTA reaches distinct registry host');
  for (const [host, route] of [[product, '/robots.txt'], [product, '/sitemap.xml'], [product, '/llms.txt'], [product, '/agent.json'], [origin, '/robots.txt'], [origin, '/sitemap.xml'], [origin, '/llms.txt'], [origin, '/v1/community']]) {
    const response = await guest.request.get(`${host}${route}`); assert.equal(response.status(), 200);
  }
  record('Public robots/sitemap/agent/community discovery routes respond');
  assert.equal((await (await guest.request.get(`${origin}/web/session`)).json()).user, null);
}

/** Exercise the deployed regression and disposable participation workflow on mobile. */
async function stagingFocus() {
  assert.equal(origin, 'https://skills-staging.moesegfault.dev', 'Mutable journeys are restricted to the owned staging workspace');
  const actor = option('actor'), secondActor = option('second-actor'), archive = option('archive');
  assert.ok(actor && secondActor && archive, 'Provide both authorized QA actors and a disposable .skill');
  const primary = await context(true), page = await primary.newPage();
  const owner = await signIn(primary, page, actor);
  if (option('expected-owner')) assert.equal(owner, option('expected-owner'));
  await page.locator('[data-publish]').first().click();
  await page.locator('#publish-dialog').waitFor({ state: 'visible' });
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
  await screenshot(page, 'mobile-upload-dialog');
  await page.locator('#archive-file').setInputFiles(path.resolve(archive));
  await page.locator('#publish-submit').click();
  await page.waitForURL(url => url.pathname.startsWith(`/skills/${owner}/`));
  await page.waitForLoadState('networkidle');
  const detail = page.url(); record('Genuine primary login and actual mobile publication');

  let release; const gate = new Promise(resolve => { release = resolve; });
  await page.route('**/assets/app.js', async route => { await gate; await route.continue(); });
  await page.goto(detail, { waitUntil: 'commit' });
  await page.locator('[data-tab=files]').waitFor({ state: 'visible' });
  assert.equal(await page.locator('[data-tab=files]').isEnabled(), false);
  assert.equal(await page.locator('[data-tab=files]').getAttribute('aria-disabled'), 'true');
  await screenshot(page, 'early-tabs-disabled'); release();
  await page.waitForLoadState('networkidle'); await page.unroute('**/assets/app.js');
  assert.equal(await page.locator('[data-tab=files]').isEnabled(), true);
  record('Early SSR tabs honestly disabled until handlers exist');

  const second = await context(true), other = await second.newPage();
  assert.notEqual(await signIn(second, other, secondActor), owner);
  await other.goto(`${detail}#discussion`, { waitUntil: 'networkidle' });
  await other.locator('#comment-body').fill('Disposable staged author comment');
  await other.locator('#comment-submit').click();
  const own = other.locator('.comment').filter({ hasText: 'Disposable staged author comment' });
  await own.waitFor({ state: 'visible' }); await screenshot(other, 'mobile-comment');
  await other.route('**/web/session', async route => { await new Promise(resolve => setTimeout(resolve, 5000)); await route.continue(); });
  await other.reload({ waitUntil: 'domcontentloaded' });
  await own.waitFor({ state: 'visible' });
  assert.equal(await own.getByRole('button', { name: '编辑', exact: true }).count(), 0);
  await own.getByRole('button', { name: '编辑', exact: true }).waitFor({ state: 'visible', timeout: 12000 });
  await other.unroute('**/web/session'); await screenshot(other, 'delayed-session-permissions');
  await own.getByRole('button', { name: '编辑', exact: true }).click();
  await other.locator('#comments textarea').fill('Disposable staged author edited');
  await other.locator('#comments').getByRole('button', { name: '保存', exact: true }).click();
  const edited = other.locator('.comment').filter({ hasText: 'Disposable staged author edited' });
  await edited.waitFor({ state: 'visible' }); other.once('dialog', dialog => dialog.accept());
  await edited.getByRole('button', { name: '删除评论', exact: true }).click();
  await edited.waitFor({ state: 'hidden' }); record('Delayed genuine session and mobile author comment lifecycle');

  page.once('dialog', dialog => dialog.accept()); await page.locator('#delete-skill').click();
  await page.waitForURL(`${origin}/mine`);
  await page.getByRole('button', { name: '退出登录', exact: true }).click();
  await page.waitForURL(`${origin}/`);
  await other.getByRole('button', { name: '退出 moeSegFault 账号', exact: true }).click();
  await other.waitForURL(`${origin}/`, { timeout: 30000 }); record('Disposable cleanup and application/provider logout');
}

try {
  if (mode === 'read-only') await readOnly(); else await stagingFocus();
  await writeFile(path.join(base, 'results.json'), JSON.stringify({ pass: true, mode, events }, null, 2));
} catch (error) {
  const failure = sanitize(error.message);
  await writeFile(path.join(base, 'results.json'), JSON.stringify({ pass: false, mode, events, error: failure }, null, 2));
  console.error(failure); process.exitCode = 1;
} finally { await browser.close(); }
