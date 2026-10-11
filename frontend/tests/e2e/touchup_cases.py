"""Example-led UI touch-ups: real DOM geometry and existing local actions."""
from pathlib import Path
import re
from playwright.sync_api import expect


class TouchupCases:
    def test_172_choice_rows_are_inside_the_bubble_and_keep_selection_behavior(self):
        self.scenario('choices')
        group = self.page.locator('.choice-bubble .choice-options')
        expect(group).to_be_visible()
        self.assertEqual(group.evaluate('el=>getComputedStyle(el).flexDirection'), 'column')
        choice = group.get_by_role('button', name='The coast walk', exact=True)
        self.assertEqual(choice.evaluate('el=>getComputedStyle(el).borderTopStyle'), 'dashed')
        choice.click()
        expect(choice).to_have_attribute('aria-pressed', 'true')
        expect(choice).to_be_disabled()
        expect(self.page.locator('.user-bubble').last).to_have_text('The coast walk')
        expect(self.page.locator('#message-input')).to_be_enabled()

    def test_173_latest_pill_tracks_a_multiline_composer(self):
        self.page.locator('.chat-scroll').evaluate('el=>el.scrollTop=20')
        jump = self.page.get_by_role('button', name='Jump to latest', exact=True)
        expect(jump).to_have_text('See more')
        for text in ['A draft', '\n'.join(['A multiline draft'] * 6)]:
            self.page.locator('#message-input').fill(text)
            self.page.wait_for_timeout(50)
            a = jump.bounding_box()
            b = self.page.locator('.composer').bounding_box()
            self.assertLessEqual(a['y'] + a['height'] + 8, b['y'])
        jump.click()
        expect(jump).to_be_hidden()
        expect(self.page.locator('#message-input')).to_have_value(text)

    def test_174_reactions_are_a_horizontal_picker_and_do_not_cover_the_file(self):
        self.page.clock.install()
        self.scenario('bundle-files')
        self.page.clock.fast_forward(16000)
        row = self.page.locator('.kind-attachment').last
        row.scroll_into_view_if_needed()
        row.hover()
        actions = row.locator('.message-actions')
        card = row.locator('.native-file-card')
        a, c = actions.bounding_box(), card.bounding_box()
        self.assertGreaterEqual(a['x'], c['x'] + c['width'])
        row.locator('.reaction-menu summary').click()
        picker = row.locator('.reaction-menu .menu-popover')
        expect(picker).to_be_visible()
        self.assertEqual(picker.evaluate('el=>getComputedStyle(el).flexDirection'), 'row')
        bounds = picker.bounding_box()
        self.assertGreater(bounds['width'], bounds['height'] * 3)
        self.page.keyboard.press('Escape')
        expect(self.page.locator('#message-input')).to_be_visible()

    def test_175_scheduled_card_keeps_its_real_evidence_action(self):
        self.scenario('mixed-stream')
        card = self.page.locator('[data-message-id="mixed-scheduled"] .scheduled-message-card')
        card.scroll_into_view_if_needed()
        expect(card.locator('header')).to_contain_text('Scheduled update')
        expect(card).to_contain_text('Your arranged weekly recap is ready')
        card.get_by_role('button', name='View evidence', exact=True).click()
        expect(self.page.get_by_role('dialog', name='Prepare a project handoff', exact=True)).to_be_visible()

    def test_176_choice_cards_and_actions_reflow_at_phone_tablet_and_desktop_widths(self):
        for width in [320, 390, 768, 1280, 1919]:
            self.page.set_viewport_size({'width': width, 'height': 844})
            self.scenario('choices')
            self.assert_no_overflow()
            bubble = self.page.locator('.choice-bubble')
            bubble.scroll_into_view_if_needed()
            box = bubble.bounding_box()
            for choice in bubble.locator('.choice-button').all():
                b = choice.bounding_box()
                self.assertGreaterEqual(b['x'], box['x'])
                self.assertLessEqual(b['x'] + b['width'], box['x'] + box['width'] + 1)
            self.assertEqual(self.page.evaluate('visualViewport.scale'), 1)

    def test_177_issue_report_still_downloads_locally_with_all_fields(self):
        self.page.locator('#rail-more').click()
        self.page.get_by_role('button', name='Report an issue', exact=True).click()
        form = self.page.locator('.issue-report-form')
        form.get_by_label('Issue title', exact=True).fill('Spacing check')
        form.get_by_label('What happened?', exact=True).fill('Example issue description.')
        form.get_by_label('Expected behavior', exact=True).fill('The layout should fit.')
        with self.page.expect_download() as download:
            form.get_by_role('button', name='Download issue note', exact=True).click()
        d = download.value
        self.assertEqual(d.suggested_filename, 'hatchling-issue.md')
        body = Path(d.path()).read_text()
        for value in ['Spacing check', 'Example issue description.', 'The layout should fit.', '1.9.0']:
            self.assertIn(value, body)

    def test_178_card_styles_are_shared_by_served_and_standalone_builds(self):
        root = Path(__file__).resolve().parents[2]
        index = (root / 'dist/index.html').read_text()
        html = (root / 'dist/hatchling-standalone.html').read_text()
        css = (root / 'dist/styles/touchup.css').read_text()
        self.assertIn('./styles/touchup.css', index)
        self.assertIn(css, html)
        self.assertEqual(self.page.locator('body').evaluate('el=>getComputedStyle(el).backgroundColor'), 'rgb(24, 24, 24)')

    def test_179_browser_dock_actions_stay_below_cards_and_text(self):
        self.page.clock.install()
        self.scenario('browser-watch')
        self.page.clock.fast_forward(5000)
        self.page.get_by_role('button', name='Open browser', exact=True).click()
        expect(self.page.locator('body')).to_have_class(re.compile('has-browser-session'))
        for selector in ['.kind-browser .chat-browser-card', '.message-row.agent.kind-text .bubble']:
            content = self.page.locator(selector).last
            content.scroll_into_view_if_needed()
            row = content.locator('..')
            actions = row.locator('.message-actions')
            expect(actions).to_be_visible()
            a, b = actions.bounding_box(), content.bounding_box()
            self.assertGreaterEqual(a['y'], b['y'] + b['height'] - 1)
            self.assertGreaterEqual(a['x'], b['x'] - 1)
            self.assertLessEqual(a['x'] + a['width'], row.bounding_box()['x'] + row.bounding_box()['width'] + 1)
        self.assert_no_overflow()
