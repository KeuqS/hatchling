"""Shared menu styling and actual sandboxed-preview navigation on the compiled app.
No transport, parent-window access, production host or security-policy bypass.
"""
from pathlib import Path
import json
import os
import shutil
import sys
import time
import unittest
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'artifacts' / 'menus-preview'
SURFACE = """el => { const s = getComputedStyle(el); return Object.fromEntries(
['backgroundColor','color','borderRadius','borderTopColor','borderTopWidth','boxShadow','padding','fontFamily'].map(k => [k,s[k]])); }"""
ROW = """el => { const s = getComputedStyle(el); return Object.fromEntries(
['borderRadius','fontSize','fontWeight','lineHeight','padding','gap','minHeight'].map(k => [k,s[k]])); }"""

class MenuPreviewTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        OUT.mkdir(parents=True, exist_ok=True)
        cls.html = (ROOT / 'dist/hatchling-standalone.html').read_text()
        cls.driver = sync_playwright().start()
        cls.browser = cls.driver.chromium.launch(executable_path=os.environ.get('CHROMIUM_PATH') or shutil.which('chromium'), headless=True, args=['--no-sandbox'])

    @classmethod
    def tearDownClass(cls):
        cls.browser.close()
        cls.driver.stop()

    def setUp(self):
        self.context = self.browser.new_context(viewport={'width': 1600, 'height': 960}, color_scheme='dark', reduced_motion='reduce')
        self.page = self.context.new_page()
        self.page.set_default_timeout(5000)
        self.errors = []
        self.page.on('pageerror', lambda error: self.errors.append(str(error)))
        self.page.on('console', lambda message: self.errors.append(message.text) if message.type == 'error' else None)
        self.page.set_content(self.html, wait_until='load')
        expect(self.page.locator('#message-input')).to_be_visible()

    def tearDown(self):
        self.context.close()
        self.assertEqual(self.errors, [])

    def capture(self, name):
        self.page.screenshot(path=str(OUT / f'{name}.png'), animations='disabled')

    def open_menu(self, selector):
        trigger = self.page.locator(selector).first
        trigger.click()
        expect(trigger).to_have_attribute('aria-expanded', 'true')
        pop = self.page.locator('details.menu[open] > .menu-popover')
        expect(pop).to_have_count(1)
        expect(pop).to_be_visible()
        return pop

    def close_menu(self):
        self.page.keyboard.press('Escape')
        expect(self.page.locator('details.menu[open]')).to_have_count(0)

    def reference(self):
        pop = self.open_menu('#rail-more')
        surface, row = pop.evaluate(SURFACE), pop.locator('button').first.evaluate(ROW)
        self.close_menu()
        return surface, row

    def contained(self, pop):
        self.assertTrue(pop.evaluate('el => { const r=el.getBoundingClientRect(); return r.left>=7.5 && r.top>=7.5 && r.right<=innerWidth-7.5 && r.bottom<=innerHeight-7.5 && el.scrollWidth<=el.clientWidth+1; }'))

    def route(self, name):
        self.page.evaluate('(name) => { location.hash=name; }', name)
        self.page.wait_for_timeout(70)

    def test_01_chat_menus_match_more_surface_and_rows(self):
        surface, row = self.reference()
        for name, selector in [
            ('02-chat-actions', '.chat-nav-row summary'),
            ('03-chat-options', '.side-chat-options summary'),
            ('04-attachment', '.composer .menu summary'),
            ('05-avatar', '.avatar-edit-menu summary'),
            ('07-connector', '.connector-chat-row summary'),
        ]:
            with self.subTest(selector=selector):
                pop = self.open_menu(selector)
                self.assertEqual(pop.evaluate(SURFACE), surface)
                self.assertEqual(pop.locator('button').first.evaluate(ROW), row)
                self.contained(pop)
                self.capture(name)
                self.close_menu()
        pop = self.open_menu('#rail-more')
        self.capture('01-more')

    def test_02_page_and_item_menus_share_the_same_contract(self):
        surface, row = self.reference()
        for route, selector in [
            ('feed', '[aria-label="Feed options"]'),
            ('feed', '.feed-post .menu > summary'),
            ('ideas', '[aria-label="Idea views"]'),
            ('ideas', '.idea-row .menu > summary'),
            ('goals', '[aria-label="Goal options"]'),
            ('library', '[data-artifact="backlog"] summary'),
            ('library', '[aria-label="Library options"]'),
            ('memory', '.memory-entry .menu > summary'),
            ('schedule', '.job-card .menu > summary'),
        ]:
            with self.subTest(route=route, selector=selector):
                self.route(route)
                pop = self.open_menu(selector)
                self.assertEqual(pop.evaluate(SURFACE), surface)
                self.assertEqual(pop.locator('button').first.evaluate(ROW), row)
                self.contained(pop)
                if route == 'library': self.capture('08-library-actions')
                self.close_menu()

    def test_03_document_menu_is_not_a_second_style(self):
        surface, row = self.reference()
        self.page.get_by_role('tab', name='Identity', exact=True).click()
        self.page.get_by_role('button', name='Edit', exact=True).click()
        pop = self.open_menu('.identity-document-dock summary')
        self.assertEqual(pop.evaluate(SURFACE), surface)
        self.assertEqual(pop.locator('button').first.evaluate(ROW), row)
        self.contained(pop)
        self.capture('09-document-menu')
        self.close_menu()
        expect(self.page.locator('.identity-document-dock')).to_be_visible()

    def test_04_keyboard_open_focuses_first_and_up_focuses_last(self):
        expect(self.page.get_by_label('Side chat options', exact=True)).to_have_count(1)
        summary = self.page.locator('.chat-nav-row summary').first
        summary.focus()
        self.page.keyboard.press('ArrowDown')
        expect(self.page.get_by_role('button', name='Rename chat', exact=True)).to_be_focused()
        self.page.keyboard.press('End')
        expect(self.page.get_by_role('button', name='Delete chat', exact=True)).to_be_focused()
        self.page.keyboard.press('Home')
        expect(self.page.get_by_role('button', name='Rename chat', exact=True)).to_be_focused()
        self.close_menu()
        expect(summary).to_be_focused()
        self.page.keyboard.press('ArrowUp')
        expect(self.page.get_by_role('button', name='Delete chat', exact=True)).to_be_focused()
        self.page.keyboard.press('ArrowDown')
        expect(self.page.get_by_role('button', name='Rename chat', exact=True)).to_be_focused()

    def test_05_typeahead_tab_outside_click_and_single_open_menu(self):
        self.open_menu('.chat-nav-row summary')
        self.page.keyboard.press('p')
        expect(self.page.get_by_role('button', name='Pin', exact=True)).to_be_focused()
        self.page.keyboard.press('Tab')
        expect(self.page.locator('details.menu[open]')).to_have_count(0)
        self.open_menu('.side-chat-options summary')
        self.page.locator('#message-input').click()
        expect(self.page.locator('details.menu[open]')).to_have_count(0)
        self.open_menu('#rail-more')
        self.open_menu('.chat-nav-row summary')
        expect(self.page.locator('details.menu[open]')).to_have_count(1)

    def test_06_rename_pin_and_danger_confirmation_keep_working(self):
        self.open_menu('.chat-nav-row summary')
        self.page.get_by_role('button', name='Rename chat', exact=True).click()
        dialog = self.page.get_by_role('dialog', name='Rename side chat', exact=True)
        expect(dialog).to_be_visible()
        self.page.get_by_label('Chat name', exact=True).fill('Menu consistency check')
        dialog.get_by_role('button', name='Save name', exact=True).click()
        trigger = self.page.locator('summary[aria-label="Manage chat Menu consistency check"]')
        self.open_menu('summary[aria-label="Manage chat Menu consistency check"]')
        self.page.get_by_role('button', name='Pin', exact=True).click()
        expect(trigger).to_be_focused()
        pop = self.open_menu('summary[aria-label="Manage chat Menu consistency check"]')
        expect(pop.get_by_role('button', name='Unpin', exact=True)).to_be_visible()
        expect(pop.locator('.menu-separator')).to_have_count(1)
        delete = pop.get_by_role('button', name='Delete chat', exact=True)
        self.assertNotEqual(delete.evaluate('el=>getComputedStyle(el).color'), pop.locator('button').first.evaluate('el=>getComputedStyle(el).color'))
        delete.click()
        expect(self.page.get_by_role('dialog', name='Delete this side chat?', exact=True)).to_be_visible()
        self.page.get_by_role('button', name='Cancel', exact=True).click()
        expect(self.page.get_by_role('button', name='Menu consistency check', exact=True)).to_be_visible()

    def test_07_disabled_menu_items_are_skipped_by_keyboard(self):
        pop = self.open_menu('.chat-nav-row summary')
        # Actual app controls, with disabled state isolated from delivery timing.
        pop.get_by_role('button', name='Delete chat', exact=True).evaluate('(el)=>el.disabled=true')
        self.page.keyboard.press('End')
        expect(pop.get_by_role('button', name='Archive chat', exact=True)).to_be_focused()
        self.page.keyboard.press('ArrowDown')
        expect(pop.get_by_role('button', name='Rename chat', exact=True)).to_be_focused()
        pop.locator('button').evaluate_all('(els)=>els.forEach(el=>el.disabled=true)')
        self.page.keyboard.press('ArrowDown')
        self.close_menu()

    def test_08_the_reaction_pill_keeps_its_centered_plus(self):
        row = self.page.locator('.message-row.agent').last
        row.hover()
        row.locator('.reaction-menu summary').click()
        pop = row.locator('.reaction-menu .menu-popover')
        self.assertEqual(pop.evaluate('el=>getComputedStyle(el).flexDirection'), 'row')
        extra = row.get_by_role('button', name='More reactions', exact=True)
        b, s = extra.bounding_box(), extra.locator('svg').bounding_box()
        for axis, dim in [('x', 'width'), ('y', 'height')]:
            self.assertLess(abs(b[axis]+b[dim]/2 - s[axis]-s[dim]/2), .51)
        self.assertEqual(b['height'], b['width'])
        self.capture('10-reactions-preserved')
        extra.click()
        expect(self.page.get_by_role('dialog', name='More reactions', exact=True)).to_be_visible()

    def test_09_light_and_interface_sizes_keep_one_menu_style(self):
        for size in ['Compact', 'Comfortable', 'Large']:
            self.open_menu('#rail-more').get_by_role('button', name='Settings', exact=True).click()
            self.page.get_by_role('radio', name=size, exact=True).click()
            if size == 'Large': self.page.get_by_role('radio', name='Light mode', exact=True).click()
            self.page.keyboard.press('Escape')
            surface, row = self.reference()
            pop = self.open_menu('.chat-nav-row summary')
            self.assertEqual(pop.evaluate(SURFACE), surface)
            self.assertEqual(pop.locator('button').first.evaluate(ROW), row)
            self.contained(pop)
            self.capture('11-light-large' if size == 'Large' else '11-'+size.lower())
            self.close_menu()

    def test_10_phone_menus_wrap_long_labels_and_keep_touch_targets(self):
        for width in [320, 390]:
            context = self.browser.new_context(viewport={'width':width,'height':760},has_touch=True,is_mobile=True,color_scheme='dark',reduced_motion='reduce')
            page = context.new_page()
            page.set_content(self.html,wait_until='load')
            page.locator('#rail-more').click()
            pop = page.locator('.rail-more .menu-popover')
            self.assertTrue(pop.evaluate('el=>{const r=el.getBoundingClientRect(); return r.left>=8 && r.right<=innerWidth-8 && el.scrollWidth<=el.clientWidth}'))
            self.assertTrue(pop.locator('button').evaluate_all('els=>els.every(el=>el.getBoundingClientRect().height>=44)'))
            pop.locator('.menu-item-label').first.evaluate('(el)=>el.textContent="A very long menu action that must wrap rather than spill outside the phone"')
            page.wait_for_function('el=>{const r=el.getBoundingClientRect();return r.left>=8 && r.right<=innerWidth-8 && r.bottom<=innerHeight-8 && el.scrollWidth<=el.clientWidth}',arg=pop.element_handle())
            page.screenshot(path=str(OUT/f'12-phone-{width}.png'))
            context.close()

    def test_11_short_window_scrolls_the_menu_not_the_page(self):
        self.page.set_viewport_size({'width': 900, 'height': 220})
        pop = self.open_menu('#rail-more')
        self.contained(pop)
        pop.locator('button').last.focus()
        self.assertTrue(pop.locator('button').last.evaluate('el=>{const r=el.getBoundingClientRect();return r.bottom<=innerHeight-8}'))
        self.capture('13-short-window')

    def test_12_scrolling_an_anchor_and_resizing_never_leaves_a_stray_menu(self):
        self.open_menu('.chat-nav-row summary')
        self.page.locator('.side-chat-list').evaluate('(el)=>el.scrollTop=el.scrollHeight')
        expect(self.page.locator('details.menu[open]')).to_have_count(0)
        self.open_menu('#rail-more')
        self.page.set_viewport_size({'width': 1000, 'height': 700})
        expect(self.page.locator('details.menu[open]')).to_have_count(0)
        # A rerender may remove an open owner without dispatching a toggle event.
        self.open_menu('.chat-nav-row summary')
        self.page.locator('details.menu[open]').evaluate('(el)=>el.remove()')
        self.open_menu('#rail-more')
        self.close_menu()

    def test_13_popoverless_fallback_stays_visible_and_inside_the_viewport(self):
        self.page.evaluate('''() => { Object.defineProperty(HTMLElement.prototype, 'showPopover', {value:undefined,configurable:true}); Object.defineProperty(HTMLElement.prototype, 'hidePopover', {value:undefined,configurable:true}); }''')
        pop = self.open_menu('.chat-nav-row summary')
        self.contained(pop)
        self.assertEqual(pop.evaluate('el=>getComputedStyle(el).position'),'fixed')
        self.close_menu()

    def sandbox(self):
        self.page.set_content('<style>body{margin:0}iframe{border:0;width:100vw;height:100vh}</style><iframe title="Hatchling preview" sandbox="allow-scripts allow-downloads"></iframe>')
        self.page.locator('iframe').evaluate('(frame,html)=>frame.srcdoc=html', self.html)
        frame = self.page.frame_locator('iframe')
        expect(frame.locator('#message-input')).to_be_visible()
        return frame

    def test_14_real_opaque_srcdoc_preview_can_navigate_all_primary_pages(self):
        parent_url = self.page.url
        frame = self.sandbox()
        for route, heading in [('feed','Feed'),('ideas','Ideas'),('goals','Goals'),('library','All artifacts')]:
            frame.locator('#rail-'+route).click()
            expect(frame.get_by_role('heading',name=heading,level=1,exact=True)).to_be_visible()
        self.assertEqual(self.page.url,parent_url)
        self.assertEqual(frame.locator('html').evaluate('() => location.href'), 'about:srcdoc')
        self.assertEqual(frame.locator('html').evaluate('() => origin'), 'null')
        self.capture('14-sandbox-library')
        frame.locator('#rail-ideas').click()
        self.capture('06-sandbox-navigation')

    def test_15_sandbox_preview_keeps_drafts_menus_and_settings(self):
        frame = self.sandbox()
        frame.locator('#message-input').fill('Keep this while I explore the preview')
        frame.locator('#rail-ideas').click()
        frame.locator('#rail-chat').click()
        expect(frame.locator('#message-input')).to_have_value('Keep this while I explore the preview')
        frame.locator('.chat-nav-row summary').first.click()
        frame.get_by_role('button',name='Rename chat',exact=True).click()
        expect(frame.get_by_role('dialog',name='Rename side chat',exact=True)).to_be_visible()
        frame.get_by_role('button',name='Close Rename side chat',exact=True).click()
        frame.locator('#rail-more').click()
        frame.get_by_role('button',name='Settings',exact=True).click()
        expect(frame.get_by_role('dialog',name='Settings',exact=True)).to_be_visible()

    def test_16_denied_history_methods_do_not_interrupt_navigation_or_route_repair(self):
        self.page.evaluate('''() => { for(const key of ['pushState','replaceState']) history[key]=()=>{throw new DOMException('Blocked by preview host','SecurityError')}; }''')
        self.page.locator('#rail-ideas').click()
        expect(self.page.get_by_role('heading',name='Ideas',level=1)).to_be_visible()
        self.page.locator('#rail-chat').click()
        expect(self.page.locator('#message-input')).to_be_visible()
        self.route('chat/missing-conversation')
        expect(self.page.locator('.conversation-toolbar')).to_have_attribute('data-chat-id','main')

    def test_17_no_javascript_still_shows_honest_opening_instructions(self):
        context = self.browser.new_context(java_script_enabled=False,viewport={'width':900,'height':700})
        page = context.new_page()
        page.set_content(self.html,wait_until='load')
        expect(page.get_by_role('heading',name='Opening Hatchling…',exact=True)).to_be_visible()
        expect(page.locator('.preview-startup')).to_contain_text('Attachment viewers that block scripts')
        page.screenshot(path=str(OUT/'15-script-blocked-fallback.png'))
        context.close()
        expect(self.page.locator('.preview-startup')).to_have_count(0)

    def test_18_standalone_has_no_external_asset_dependencies(self):
        requests = []
        self.page.on('request',lambda request:requests.append(request.url))
        self.page.set_content(self.html,wait_until='load')
        expect(self.page.locator('.agent-portrait > img')).to_be_visible()
        self.assertEqual([url for url in requests if url.startswith(('http:','https:'))],[])
        self.assertEqual(self.page.locator('link[rel=stylesheet],script[src]').count(),0)
        self.capture('16-workspace')

if __name__ == '__main__':
    started = time.monotonic()
    suite = unittest.defaultTestLoader.loadTestsFromNames(sys.argv[1:], sys.modules[__name__]) if len(sys.argv)>1 else unittest.defaultTestLoader.loadTestsFromTestCase(MenuPreviewTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    (OUT/'menus-browser-results.json').write_text(json.dumps({'tests':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'success':result.wasSuccessful(),'duration_seconds':round(time.monotonic()-started,2),'mode':'Compiled standalone; actual opaque-origin sandboxed srcdoc frame; local controlled history denial and no-popover fallback. No browser-policy changes.'},indent=2)+'\n')
    raise SystemExit(0 if result.wasSuccessful() else 1)
