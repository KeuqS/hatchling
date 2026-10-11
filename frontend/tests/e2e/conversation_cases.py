"""v7 reference interactions. Tests use user-facing DOM, not injected app state."""
import re
from pathlib import Path
from playwright.sync_api import expect

class ConversationCases:
    def watch_browser(self):
        self.page.clock.install()
        self.scenario('browser-watch')
        self.page.get_by_role('button', name='Open browser', exact=True).click()
        return self.page.locator('.browser-session-dock')

    def finished_bundle(self):
        self.page.clock.install()
        self.scenario('bundle-files')
        self.page.clock.fast_forward(14000)
        self.route('chat/reference')
        self.page.locator('.chat-scroll').evaluate('(el)=>el.scrollTop=el.scrollHeight')
        return self.page.locator('.native-file-card').filter(has_text='Sample handoff tarball')

    def test_105_side_chat_disclosure_rotates_and_hides_only_the_list(self):
        control=self.page.get_by_role('button', name='Side chats', exact=True)
        self.page.locator('#message-input').fill('Keep my draft')
        count=self.page.locator('.chat-nav-row').count()
        expect(control).to_have_attribute('aria-expanded','true')
        down=control.locator('svg').evaluate('(el)=>getComputedStyle(el).transform')
        control.click()
        expect(control).to_have_attribute('aria-expanded','false')
        self.assertTrue(self.page.locator('#side-chat-accordion').evaluate('(el)=>el.inert'))
        self.assertLess(self.page.locator('#side-chat-accordion').bounding_box()['height'],2)
        self.assertEqual(self.page.locator('.chat-nav-row').count(), count)
        expect(self.page.locator('.navigation-panel')).to_be_visible()
        expect(self.page.locator('.main-chat-link')).to_be_visible()
        self.assertNotEqual(control.locator('svg').evaluate('(el)=>getComputedStyle(el).transform'),down)
        control.click()
        expect(control).to_have_attribute('aria-expanded','true')
        expect(self.page.locator('#message-input')).to_have_value('Keep my draft')
        self.assertEqual(control.locator('svg').evaluate('(el)=>getComputedStyle(el).transform'),down)

    def test_106_disclosure_animation_has_intermediate_geometry_and_reduced_motion(self):
        self.page.emulate_media(reduced_motion='no-preference')
        node=self.page.locator('#side-chat-accordion')
        original=node.bounding_box()['height']
        control=self.page.get_by_role('button',name='Side chats',exact=True)
        # Sample the real CSS transitions at a known time, not 70 ms of VM wall time.
        # Keep the real user click; capture/pause immediately after its native handler.
        control.evaluate("""button => button.addEventListener('click', () => {
            const accordion = document.getElementById('side-chat-accordion');
            accordion.__testedTransitions = accordion.getAnimations();
            for (const animation of accordion.__testedTransitions) {
                animation.pause();
                animation.currentTime = 70;
            }
        }, {once:true})""")
        control.click()
        self.assertGreater(node.evaluate('el => el.__testedTransitions.length'), 0)
        middle=node.bounding_box()['height']
        self.assertGreater(middle,0);self.assertLess(middle,original)
        node.evaluate('el => el.__testedTransitions.forEach(animation => animation.finish())')
        self.assertLess(node.bounding_box()['height'],2)
        self.page.emulate_media(reduced_motion='reduce')
        self.page.get_by_role('button',name='Side chats',exact=True).click()
        self.assertAlmostEqual(node.bounding_box()['height'],original,delta=2)

    def test_107_hover_recency_and_ellipsis_share_space_without_clipping(self):
        row=self.page.locator('.chat-nav-row').first
        self.page.mouse.move(900,400)
        expect(row.locator('.chat-recency')).to_be_hidden()
        row.hover()
        expect(row.locator('.chat-recency')).to_be_visible()
        expect(row.locator('.chat-recency')).to_have_text('35m')
        expect(row.locator('summary')).to_be_visible()
        right=row.bounding_box()['x']+row.bounding_box()['width']
        for node in [row.locator('.chat-recency'),row.locator('summary')]:
            box=node.bounding_box();self.assertLessEqual(box['x']+box['width'],right+1)
        self.page.mouse.move(900,400)
        expect(row.locator('.chat-recency')).to_be_hidden()

    def test_108_sidebar_drag_resize_keyboard_and_click_close(self):
        node=self.page.get_by_role('separator',name='Side chats width',exact=True)
        initial=self.page.locator('.navigation-panel').bounding_box()['width']
        box=node.bounding_box();self.page.mouse.move(box['x']+4,400);self.page.mouse.down();self.page.mouse.move(box['x']+84,400,steps=8);self.page.mouse.up()
        self.assertAlmostEqual(self.page.locator('.navigation-panel').bounding_box()['width'],initial+80,delta=2)
        node.focus();node.press('ArrowLeft')
        self.assertAlmostEqual(self.page.locator('.navigation-panel').bounding_box()['width'],initial+64,delta=2)
        node.click()
        expect(self.page.locator('.navigation-panel')).to_have_count(0)
        self.page.get_by_role('button',name='Toggle navigation',exact=True).click()
        expect(self.page.locator('.navigation-panel')).to_be_visible()

    def test_109_profile_prompt_preserves_draft_and_does_not_submit(self):
        self.page.locator('#message-input').fill('A thought to keep.')
        count=self.page.locator('.message-row').count()
        self.page.locator('.avatar-edit-menu summary').click()
        self.page.get_by_role('button',name='Edit name',exact=True).click()
        expect(self.page.locator('#message-input')).to_have_value('A thought to keep.\nChange my name to ')
        expect(self.page.locator('#message-input')).to_be_focused()
        self.assertEqual(self.page.locator('.message-row').count(),count)
        expect(self.page.locator('dialog')).to_have_count(0)
        expect(self.page.locator('.composer .send-button')).to_be_visible()
        expect(self.page.get_by_role('button',name='Voice dictation preview',exact=True)).to_be_visible()

    def test_110_profile_avatar_prefill_and_composer_native_sizing(self):
        self.page.locator('.avatar-edit-menu summary').click()
        self.page.get_by_role('button',name='Change avatar',exact=True).click()
        expect(self.page.locator('#message-input')).to_have_value('Change my avatar to ')
        self.assertAlmostEqual(float(self.page.locator('#message-input').evaluate('(el)=>parseFloat(getComputedStyle(el).fontSize)')),17.92,delta=.1)
        self.assertEqual(self.page.evaluate('visualViewport.scale'),1)
        self.assertEqual(self.page.locator('html').evaluate('(el)=>getComputedStyle(el).zoom'),'1')
        self.assertGreaterEqual(self.page.locator('.composer').bounding_box()['height'],54)

    def test_111_activity_inner_split_is_resizable_and_remembered(self):
        dialog=self.active_bundle()
        divider=self.page.get_by_role('separator',name='Activity timeline width',exact=True)
        old=dialog.locator('.activity-timeline').bounding_box()['width']
        divider.focus();divider.press('ArrowRight');divider.press('ArrowRight')
        self.assertAlmostEqual(dialog.locator('.activity-timeline').bounding_box()['width'],old+32,delta=1)
        self.page.keyboard.press('Escape')
        self.page.get_by_role('button',name='Open evidence: Bundle sample files',exact=True).click()
        self.assertAlmostEqual(dialog.locator('.activity-timeline').bounding_box()['width'],old+32,delta=1)
        size=self.page.get_by_role('separator',name='Activity window width',exact=True)
        oldw=dialog.bounding_box()['width'];size.focus();size.press('ArrowLeft')
        self.assertAlmostEqual(dialog.bounding_box()['width'],oldw-16,delta=1)
        self.assertAlmostEqual(dialog.bounding_box()['x']+dialog.bounding_box()['width']/2,1919/2,delta=1)

    def test_112_activity_shows_both_live_lanes_and_keeps_reading_position(self):
        dialog=self.active_bundle()
        self.assertGreaterEqual(dialog.locator('.pending-entry').count(),2)
        expect(dialog.locator('.child-lane h3 .activity-spinner')).to_be_visible()
        self.page.locator('#timeline-bundle-sha-failed').click()
        nav=dialog.locator('.activity-timeline')
        nav.evaluate('(el)=>el.scrollTop=70')
        old=nav.evaluate('(el)=>el.scrollTop')
        self.page.clock.fast_forward(7000)
        expect(self.page.locator('#timeline-bundle-sha-failed')).to_have_attribute('aria-pressed','true')
        self.assertAlmostEqual(nav.evaluate('(el)=>el.scrollTop'),old,delta=1)
        self.assertGreaterEqual(dialog.locator('.child-lane .timeline-entry').count(),9)
        expect(dialog.locator('#timeline-verify-current')).to_have_attribute('data-state','running')
        self.page.clock.fast_forward(7000)
        expect(self.page.locator('#timeline-bundle-sha-failed')).to_have_attribute('aria-pressed','true')
        expect(dialog.locator('.pending-entry')).to_have_count(0)

    def test_113_follow_live_is_explicit_and_manual_selection_cancels_it(self):
        dialog=self.active_bundle()
        follow=dialog.get_by_role('button',name='Follow live',exact=True)
        expect(follow).to_have_attribute('aria-pressed','false')
        follow.click();expect(follow).to_have_attribute('aria-pressed','true')
        self.page.clock.fast_forward(7000)
        expect(self.page.locator('#timeline-verify-current')).to_have_attribute('aria-pressed','true')
        self.page.locator('#timeline-bundle-sha-failed').click()
        expect(follow).to_have_attribute('aria-pressed','false')

    def test_114_file_card_has_exact_download_menu_and_real_gzip_bytes(self):
        card=self.finished_bundle()
        expect(card).to_be_visible();expect(card.locator('.native-file-type')).to_have_text('GZ')
        card.locator('summary').click()
        menu=card.locator('.menu-popover')
        self.assertEqual(menu.get_by_role('button').all_text_contents(),['Download'])
        with self.page.expect_download() as download:
            menu.get_by_role('button',name='Download',exact=True).click()
        self.assertTrue(download.value.suggested_filename.endswith('.tar.gz'))
        self.assertEqual(Path(download.value.path()).read_bytes()[:2],b'\x1f\x8b')
        expect(self.page.locator('.choice-options')).to_have_count(0)

    def test_115_file_reactions_toolbar_sets_one_reaction_and_reply_prefills(self):
        card=self.finished_bundle();row=card.locator('..')
        row.hover();row.locator('.reaction-menu summary').click()
        menu=row.locator('.reaction-menu .menu-popover')
        self.assertEqual(menu.get_by_role('button').count(),7)
        menu.get_by_role('button',name='❤️',exact=True).click()
        expect(row.get_by_role('button',name='Remove ❤️ reaction',exact=True)).to_be_visible()
        row.hover();row.locator('.reaction-menu summary').click();row.get_by_role('button',name='👍',exact=True).click()
        expect(row.locator('.reaction-chip')).to_have_count(1)
        row.get_by_role('button',name=re.compile('Reply to message')).click()
        expect(self.page.locator('#message-input')).to_have_value(re.compile(r'^> '))
        self.assertIn('\n\n',self.page.locator('#message-input').input_value())

    def test_116_browser_inline_card_expands_to_30_70_without_modal_backdrop(self):
        dock=self.watch_browser()
        self.assertAlmostEqual(dock.bounding_box()['width']/1919,.70,delta=.01)
        expect(self.page.locator('#message-input')).to_be_enabled()
        expect(self.page.locator('dialog:modal')).to_have_count(0)
        expect(self.page.locator('.chat-browser-card')).to_be_visible()
        expect(dock.get_by_role('button',name='Take control of the browser',exact=True)).to_be_visible()
        expect(dock).to_contain_text('Scripted preview')
        self.page.locator('#message-input').fill('Keep this browser-side draft')
        dock.get_by_role('button',name='Close browser session',exact=True).click()
        expect(self.page.locator('#message-input')).to_have_value('Keep this browser-side draft')
        expect(self.page.locator('.chat-browser-card header')).to_contain_text('Working')

    def test_117_browser_card_and_workspace_update_to_the_same_recorded_frame(self):
        dock=self.watch_browser()
        self.page.clock.run_for(5000)
        expect(dock.locator('.browser-frame')).to_have_attribute('data-sequence','2')
        expect(self.page.locator('.chat-browser-card .browser-frame')).to_have_attribute('data-sequence','2')
        expect(dock.locator('.recorded-browser-frame')).to_be_visible()
        expect(dock).to_contain_text('Waiting for you')
        self.page.clock.run_for(5000)
        expect(dock.locator('.browser-frame')).to_have_attribute('data-sequence','2')

    def test_118_browser_takeover_pauses_frames_and_returns_from_fresh_state(self):
        dock=self.watch_browser()
        dock.get_by_role('button',name='Take control of the browser',exact=True).click()
        expect(dock.get_by_role('form',name='Local takeover controls')).to_be_visible()
        self.page.clock.run_for(6000)
        expect(dock.locator('.browser-frame')).to_have_attribute('data-sequence','0')
        dock.get_by_role('searchbox',name='Sample browser search').fill('my local query')
        dock.get_by_role('button',name='Search sample page',exact=True).click()
        expect(dock.locator('.sample-searchbox')).to_contain_text('my local query')
        dock.get_by_role('button',name='Return control to agent',exact=True).click()
        self.page.clock.run_for(2500)
        expect(dock.locator('.browser-frame')).to_have_attribute('data-sequence','1')

    def test_119_takeover_challenge_requires_explicit_completion_before_handback(self):
        dock=self.watch_browser();self.page.clock.run_for(5000)
        dock.get_by_role('button',name='Take control of the browser',exact=True).click()
        expect(dock.get_by_role('button',name='Return control to agent',exact=True)).to_be_disabled()
        dock.get_by_role('button',name='Complete sample challenge',exact=True).click()
        expect(dock.get_by_role('button',name='Return control to agent',exact=True)).to_be_enabled()
        dock.get_by_role('button',name='Return control to agent',exact=True).click()
        self.page.clock.run_for(2500)
        expect(dock).to_contain_text('Completed')
        expect(dock.locator('.sample-result')).to_contain_text('not a web search')

    def test_120_stopped_browser_keeps_preview_and_has_no_live_actions(self):
        dock=self.watch_browser();self.page.clock.run_for(5000)
        dock.get_by_role('button',name='Stop',exact=True).click()
        expect(dock).to_contain_text('Stopped')
        expect(dock.get_by_role('button',name='Take control of the browser',exact=True)).to_have_count(0)
        dock.get_by_role('button',name='Close browser session',exact=True).click()
        expect(self.page.get_by_role('button',name='Open preview',exact=True)).to_be_visible()
        self.page.get_by_role('button',name='Open preview',exact=True).click()
        expect(self.page.locator('.browser-session-dock .recorded-browser-frame')).to_be_visible()
        expect(self.page.locator('.browser-session-footer')).to_contain_text('read-only')

    def test_121_browser_resize_and_avatar_restore_preserve_chat_draft(self):
        dock=self.watch_browser()
        self.page.locator('#message-input').fill('Not sent yet')
        splitter=self.page.get_by_role('separator',name='Browser workspace width',exact=True)
        old=dock.bounding_box()['width'];splitter.focus();splitter.press('ArrowRight')
        self.assertAlmostEqual(dock.bounding_box()['width'],old-16,delta=1)
        self.page.get_by_role('button',name='Show agent status',exact=True).click()
        expect(dock).to_have_count(0)
        expect(self.page.locator('.context-panel')).to_be_visible()
        expect(self.page.locator('#message-input')).to_have_value('Not sent yet')

    def test_122_side_chat_delete_uses_top_center_confirmation_toast(self):
        row=self.page.locator('.chat-nav-row').first
        row.hover();row.locator('summary').click()
        row.get_by_role('button',name='Delete chat',exact=True).click()
        self.page.get_by_role('dialog',name='Delete this side chat?',exact=True).get_by_role('button',name='Delete chat',exact=True).click()
        toast=self.page.locator('.toast').filter(has_text='Side chat deleted')
        expect(toast).to_be_visible()
        box=toast.bounding_box();self.assertLess(box['y'],40)
        self.assertAlmostEqual(box['x']+box['width']/2,1919/2,delta=2)
        expect(toast.locator('svg').first).to_be_visible()

    def test_123_browser_and_activity_remain_operable_at_320_pixels(self):
        self.page.set_viewport_size({'width':320,'height':740})
        dock=self.watch_browser();self.page.clock.run_for(5000)
        self.assert_no_overflow()
        dock.get_by_role('button',name='Take control of the browser',exact=True).click()
        expect(dock.get_by_role('button',name='Complete sample challenge',exact=True)).to_be_visible()
        self.assert_no_overflow()
        dock.get_by_role('button',name='Close browser session',exact=True).click()
        self.scenario('bundle-files')
        self.page.get_by_role('button',name='Show agent status',exact=True).click()
        self.page.get_by_role('button',name='Open evidence: Bundle sample files',exact=True).click()
        divider=self.page.get_by_role('separator',name='Activity timeline height',exact=True)
        expect(divider).to_have_attribute('aria-orientation','horizontal')
        divider.focus();divider.press('ArrowDown')
        self.assert_no_overflow()

    def test_124_stale_browser_commands_do_not_change_an_open_activity_report(self):
        self.watch_browser()
        self.page.get_by_role('button',name='Close browser session',exact=True).click()
        self.page.get_by_role('button',name='Open evidence: Search the web',exact=True).click()
        self.page.clock.run_for(2500)
        self.page.locator('#timeline-watch-open').click()
        self.page.clock.run_for(3000)
        expect(self.page.locator('#timeline-watch-open')).to_have_attribute('aria-pressed','true')
        expect(self.page.locator('#activity-selected-report')).to_contain_text('No external request was made')

    def test_125_rejected_name_keeps_composer_text_for_correction(self):
        text='Change my name to '+'x'*61
        self.page.locator('#message-input').fill(text)
        self.page.locator('#message-input').press('Enter')
        expect(self.page.locator('.delivery-failed')).to_contain_text(text)
        self.page.locator('.delivery-failed').get_by_role('button', name='Edit', exact=True).click()
        expect(self.page.locator('#message-input')).to_have_value(text)
        expect(self.page.locator('.agent-profile h2')).to_have_text('hatchling')

    def test_126_message_avatar_attachment_is_explicit_and_samples_color(self):
        import base64
        data=self.page.evaluate("""() => {const c=document.createElement('canvas');c.width=16;c.height=16;const x=c.getContext('2d');x.fillStyle='#24aacc';x.fillRect(0,0,16,16);return c.toDataURL().split(',')[1];}""")
        self.page.locator('.avatar-edit-menu summary').click()
        self.page.get_by_role('button',name='Change avatar',exact=True).click()
        self.page.locator('.composer summary').click()
        with self.page.expect_file_chooser() as chooser:
            self.page.get_by_role('button',name='Add a photo',exact=True).click()
        chooser.value.set_files({'name':'avatar.png','mimeType':'image/png','buffer':base64.b64decode(data)})
        expect(self.page.locator('.attached-files')).to_contain_text('avatar.png')
        self.page.locator('#message-input').press('Enter')
        expect(self.page.locator('.agent-portrait > img')).to_have_attribute('src','data:image/png;base64,' + data)
        self.assertEqual(self.page.locator('html').get_attribute('data-accent'),'white')
        self.page.keyboard.press('Control+,')
        self.page.get_by_role('radio',name='Match my avatar',exact=True).click()
        self.assertEqual(self.page.locator('html').evaluate('(el)=>el.style.getPropertyValue("--theme-color")'),'#24aacc')
