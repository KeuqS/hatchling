"""Reproducible native browser screenshots/recording. No source references are bundled."""
from pathlib import Path
import argparse,json,os,shutil,subprocess
from playwright.sync_api import sync_playwright,expect
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'artifacts/parity-v10'
OUT.mkdir(parents=True,exist_ok=True)
parser=argparse.ArgumentParser();parser.add_argument('--motion',action='store_true');args=parser.parse_args()
with sync_playwright() as p:
    browser=p.chromium.launch(executable_path=os.environ.get('CHROMIUM_PATH') or shutil.which('chromium'),args=['--no-sandbox'] if os.geteuid()==0 else [])
    options={'viewport':{'width':1756,'height':934},'color_scheme':'dark','reduced_motion':'no-preference' if args.motion else 'reduce'}
    if args.motion:options.update(record_video_dir=str(OUT/'recording'),record_video_size={'width':1756,'height':934})
    context=browser.new_context(**options);q=context.new_page();errors=[];q.on('pageerror',lambda e:errors.append(str(e)))
    html=(ROOT/'dist/hatchling-standalone.html').read_text()
    def reset():
        q.set_content(html);q.locator('#message-input').wait_for();q.wait_for_timeout(80)
    def shot(name):
        q.mouse.move(1000,920);q.wait_for_timeout(100);q.screenshot(path=str(OUT/(name+'.png')))
    def route(name):q.evaluate('(hash)=>location.hash=hash',name);q.wait_for_timeout(100)
    def scenario(name):q.keyboard.press('Control+Shift+D');q.locator(f'[data-scenario="{name}"]').click()
    reset()
    if args.motion:
        q.wait_for_timeout(900)
        q.locator('.context-close button').click();q.wait_for_timeout(950)
        q.get_by_role('button',name='Show agent status',exact=True).click();q.wait_for_timeout(1500)
        q.get_by_role('tab',name='Approvals',exact=True).click();q.wait_for_timeout(450)
        q.get_by_role('tab',name='Schedule',exact=True).click();q.wait_for_timeout(450)
        q.get_by_role('tab',name='Identity',exact=True).click();q.wait_for_timeout(650)
        q.locator('#message-input').fill('Change my name to Pip');q.wait_for_timeout(500)
        q.get_by_role('button',name='Send message',exact=True).click();q.wait_for_timeout(1000)
        scenario('fix-ci');q.wait_for_timeout(900)
        q.locator('.context-close button').click();q.wait_for_timeout(500)
        q.get_by_role('button',name='Show agent status',exact=True).click();q.wait_for_timeout(5300)
        scenario('bundle-files');q.get_by_role('button',name='Open evidence: Bundle sample files',exact=True).click();q.wait_for_timeout(2500)
        q.locator('.activity-timeline').evaluate('el=>el.scrollTop=el.scrollHeight');q.wait_for_timeout(1800)
        q.keyboard.press('Escape');q.wait_for_timeout(600)
        scenario('browser-watch');q.get_by_role('button',name='Open browser',exact=True).click();q.wait_for_timeout(1000)
        q.get_by_role('button',name='Take control of the browser',exact=True).click();q.wait_for_timeout(1200)
        q.get_by_role('button',name='Close browser session',exact=True).click();q.wait_for_timeout(800)
        video=q.video;context.close();raw=Path(video.path());shutil.copyfile(raw,OUT/'profile-and-workspace.webm');raw.unlink()
        subprocess.run(['ffmpeg','-y','-loglevel','error','-i',str(OUT/'profile-and-workspace.webm'),'-c:v','libx264','-preset','fast','-crf','23','-pix_fmt','yuv420p','-movflags','+faststart',str(OUT/'profile-and-workspace.mp4')],check=True)
    else:
        shot('01-workspace')
        avatar=q.locator('.agent-portrait > img').bounding_box()
        q.screenshot(path=str(OUT/'02-profile.png'),clip={'x':avatar['x']-102,'y':avatar['y']-27,'width':313,'height':221})
        measurements=q.locator('.agent-profile').evaluate('''el=>({
          viewport:[innerWidth,innerHeight],zoom:visualViewport.scale,
          name:getComputedStyle(el.querySelector('.agent-name')).fontSize,
          status:getComputedStyle(el.querySelector('.agent-status')).fontSize,
          avatar:el.querySelector('img').getBoundingClientRect().toJSON(),
          composer:document.querySelector('.composer').getBoundingClientRect().toJSON(),
          input:getComputedStyle(document.querySelector('#message-input')).fontSize,
          family:getComputedStyle(el).fontFamily
        })''')
        (OUT/'measurements.json').write_text(json.dumps(measurements,indent=2)+'\n')
        q.locator('.identity-card').get_by_role('button',name='Edit',exact=True).click();shot('03-identity');q.keyboard.press('Escape')
        for tab in ['Activity','Approvals','Schedule']:
            q.get_by_role('tab',name=tab,exact=True).click();shot('04-panel-'+tab.lower())
        q.get_by_role('tab',name='Activity',exact=True).click()
        q.get_by_role('button',name='Open evidence: Review sample document collection',exact=True).click();shot('05-activity')
        q.locator('#timeline-dynamic-inspection').click();shot('06-step-evidence');q.keyboard.press('Escape')
        for page in ['goals','ideas','feed','library/system','library','memory','schedule','notifications']:
            route(page);shot('07-'+page.replace('/','-'))
        q.keyboard.press('Control+,')
        for section in ['General','Connectors','Wallet','Secure store','Permissions','Messaging channels','Devices','Data controls','Help & support','Legal info']:
            q.get_by_role('navigation',name='Settings sections').get_by_role('button',name=section,exact=True).click();shot('08-settings-'+section.lower().replace(' ','-').replace('&','and'))
        q.keyboard.press('Escape');route('chat/reference')
        q.locator('#rail-more').click();shot('09-more-menu');q.keyboard.press('Escape')
        q.get_by_role('button',name='Search chats',exact=True).click();shot('10-search');q.keyboard.press('Escape')
        q.locator('.context-close button').click();shot('11-avatar-collapsed');q.get_by_role('button',name='Show agent status',exact=True).click()
        scenario('browser-watch');q.get_by_role('button',name='Open browser',exact=True).click();q.wait_for_timeout(4200);shot('12-browser')
        q.get_by_role('button',name='Take control of the browser',exact=True).click();shot('13-browser-takeover');q.get_by_role('button',name='Close browser session',exact=True).click()
        reset();q.emulate_media(color_scheme='light');shot('14-light-workspace')
        q.emulate_media(color_scheme='dark')
        for width in [320,390]:
            q.set_viewport_size({'width':width,'height':844});reset();shot('15-chat-'+str(width))
            q.get_by_role('button',name='Show agent status',exact=True).click();shot('16-profile-'+str(width));q.locator('.context-close button').click()
            q.keyboard.press('Control+,');shot('17-settings-'+str(width));q.keyboard.press('Escape')
        context.close()
    browser.close()
    (OUT/('motion-results.json' if args.motion else 'capture-results.json')).write_text(json.dumps({'errors':errors,'source':'Running compiled frontend; synthetic local fixtures only. No connected backend.'},indent=2)+'\n')
    assert not errors,errors
print(OUT)
