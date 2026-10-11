"""Loading/delivery UI with actual DOM interactions; all service responses are local fixtures."""
import json
import base64
import hashlib
import re
import os
from pathlib import Path
import shutil
import unittest
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[2]
ARTIFACTS = ROOT / 'artifacts' / 'reliability'

class ReliabilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.playwright = sync_playwright().start()
        executable = os.environ.get('CHROMIUM_PATH') or shutil.which('chromium') or shutil.which('chromium-browser')
        cls.browser = cls.playwright.chromium.launch(**({'executable_path': executable} if executable else {}), headless=True, args=['--no-sandbox'])
        cls.html = (ROOT / 'dist' / 'hatchling-standalone.html').read_text()
        ARTIFACTS.mkdir(parents=True, exist_ok=True)

    @classmethod
    def tearDownClass(cls):
        cls.browser.close()
        cls.playwright.stop()

    def setUp(self):
        self.context = self.browser.new_context(viewport={'width': 1600, 'height': 960}, color_scheme='dark', reduced_motion='reduce')
        self.page = self.context.new_page()
        self.page.set_default_timeout(6000)
        self.errors = []
        self.page.on('pageerror', lambda e: self.errors.append(str(e)))
        self.page.set_content(self.html, wait_until='domcontentloaded')
        expect(self.page.locator('#message-input')).to_be_visible()

    def tearDown(self):
        self.context.close()
        self.assertEqual(self.errors, [])

    def scenario(self, name):
        self.page.keyboard.press('Control+Shift+D')
        self.page.locator(f'[data-reliability-scenario="{name}"]').click()

    def send(self, text):
        self.page.locator('#message-input').fill(text)
        self.page.locator('#message-input').press('Enter')

    def origin(self):
        # This managed browser blocks all URL navigation. Inject the adapter's storage
        # dependencies into a fresh document; no policy or live origin is bypassed.
        self.refresh_with_storage({"local": {}, "session": {}})

    def refresh_with_storage(self, values=None):
        if values is None:
            values = self.page.evaluate("window.__recoveryTestStores")
        script = re.search(r'<script>(.*?)</script>', self.html, re.S).group(1)
        prelude = "window.__recoveryTestStores = " + json.dumps(values) + ";\n"
        prelude += """function recoveryTestStore(kind) { return {
            getItem: key => window.__recoveryTestStores[kind][key] ?? null,
            setItem: (key,value) => { window.__recoveryTestStores[kind][key] = value; },
            removeItem: key => { delete window.__recoveryTestStores[kind][key]; }
        }; }\n"""
        modified = prelude + script.replace('storage = window.localStorage;', "storage = recoveryTestStore('local');").replace('recoveryStorage = window.sessionStorage;', "recoveryStorage = recoveryTestStore('session');")
        digest = base64.b64encode(hashlib.sha256(modified.encode()).digest()).decode()
        html = self.html.replace(script, modified).replace(re.search(r"sha256-[A-Za-z0-9+/=]+", self.html).group(0), 'sha256-' + digest)
        self.page.close()
        self.page = self.context.new_page()
        self.page.set_default_timeout(6000)
        self.page.on('pageerror', lambda e: self.errors.append(str(e)))
        self.page.set_content(html, wait_until='domcontentloaded')
        expect(self.page.locator('#message-input')).to_be_visible()

    def attach(self, name='receipt.txt', content=b'Example receipt, not a real purchase.'):
        self.page.get_by_label('Add attachments or voice', exact=True).click()
        with self.page.expect_file_chooser() as chooser:
            self.page.get_by_role('button', name='Attach a file', exact=True).click()
        chooser.value.set_files({'name': name, 'mimeType': 'text/plain', 'buffer': content})

    def user_messages(self, text):
        return self.page.locator('.message-row.user[data-message-id]').filter(has_text=text)

    def test_01_connector_section_is_bottom_anchored_and_separate(self):
        expect(self.page.locator('.side-chat-list').get_by_text('WhatsApp', exact=True)).to_have_count(0)
        expect(self.page.locator('.side-chat-list').get_by_text('Messenger', exact=True)).to_have_count(0)
        expect(self.page.locator('#connector-chat-list .connector-chat-row')).to_have_count(2)
        self.assertGreater(self.page.locator('.connector-chat-section').bounding_box()['y'], 650)
        self.page.get_by_role('button', name='Side chats', exact=True).click()
        expect(self.page.locator('#connector-chat-list')).to_be_visible()
        self.page.locator('#connector-chat-toggle').click()
        expect(self.page.locator('#connector-chat-list')).to_be_hidden()
        expect(self.page.locator('.connector-unread-count')).to_have_text('1')
        self.page.locator('#connector-chat-toggle').press('Enter')
        expect(self.page.locator('#connector-chat-list')).to_be_visible()

    def test_02_connector_chat_has_its_own_context_and_connection_state(self):
        self.page.locator('#chat-link-chat-messenger').click()
        expect(self.page.locator('.connector-conversation-banner')).to_contain_text('Connector chat')
        expect(self.page.locator('.connector-conversation-banner')).to_contain_text('Updates paused')
        expect(self.page.locator('#message-input')).to_have_count(0)
        expect(self.page.locator('.composer-caption')).to_contain_text('connector chat')
        self.page.locator('#manage-chat-connection').click()
        expect(self.page.locator('.settings-dialog')).to_be_visible()

    def test_03_failed_history_keeps_draft_and_retries_in_place(self):
        self.page.locator('#message-input').fill('Keep this unfinished thought')
        self.scenario('history-error')
        expect(self.page.locator('.history-placeholder')).to_contain_text('couldn’t load')
        expect(self.page.locator('.send-button')).to_be_disabled()
        self.page.locator('#retry-chat-history').click()
        expect(self.page.locator('.chat-scroll')).to_have_attribute('aria-busy', 'true')
        expect(self.page.locator('.history-placeholder')).to_have_count(0, timeout=4000)
        expect(self.page.locator('#message-input')).to_have_value('Keep this unfinished thought')
        expect(self.page.locator('.send-button')).to_be_enabled()

    def test_04_cached_history_stays_readable_and_retry_preserves_it(self):
        count = self.page.locator('[data-message-id]').count()
        self.scenario('history-stale')
        expect(self.page.locator('.history-inline')).to_contain_text('last loaded chat')
        self.assertEqual(self.page.locator('[data-message-id]').count(), count)
        self.page.locator('#message-input').fill('Draft while history is unavailable')
        self.page.locator('#retry-chat-history').click()
        self.assertEqual(self.page.locator('[data-message-id]').count(), count)
        expect(self.page.locator('.history-inline')).to_have_count(0)
        expect(self.page.locator('#message-input')).to_have_value('Draft while history is unavailable')

    def test_05_initial_loader_is_distinct_from_an_empty_conversation(self):
        self.scenario('history-loading')
        expect(self.page.locator('.history-skeleton')).to_be_visible()
        expect(self.page.locator('.chat-scroll')).to_have_attribute('aria-busy', 'true')
        self.page.locator('#message-input').fill('Draft during loading')
        expect(self.page.locator('.send-button')).to_be_disabled()
        expect(self.page.locator('.history-skeleton')).to_have_attribute('aria-hidden', 'true')

    def test_06_slow_send_stays_visible_and_preserves_new_typing(self):
        self.scenario('slow-send')
        self.send('A visible sending message')
        expect(self.page.locator('.delivery-sending')).to_contain_text('A visible sending message')
        self.assertLess(float(self.page.locator('.delivery-sending .bubble').evaluate('(el) => getComputedStyle(el).opacity')), .8)
        expect(self.page.locator('#message-input')).to_have_value('')
        self.page.locator('#message-input').press('Enter')
        self.page.locator('#message-input').fill('A newer draft')
        expect(self.user_messages('A visible sending message')).to_have_count(1, timeout=10000)
        expect(self.page.locator('.outbox-row')).to_have_count(0)
        expect(self.page.locator('#message-input')).to_have_value('A newer draft')
        expect(self.page.locator('#message-input')).to_be_focused()
        expect(self.page.get_by_label('Message sent', exact=True)).to_be_visible()

    def test_07_definite_failure_has_inline_retry_without_duplication(self):
        self.scenario('send-failed'); self.send('A retryable message')
        expect(self.page.locator('.delivery-failed')).to_contain_text('Not sent')
        self.page.locator('#message-input').fill('Do not overwrite this draft')
        self.page.locator('.delivery-failed').get_by_role('button', name='Retry', exact=True).click()
        expect(self.user_messages('A retryable message')).to_have_count(1)
        expect(self.page.locator('.outbox-row')).to_have_count(0)
        expect(self.page.locator('#message-input')).to_have_value('Do not overwrite this draft')

    def test_08_uncertain_delivery_checks_receipt_without_retry(self):
        self.scenario('send-unknown'); self.send('Reconcile me only once')
        row = self.page.locator('.delivery-unconfirmed')
        expect(row).to_contain_text('Delivery not confirmed')
        expect(row.get_by_role('button', name='Retry', exact=True)).to_have_count(0)
        expect(row.get_by_role('button', name='Remove', exact=True)).to_have_count(0)
        row.get_by_role('button', name='Check status', exact=True).click()
        expect(self.user_messages('Reconcile me only once')).to_have_count(1)
        expect(self.page.locator('.outbox-row')).to_have_count(0)

    def test_09_offline_queue_drains_once_in_order_after_reconnect(self):
        self.scenario('offline'); self.send('First queued note'); self.send('Second queued note')
        expect(self.page.locator('.delivery-queued')).to_have_count(2)
        expect(self.user_messages('First queued note')).to_have_count(0)
        self.page.locator('#reconnect-chat').click()
        expect(self.page.locator('.connection-banner')).to_have_count(0)
        expect(self.page.locator('.outbox-row')).to_have_count(0)
        rows = self.page.locator('.message-row.user[data-message-id]').all_text_contents()
        self.assertLess(next(i for i, text in enumerate(rows) if 'First queued note' in text), next(i for i, text in enumerate(rows) if 'Second queued note' in text))

    def test_10_navigation_mid_send_never_moves_the_message_or_new_draft(self):
        self.scenario('slow-send'); self.send('Stay in the original chat')
        self.page.locator('#chat-link-main').click()
        self.page.locator('#message-input').fill('Main-only draft')
        expect(self.page.locator('#chat-link-reference .chat-outbox-count')).to_be_visible()
        self.page.wait_for_timeout(7200)
        expect(self.user_messages('Stay in the original chat')).to_have_count(0)
        expect(self.page.locator('#message-input')).to_have_value('Main-only draft')
        self.page.locator('#chat-link-reference').click()
        expect(self.user_messages('Stay in the original chat')).to_have_count(1)
        expect(self.page.locator('#message-input')).to_have_value('')

    def test_11_partial_file_failure_only_retries_the_file(self):
        self.scenario('attachment-failed')
        self.attach()
        expect(self.page.locator('.attachment-chip')).to_contain_text('receipt.txt')
        self.send('Text that must not repeat')
        expect(self.user_messages('Text that must not repeat')).to_have_count(1)
        expect(self.page.locator('.delivery-failed')).to_contain_text('receipt.txt')
        self.page.locator('.delivery-failed').get_by_role('button', name='Retry', exact=True).click()
        expect(self.page.locator('.outbox-row')).to_have_count(0)
        expect(self.user_messages('Text that must not repeat')).to_have_count(1)
        expect(self.page.locator('.message-row.user.kind-attachment')).to_have_count(1)

    def test_12_failed_file_preparation_is_inline_and_keeps_the_draft(self):
        self.page.locator('#message-input').fill('Keep my draft')
        self.attach('too-large.txt', b'x' * 2000001)
        expect(self.page.locator('.file-import-status.failed')).to_contain_text('under 2 MB')
        expect(self.page.locator('#message-input')).to_have_value('Keep my draft')
        self.page.get_by_role('button', name='Remove too-large.txt', exact=True).click()
        expect(self.page.locator('.file-import-status')).to_have_count(0)

    def test_13_expired_session_retains_draft_until_reconnected(self):
        self.scenario('session-expired')
        self.page.locator('#message-input').fill('A retained session draft')
        expect(self.page.locator('.send-button')).to_be_disabled()
        expect(self.page.locator('.agent-status-label')).to_have_text('Session needs attention')
        self.page.locator('#reconnect-chat').click()
        expect(self.page.locator('.send-button')).to_be_enabled()
        expect(self.page.locator('#message-input')).to_have_value('A retained session draft')

    def test_14_archived_chats_require_restore_before_sending(self):
        self.page.locator('.chat-nav-row.active summary').click()
        self.page.locator('.chat-nav-row.active').get_by_role('button', name='Archive chat', exact=True).click()
        self.page.locator('#message-input').fill('Restore first')
        expect(self.page.locator('.send-button')).to_be_disabled()
        self.page.locator('#restore-current-chat').click()
        expect(self.page.locator('.send-button')).to_be_enabled()
        expect(self.page.locator('#message-input')).to_have_value('Restore first')

    def test_15_mobile_error_and_pending_controls_stay_inside_viewport(self):
        for width in [320, 390, 768, 1280]:
            self.page.set_viewport_size({'width': width, 'height': 844})
            self.scenario('history-error')
            self.assertTrue(self.page.evaluate('document.documentElement.scrollWidth <= innerWidth'))
            for selector in ['.history-placeholder', '.composer', '.history-state-actions']:
                rect = self.page.locator(selector).bounding_box()
                self.assertGreaterEqual(rect['x'], 0)
                self.assertLessEqual(rect['x'] + rect['width'], width + 1)
            self.scenario('ready')
        self.page.set_viewport_size({'width': 390, 'height': 844})
        self.scenario('send-failed'); self.send('A long mobile message that still leaves its recovery controls accessible')
        expect(self.page.locator('.delivery-failed')).to_be_visible()
        for control in self.page.locator('.outbox-actions button').all():
            rect = control.bounding_box(); self.assertLessEqual(rect['x'] + rect['width'], 391)
        self.page.screenshot(path=str(ARTIFACTS / 'mobile-failed.png'))

    def test_16_reduced_motion_disables_skeleton_and_spinner_animation(self):
        self.scenario('history-loading')
        self.assertEqual(self.page.locator('.history-skeleton').evaluate('(el) => getComputedStyle(el).animationName'), 'none')
        self.scenario('ready'); self.scenario('slow-send'); self.send('Reduced motion')
        expect(self.page.locator('.delivery-sending')).to_be_visible()
        self.assertEqual(self.page.locator('.delivery-sending .is-spinning svg').evaluate('(el) => getComputedStyle(el).animationName'), 'none')

    def test_17_tab_recovery_restores_drafts_and_requires_queue_review(self):
        self.origin()
        self.page.locator('#chat-link-main').click()
        self.page.locator('#message-input').fill('A draft saved across refresh')
        self.page.locator('#chat-link-reference').click()
        self.scenario('offline'); self.send('Recovered queued message')
        expect(self.page.locator('.outbox-notice')).to_contain_text('saved for this tab')
        self.refresh_with_storage()
        expect(self.page.locator('#resume-outbox')).to_be_visible()
        expect(self.user_messages('Recovered queued message')).to_have_count(0)
        self.page.locator('#resume-outbox').click()
        expect(self.user_messages('Recovered queued message')).to_have_count(1)
        self.page.locator('#chat-link-main').click()
        expect(self.page.locator('#message-input')).to_have_value('A draft saved across refresh')

    def test_18_reload_during_dispatch_never_turns_unknown_into_automatic_retry(self):
        self.origin(); self.scenario('slow-send'); self.send('Do not replay after reload')
        expect(self.page.locator('.delivery-sending')).to_be_visible()
        self.refresh_with_storage()
        expect(self.page.locator('.delivery-unconfirmed')).to_be_visible()
        expect(self.user_messages('Do not replay after reload')).to_have_count(0)
        expect(self.page.locator('.delivery-unconfirmed').get_by_role('button', name='Retry', exact=True)).to_have_count(0)

    def test_19_vm_restart_queues_and_resumes_without_user_resending(self):
        self.scenario('vm-restarting'); self.send('Wait through the restart')
        expect(self.page.locator('.delivery-queued')).to_be_visible()
        expect(self.page.locator('.agent-status-label')).to_have_text('Workspace restarting')
        expect(self.user_messages('Wait through the restart')).to_have_count(1, timeout=13000)
        expect(self.page.locator('.outbox-row')).to_have_count(0)
        expect(self.page.locator('.connection-banner')).to_have_count(0)

    def test_20_edit_failed_message_preserves_a_newer_draft(self):
        self.scenario('send-failed'); self.send('Earlier failed message')
        expect(self.page.locator('.delivery-failed')).to_be_visible()
        self.page.locator('#message-input').fill('Newer draft')
        self.page.locator('.delivery-failed').get_by_role('button', name='Edit', exact=True).click()
        expect(self.page.locator('#message-input')).to_have_value('Earlier failed message\n\nNewer draft')
        expect(self.page.locator('.outbox-row')).to_have_count(0)
        expect(self.user_messages('Earlier failed message')).to_have_count(0)

    def test_21_editing_rejected_text_keeps_its_file_in_the_draft(self):
        self.scenario('send-failed'); self.attach()
        expect(self.page.locator('.attached-files')).to_contain_text('receipt.txt')
        self.send('Please review the attached receipt')
        expect(self.page.locator('.delivery-failed')).to_be_visible()
        self.page.locator('.delivery-failed').get_by_role('button', name='Edit', exact=True).click()
        expect(self.page.locator('#message-input')).to_have_value('Please review the attached receipt')
        expect(self.page.locator('.attached-files')).to_contain_text('receipt.txt')
        expect(self.page.locator('.outbox-row')).to_have_count(0)
        expect(self.user_messages('Please review the attached receipt')).to_have_count(0)

    def test_22_delete_is_disabled_while_messages_are_unresolved(self):
        self.scenario('send-unknown'); self.send('Keep my delivery status available')
        expect(self.page.locator('.delivery-unconfirmed')).to_be_visible()
        self.page.get_by_label('Manage chat Organize a sample project', exact=True).click()
        expect(self.page.get_by_role('button', name='Delete chat', exact=True)).to_be_disabled()

    def test_23_dismissing_unknown_warns_it_is_not_cancellation_and_never_resends(self):
        self.scenario('send-unknown'); self.send('Uncertain original')
        expect(self.page.locator('.delivery-unconfirmed')).to_be_visible()
        self.send('Next message after review')
        expect(self.page.locator('.delivery-queued')).to_be_visible()
        self.page.locator('.delivery-unconfirmed').get_by_role('button', name='Dismiss', exact=True).click()
        expect(self.page.get_by_role('dialog')).to_contain_text('does not cancel')
        expect(self.page.get_by_role('dialog')).to_contain_text('may still arrive')
        self.page.get_by_role('button', name='Dismiss warning', exact=True).click()
        expect(self.user_messages('Next message after review')).to_have_count(1)
        expect(self.user_messages('Uncertain original')).to_have_count(0)
        expect(self.page.locator('.outbox-row')).to_have_count(0)

if __name__ == '__main__':
    unittest.main(verbosity=2)
