"""Compiled-app loading states, controlled resource fixtures and genuine data-image events.
Managed Chromium prohibits URL navigation: tests use set_content without disabling policy.
A test-only module seam holds image source assignment; no network/VM reliability is claimed.
"""
from pathlib import Path
import json
import os
import shutil
import base64
import hashlib
import re
import sys
import time
import unittest
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'artifacts' / 'design-system'


class LoadingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        OUT.mkdir(parents=True, exist_ok=True)
        cls.html = (ROOT / 'dist/hatchling-standalone.html').read_text()
        script = re.search(r'<script>(.*?)</script>', cls.html, re.S).group(1)
        exposed = script.replace('const disposeAgentControl =', 'window.__loadingTestApp = app;\nconst disposeAgentControl =')
        exposed = exposed.rsplit('})();', 1)[0] + 'window.__loadingTestLoad = load;\n})();'
        digest = base64.b64encode(hashlib.sha256(exposed.encode()).digest()).decode()
        cls.test_html = cls.html.replace(script, exposed).replace(re.search(r"sha256-[A-Za-z0-9+/=]+", cls.html).group(0), 'sha256-' + digest)
        cls.driver = sync_playwright().start()
        cls.browser = cls.driver.chromium.launch(executable_path=os.environ.get('CHROMIUM_PATH') or shutil.which('chromium'), headless=True, args=['--no-sandbox'])

    @classmethod
    def tearDownClass(cls):
        cls.browser.close()
        cls.driver.stop()

    def setUp(self):
        self.context = self.browser.new_context(viewport={'width': 1600, 'height': 960}, color_scheme='dark', reduced_motion='reduce')
        self.page = self.context.new_page()
        self.page.set_default_timeout(7000)
        self.errors = []
        self.page.on('pageerror', lambda error: self.errors.append(str(error)))
        self.page.set_content(self.test_html, wait_until='domcontentloaded')
        expect(self.page.locator('#message-input')).to_be_visible()

    def tearDown(self):
        self.context.close()
        self.assertEqual(self.errors, [])

    def scenario(self, name):
        self.page.keyboard.press('Control+Shift+D')
        self.page.locator(f'[data-loading-preview="{name}"]').click()

    def route(self, name):
        self.page.evaluate('(name) => { location.hash = name; }', name)
        self.page.wait_for_timeout(60)

    def capture(self, name):
        self.page.screenshot(path=str(OUT / f'{name}.png'))

    def no_overflow(self):
        self.assertTrue(self.page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1'))

    def test_01_ready_routes_have_no_artificial_loading(self):
        for route in ['ideas', 'feed', 'library/images']:
            self.route(route)
            expect(self.page.locator('.loading-region')).to_have_count(0)
            expect(self.page.locator('.content-state')).to_have_count(0)
        expect(self.page.locator('.artifact-preview .loading-image')).to_have_attribute('data-media-state', 'ready')
        self.assertEqual(self.page.locator('.artifact-preview img').evaluate('(img) => getComputedStyle(img).objectFit'), 'contain')
        self.capture('04-library-images')

    def test_02_ideas_first_load_announces_one_status_and_resolves(self):
        self.page.clock.install()
        self.scenario('ideas-loading')
        expect(self.page.locator('.idea-row')).to_have_count(0)
        expect(self.page.locator('.loading-region [aria-busy="true"]')).to_have_count(1)
        expect(self.page.locator('.skeleton-content button, .skeleton-content [tabindex]')).to_have_count(0)
        expect(self.page.locator('.content-state [role="status"]')).to_have_count(1)
        self.assertFalse(self.page.locator('.content-state [role="status"]').evaluate('(el) => !!el.closest("[aria-busy=true]")'))
        self.capture('02-ideas-loading')
        self.page.clock.fast_forward(6100)
        expect(self.page.locator('.loading-region')).to_have_count(0)
        expect(self.page.locator('.idea-row').first).to_be_visible()
        self.capture('03-ideas-ready')

    def test_03_error_replaces_skeleton_and_retry_is_scoped(self):
        self.page.clock.install()
        self.scenario('ideas-error')
        expect(self.page.locator('.content-state')).to_contain_text('couldn’t load')
        expect(self.page.locator('.skeleton-content')).to_have_count(0)
        self.capture('05-ideas-error')
        self.page.get_by_role('button', name='Retry loading ideas', exact=True).click()
        expect(self.page.locator('.skeleton-content-ideas')).to_be_visible()
        self.page.clock.fast_forward(6100)
        expect(self.page.locator('.idea-row').first).to_be_visible()

    def test_04_cached_refresh_keeps_every_existing_post_readable(self):
        self.route('feed')
        old = self.page.locator('.feed-post h3').all_text_contents()
        self.scenario('feed-refresh')
        self.assertEqual(self.page.locator('.feed-post h3').all_text_contents(), old)
        expect(self.page.locator('.loading-region')).to_have_count(0)
        expect(self.page.locator('.content-state')).to_contain_text('last loaded content stays visible')
        self.capture('06-feed-refresh')
        self.page.get_by_role('button', name='End preview', exact=True).click()
        expect(self.page.locator('.content-state')).to_have_count(0)

    def test_05_refresh_failure_keeps_content_and_retry_available(self):
        self.scenario('feed-refresh-error')
        expect(self.page.locator('.feed-post').first).to_be_visible()
        expect(self.page.locator('.content-state')).to_contain_text('Couldn’t update')
        expect(self.page.locator('.loading-region')).to_have_count(0)
        self.capture('07-refresh-error')
        self.page.get_by_role('button', name='Retry loading feed', exact=True).click()
        expect(self.page.locator('.feed-post').first).to_be_visible()
        expect(self.page.locator('.content-state')).to_contain_text('Updating your feed')

    def test_06_library_skeleton_follows_grid_and_list(self):
        self.scenario('library-loading')
        expect(self.page.locator('.skeleton-content-library.artifact-grid')).to_be_visible()
        expect(self.page.locator('.skeleton-artifact')).to_have_count(4)
        self.capture('08-library-loading')
        self.page.get_by_role('button', name='Switch to list view', exact=True).click()
        expect(self.page.locator('.skeleton-content-library.artifact-list')).to_be_visible()
        self.no_overflow()
        self.capture('09-library-list-loading')

    def test_07_loading_navigation_preserves_original_chat_draft(self):
        self.page.locator('#message-input').fill('Keep this exact draft while the list loads.')
        self.scenario('ideas-loading')
        self.route('chat/reference')
        expect(self.page.locator('#message-input')).to_have_value('Keep this exact draft while the list loads.')
        expect(self.page.locator('.content-state')).to_have_count(0)
        expect(self.page.locator('.connector-chat-section')).to_be_visible()

    def test_08_reduced_motion_is_static_and_normal_motion_is_shared(self):
        self.scenario('ideas-loading')
        block = self.page.locator('.skeleton-block').first
        self.assertEqual(block.evaluate('(el) => getComputedStyle(el).animationName'), 'none')
        self.page.emulate_media(reduced_motion='no-preference')
        self.assertEqual(block.evaluate('(el) => getComputedStyle(el).animationName'), 'skeleton-pulse')
        self.page.emulate_media(reduced_motion='reduce')
        self.assertEqual(block.evaluate('(el) => getComputedStyle(el).animationName'), 'none')

    def test_09_phone_390_retains_current_layout_and_skeletons(self):
        self.page.set_viewport_size({'width': 390, 'height': 844})
        self.scenario('ideas-loading')
        self.no_overflow()
        expect(self.page.locator('.skeleton-content-ideas')).to_be_visible()
        self.capture('10-phone-390-loading')

    def test_10_large_text_320_and_error_actions_stay_accessible(self):
        self.page.locator('#rail-more').click()
        self.page.locator('.rail-more').get_by_role('button', name='Settings', exact=True).click()
        self.page.get_by_role('radio', name='Large', exact=True).click()
        self.page.keyboard.press('Escape')
        self.page.set_viewport_size({'width': 320, 'height': 720})
        self.scenario('ideas-error')
        expect(self.page.get_by_role('button', name='Retry loading ideas', exact=True)).to_be_visible()
        self.no_overflow()
        self.capture('11-phone-320-error')

    def test_11_light_theme_uses_semantic_skeleton_colors(self):
        self.scenario('ideas-loading')
        block = self.page.locator('.skeleton-block').first
        dark = block.evaluate('(el) => getComputedStyle(el).backgroundColor')
        self.page.evaluate("document.documentElement.dataset.theme = 'light'")
        self.assertNotEqual(block.evaluate('(el) => getComputedStyle(el).backgroundColor'), dark)
        self.no_overflow()
        self.capture('12-light-loading')

    def test_12_chat_history_uses_shared_shape_and_retains_composer(self):
        self.page.keyboard.press('Control+Shift+D')
        self.page.locator('[data-reliability-scenario="history-loading"]').click()
        expect(self.page.locator('.history-skeleton .skeleton-content-chat')).to_be_visible()
        expect(self.page.locator('.history-skeleton')).to_have_attribute('aria-hidden', 'true')
        self.page.locator('#message-input').fill('Writing while history loads')
        expect(self.page.locator('#message-input')).to_have_value('Writing while history loads')
        self.capture('13-chat-history-loading')

    def test_13_actual_image_delay_reserves_frame_and_keeps_metadata(self):
        self.media_fixture('hold')
        self.route('library/images')
        frame = self.page.locator('.artifact-preview .loading-image')
        expect(frame).to_have_attribute('data-media-state', 'loading')
        expect(frame.locator('.skeleton-media')).to_be_visible()
        expect(self.page.locator('.artifact-meta')).to_contain_text('Hatchling mascot')
        before = frame.bounding_box()
        self.capture('14-image-loading')
        self.page.evaluate('window.__mediaFixture.release()')
        expect(frame).to_have_attribute('data-media-state', 'ready')
        after = frame.bounding_box()
        self.assertLess(abs(before['height'] - after['height']), 1)
        self.assertLess(abs(before['width'] - after['width']), 1)
        expect(frame.locator('.loading-image-frame')).to_have_attribute('aria-busy', 'false')

    def test_14_image_failure_opens_retry_viewer_without_nested_buttons(self):
        self.media_fixture('fail')
        self.route('library/images')
        thumbnail = self.page.locator('.artifact-preview .loading-image')
        expect(thumbnail).to_have_attribute('data-media-state', 'error')
        expect(self.page.locator('.artifact-preview button, .artifact-preview [tabindex]')).to_have_count(0)
        expect(self.page.locator('.artifact-meta')).to_contain_text('Hatchling mascot')
        self.capture('15-image-error-thumbnail')
        self.page.locator('.artifact-preview').click()
        full = self.page.locator('.image-lightbox .loading-image')
        expect(full).to_have_attribute('data-media-state', 'error')
        expect(full.locator('.loading-image-frame')).to_have_attribute('aria-busy', 'false')
        self.capture('16-image-error-viewer')
        self.page.evaluate("window.__mediaFixture.mode = 'ready'")
        self.page.get_by_role('button', name='Retry image', exact=True).click()
        expect(full).to_have_attribute('data-media-state', 'ready')
        self.assertEqual(full.locator('img').evaluate('(img) => img.naturalWidth'), 1254)
        self.capture('17-image-recovered')

    def media_fixture(self, mode):
        # Test-only dependency seam. The loading component, its timers, observers,
        # styles and handlers are unchanged; release/failure use real browser events.
        self.page.evaluate("""(mode) => {
            const dom = window.__loadingTestLoad('components/dom.js');
            const original = dom.h;
            const state = window.__mediaFixture = {mode, pending: [], release() {
                this.mode = 'ready';
                for (const {image, src} of this.pending) {
                    delete image.complete;
                    image.setAttribute('src', src);
                }
                this.pending = [];
            }};
            dom.h = function(tag, attrs = {}, ...children) {
                if (tag !== 'img' || !attrs.class?.includes('loading-image-content') || state.mode === 'ready')
                    return original(tag, attrs, ...children);
                const props = {...attrs}; const src = props.src;
                if (state.mode === 'fail') props.src = 'data:image/png;base64,AAAA';
                else delete props.src;
                const image = original(tag, props, ...children);
                if (state.mode === 'hold') {
                    Object.defineProperty(image, 'complete', {get: () => false, configurable:true});
                    state.pending.push({image,src});
                }
                return image;
            };
        }""", mode)

    def mount_image(self, lazy=False, top=120):
        self.page.evaluate('''({lazy, top}) => {
            const {loadingImage} = window.__loadingTestLoad('components/loading-image.js');
            const host = document.createElement('div'); host.id='media-test';
            host.style.cssText=`position:fixed;left:100px;top:${top}px;width:240px;z-index:1000`;
            host.append(loadingImage({src:'./media/controlled-image.png',alt:'Controlled local sample',retry:true,lazy}));
            document.body.append(host);
        }''', {'lazy': lazy, 'top': top})
        return self.page.locator('#media-test .loading-image')

    def test_15_stalled_image_stops_pulsing_and_offers_recovery(self):
        self.media_fixture('hold')
        self.page.clock.install()
        image = self.mount_image()
        expect(image).to_have_attribute('data-media-state', 'loading')
        self.page.clock.fast_forward(20100)
        expect(image).to_have_attribute('data-media-state', 'error')
        expect(image).to_contain_text('taking too long')
        expect(image.locator('.media-retry')).to_be_visible()
        expect(image.locator('.skeleton-media')).to_be_hidden()

    def test_16_lazy_offscreen_image_does_not_time_out_before_visibility(self):
        self.media_fixture('hold')
        self.page.clock.install()
        image = self.mount_image(lazy=True, top=10000)
        self.page.clock.fast_forward(21000)
        expect(image).to_have_attribute('data-media-state', 'loading')
        self.page.locator('#media-test').evaluate("el => { el.style.top='120px'; }")
        self.page.wait_for_timeout(100)
        self.page.clock.fast_forward(20100)
        expect(image).to_have_attribute('data-media-state', 'error')

    def test_17_removed_image_detaches_callbacks(self):
        self.media_fixture('hold')
        self.mount_image()
        self.page.evaluate("window.detachedMedia = document.querySelector('#media-test .loading-image'); document.querySelector('#media-test').remove();")
        self.page.wait_for_timeout(40)
        self.page.evaluate("window.detachedMedia.querySelector('img').dispatchEvent(new Event('error'))")
        self.assertEqual(self.page.evaluate("window.detachedMedia.dataset.mediaState"), 'loading')

    def test_18_feed_first_load_and_component_document_reference(self):
        self.scenario('feed-loading')
        expect(self.page.locator('.skeleton-content-feed .skeleton-media')).to_have_count(1)
        self.capture('18-feed-loading')
        self.page.get_by_role('button', name='End preview', exact=True).click()
        self.scenario('components')
        expect(self.page.get_by_role('dialog', name='Loading component reference')).to_be_visible()
        expect(self.page.locator('.skeleton-content-document')).to_be_visible()
        self.capture('19-component-reference')
        self.page.keyboard.press('Escape')
        expect(self.page.locator('dialog[open]')).to_have_count(0)

    def test_19_standalone_includes_the_same_styles_and_skeleton_behavior(self):
        other = self.context.new_page()
        other.on('pageerror', lambda error: self.errors.append(str(error)))
        other.set_content((ROOT / 'dist/hatchling-standalone.html').read_text(), wait_until='domcontentloaded')
        expect(other.locator('#message-input')).to_be_visible()
        other.keyboard.press('Control+Shift+D')
        other.locator('[data-loading-preview="ideas-loading"]').click()
        block = other.locator('.skeleton-block').first
        expect(block).to_be_visible()
        self.assertGreater(block.bounding_box()['height'], 10)
        self.assertEqual(block.evaluate('(el) => getComputedStyle(el).animationName'), 'none')
        other.get_by_role('button', name='End preview', exact=True).click()
        other.evaluate("location.hash='#library/images'")
        expect(other.locator('.artifact-preview .loading-image')).to_have_attribute('data-media-state', 'ready')
        other.close()

    def test_20_loading_does_not_replace_pending_approval_or_draft(self):
        self.page.keyboard.press('Control+Shift+D')
        self.page.locator('[data-approval-scenario="network"]').click()
        request = self.page.locator('.task-approval-card').get_attribute('data-task-approval')
        self.page.locator('#message-input').fill('Do not lose the permission context')
        self.scenario('library-loading')
        self.route('chat/reference')
        expect(self.page.locator('.task-approval-card')).to_have_attribute('data-task-approval', request)
        expect(self.page.locator('#message-input')).to_have_value('Do not lose the permission context')
        card = self.page.locator('.task-approval-card').bounding_box()
        self.assertLessEqual(card['y'] + card['height'], self.page.locator('.composer').bounding_box()['y'])
        self.capture('20-approval-preserved')


    def hold_generation(self):
        self.route('feed')
        self.page.evaluate("""() => {
            const app = window.__loadingTestApp;
            const original = app.adapter.execute.bind(app.adapter);
            window.__feedFixture = {calls: 0, complete: null};
            app.adapter.execute = command => {
                if(command.type !== 'feed.generate') return original(command);
                window.__feedFixture.calls++;
                return new Promise(resolve => {
                    window.__feedFixture.complete = async ok => {
                        resolve(ok ? await original(command) : {ok:false, message:'Controlled sample generation failure.'});
                    };
                });
            };
            app.ui.feedPromptOpen = true; app.render();
        }""")

    def test_21_actual_generation_promise_preserves_cache_and_blocks_duplicate(self):
        self.page.clock.install()
        self.hold_generation()
        old = self.page.locator('.feed-post h3').all_text_contents()
        self.page.get_by_role('button', name='Generate', exact=True).click()
        expect(self.page.get_by_role('button', name='Generating…', exact=True)).to_be_disabled()
        self.page.clock.fast_forward(140)
        expect(self.page.locator('.content-state')).to_contain_text('Updating your feed')
        expect(self.page.locator('.loading-region')).to_have_count(0)
        expect(self.page.locator('.content-preview-caption')).to_have_count(0)
        self.assertEqual(self.page.locator('.feed-post h3').all_text_contents(), old)
        self.page.evaluate("window.__loadingTestApp.run({type:'feed.generate'})")
        self.assertEqual(self.page.evaluate('window.__feedFixture.calls'), 1)
        self.page.clock.fast_forward(10100)
        expect(self.page.locator('.content-state')).to_contain_text('Taking longer than usual')
        self.capture('21-pending-feed-command')
        self.page.evaluate('window.__feedFixture.complete(true)')
        expect(self.page.locator('.content-state')).to_have_count(0)
        expect(self.page.get_by_role('button', name='Generate', exact=True)).to_be_enabled()

    def test_22_actual_generation_failure_and_explicit_retry(self):
        self.page.clock.install()
        self.hold_generation()
        old = self.page.locator('.feed-post h3').all_text_contents()
        self.page.get_by_role('button', name='Generate', exact=True).click()
        self.page.evaluate('window.__feedFixture.complete(false)')
        expect(self.page.locator('.content-state')).to_contain_text('Couldn’t update your feed')
        self.assertEqual(self.page.locator('.feed-post h3').all_text_contents(), old)
        self.page.get_by_role('button', name='Retry loading feed', exact=True).click()
        self.assertEqual(self.page.evaluate('window.__feedFixture.calls'), 2)
        self.page.clock.fast_forward(140)
        expect(self.page.locator('.content-state')).to_contain_text('Updating your feed')
        self.page.evaluate('window.__feedFixture.complete(true)')
        expect(self.page.locator('.content-state')).to_have_count(0)


if __name__ == '__main__':
    started = time.monotonic()
    suite = unittest.defaultTestLoader.loadTestsFromNames(sys.argv[1:], sys.modules[__name__]) if len(sys.argv) > 1 else unittest.defaultTestLoader.loadTestsFromTestCase(LoadingTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    (OUT / 'loading-browser-results.json').write_text(json.dumps({'tests': result.testsRun, 'failures': len(result.failures), 'errors': len(result.errors), 'success': result.wasSuccessful(), 'duration_seconds': round(time.monotonic() - started, 2), 'mode': 'Compiled standalone with test-only media-source and command seams; actual data-image load/error; no URL navigation or external requests'}, indent=2) + '\n')
    raise SystemExit(0 if result.wasSuccessful() else 1)
