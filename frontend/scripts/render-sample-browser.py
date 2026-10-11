"""Regenerate the synthetic browser frame. No external page or reference image is read."""
from pathlib import Path
import shutil
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[1]
HTML='''<!doctype html><html lang="en"><meta charset="utf-8"><title>Sample browser challenge</title><style>
*{box-sizing:border-box}body{margin:0;font:14px Arial,sans-serif;color:#202124;background:white}.tabs{background:#e8eef7;height:38px;padding:10px 20px;width:100%}.address{background:#f5f7fb;height:38px;padding:9px 20px;color:#536170}.main{padding:48px 34px;width:560px}.challenge{display:flex;align-items:center;gap:14px;border:1px solid #d4d7d9;background:#fafafa;border-radius:3px;width:330px;padding:22px 18px;margin-bottom:30px}.box{width:26px;height:26px;border:2px solid #777;background:white}.label{line-height:1.5}.tag{font-size:11px;color:#67716c;margin-left:auto}h2{font-size:16px;margin:0 0 14px}p{line-height:1.6}.note{margin-top:28px;color:#69716c;font-size:12px}</style><div class="tabs">Sample search &nbsp; ×</div><div class="address">← &nbsp; ⟳ &nbsp; search.example / challenge</div><div class="main"><div class="challenge"><div class="box"></div><span class="label">Continue to the sample page</span><span class="tag">LOCAL<br>DEMO</span></div><h2>A sample confirmation step</h2><p>This fictional browser page demonstrates a paused task. Nothing continues until you grant permission or take control.</p><p>Use the controls above the preview to continue the local walkthrough.</p><div class="note">Synthetic fixture · no network request, account, location, or address data</div></div></html>'''
with sync_playwright() as p:
 browser=p.chromium.launch(executable_path=shutil.which('chromium'),headless=True,args=['--no-sandbox'])
 page=browser.new_page(viewport={'width':1200,'height':760},device_scale_factor=1)
 page.set_content(HTML);page.screenshot(path=str(ROOT/'public/media/browser-sample-challenge.png'))
 browser.close()
