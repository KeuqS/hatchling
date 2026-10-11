"""Capture the actual compiled UI; every transport scenario is a local simulation."""
from pathlib import Path
import os
import shutil
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'artifacts' / 'reliability'
OUT.mkdir(parents=True, exist_ok=True)
HTML = (ROOT / 'dist' / 'hatchling-standalone.html').read_text()

with sync_playwright() as p:
    executable = os.environ.get('CHROMIUM_PATH') or shutil.which('chromium') or shutil.which('chromium-browser')
    browser = p.chromium.launch(**({'executable_path': executable} if executable else {}), headless=True, args=['--no-sandbox'])
    context = browser.new_context(viewport={'width': 1600, 'height': 960}, color_scheme='dark', reduced_motion='reduce')
    errors = []
    def load():
        page = context.new_page()
        page.on('pageerror', lambda e: errors.append(str(e)))
        page.set_content(HTML, wait_until='domcontentloaded')
        expect(page.locator('#message-input')).to_be_visible()
        return page
    page = load()
    page.screenshot(path=str(OUT / '01-workspace.png'))
    page.close()
    for case in ['history-error', 'history-loading', 'history-stale', 'vm-restarting', 'slow-send', 'send-failed', 'send-unknown', 'session-expired']:
        page = load()
        page.keyboard.press('Control+Shift+D')
        page.locator(f'[data-reliability-scenario="{case}"]').click()
        if case in ['vm-restarting', 'slow-send', 'send-failed', 'send-unknown']:
            page.locator('#message-input').fill('Keep the service receipts together and let me know when you’re back.')
            page.locator('#message-input').press('Enter')
            phase = {'vm-restarting': 'queued', 'slow-send': 'sending', 'send-failed': 'failed', 'send-unknown': 'unconfirmed'}[case]
            expect(page.locator('.delivery-' + phase)).to_be_visible()
        page.screenshot(path=str(OUT / (case + '.png')))
        page.close()
    page = load()
    page.locator('#chat-link-chat-messenger').click()
    page.screenshot(path=str(OUT / 'connector-chat.png'))
    page.close()
    for case in ['history-error', 'vm-restarting', 'connector-chat']:
        page = load()
        page.set_viewport_size({'width': 390, 'height': 844})
        if page.locator('.navigation-panel').count():
            page.locator('#chat-navigation-toggle').click()
        if case == 'connector-chat':
            page.locator('#chat-navigation-toggle').click()
            expect(page.locator('.connector-chat-section')).to_be_visible()
            page.screenshot(path=str(OUT / 'mobile-connectors.png'))
            page.locator('#chat-link-chat-whatsapp').click()
        else:
            page.keyboard.press('Control+Shift+D')
            page.locator(f'[data-reliability-scenario="{case}"]').click()
            if case == 'vm-restarting':
                page.locator('#message-input').fill('Please keep this note until the workspace is back.')
                page.locator('#message-input').press('Enter')
                expect(page.locator('.delivery-queued')).to_be_visible()
        page.screenshot(path=str(OUT / ('mobile-' + case + '.png')))
        page.close()
    browser.close()
    assert errors == [], errors
    print('Captured 14 reliability screens from the compiled app; no page errors.')
