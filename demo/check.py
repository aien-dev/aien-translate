"""Load the demo page with the real translate.js and report what it shows."""
import json, sys
from playwright.sync_api import sync_playwright

url = sys.argv[1]
with sync_playwright() as p:
    b = p.chromium.launch()
    pg = b.new_page()
    reqs = []
    pg.on("request", lambda r: reqs.append(r.url) if r.url.endswith(".json") or ".json?" in r.url else None)
    pg.on("console", lambda m: print("console:", m.text[:200]) if m.type in ("error", "warning") else None)
    pg.goto(url)
    pg.wait_for_function("document.querySelector('h1').innerText.indexOf('操作系统') >= 0", timeout=120000)
    pg.wait_for_timeout(2000)
    out = pg.evaluate("""() => [...document.querySelectorAll('h1,p,button')].map(e => e.innerText)""")
    print(json.dumps({"page": out, "requests": reqs}, ensure_ascii=False, indent=1))
    b.close()
