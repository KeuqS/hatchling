"""Native UI touch-up screenshots; built app and synthetic fixtures only."""
from pathlib import Path
import sys, json, os, shutil
from playwright.sync_api import sync_playwright
root=Path(__file__).resolve().parents[2]
out=Path(sys.argv[1]) if len(sys.argv)>1 else root/'artifacts/ui-touchup';out.mkdir(parents=True,exist_ok=True)
with sync_playwright() as p:
    b=p.chromium.launch(executable_path=os.environ.get('CHROMIUM_PATH') or shutil.which('chromium'),args=['--no-sandbox'] if os.geteuid()==0 else [])
    c=b.new_context(viewport={'width':1919,'height':938},color_scheme='dark',reduced_motion='reduce')
    q=c.new_page(); errors=[]; q.on('pageerror',lambda e:errors.append(str(e)))
    html=(root/'dist/hatchling-standalone.html').read_text()
    def reset():
        q.set_content(html,wait_until='domcontentloaded');q.locator('#message-input').wait_for();q.wait_for_timeout(120)
    def shot(name):
        q.mouse.move(2,2);q.wait_for_timeout(80);q.screenshot(path=str(out/(name+'.png')))
    def route(name):
        q.evaluate('(hash)=>location.hash=hash',name);q.wait_for_timeout(100)
    def scenario(name):
        q.keyboard.press('Control+Shift+D');q.locator(f'[data-scenario="{name}"]').click();q.wait_for_timeout(100)
    reset(); shot('01-workspace')
    q.get_by_role('tab',name='Activity',exact=True).click();shot('02-activity-panel')
    q.locator('.chat-scroll').evaluate('(el)=>el.scrollTop-=420');q.wait_for_timeout(80);shot('03-see-more')
    q.get_by_role('button',name='Jump to latest',exact=True).click();q.wait_for_timeout(100)
    q.get_by_role('button',name='Open evidence: Review sample document collection',exact=True).click();shot('04-activity-dialog');q.keyboard.press('Escape')
    scenario('choices');shot('05-choices')
    scenario('mixed-stream');q.locator('[data-message-id="mixed-scheduled"]').scroll_into_view_if_needed();shot('06-scheduled')
    scenario('bundle-files');q.wait_for_timeout(14500);q.locator('.kind-attachment').last.scroll_into_view_if_needed();q.locator('.kind-attachment').last.hover();q.wait_for_timeout(300);q.screenshot(path=str(out/'07-attachment-actions.png'))
    scenario('browser-watch');q.wait_for_timeout(4300);shot('08-browser-card');q.get_by_role('button',name='Open browser',exact=True).click();shot('09-browser-workspace');q.get_by_role('button',name='Close browser session',exact=True).click()
    reset();q.locator('#rail-more').click();q.get_by_role('button',name='Report an issue',exact=True).click();shot('10-issue-report');q.keyboard.press('Escape')
    q.get_by_role('tab',name='Schedule',exact=True).click();q.locator('.context-job').first.click();shot('11-schedule-detail');q.keyboard.press('Escape')
    reset();q.emulate_media(color_scheme='light');shot('12-light');q.emulate_media(color_scheme='dark')
    for w in [320,390,768,1280]:
        q.set_viewport_size({'width':w,'height':844});reset();shot(f'13-chat-{w}')
        scenario('choices');shot(f'14-choices-{w}')
    (out/'capture.json').write_text(json.dumps({'errors':errors},indent=2)+'\n')
    b.close()
    assert not errors, errors
