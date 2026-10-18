import sys, os, asyncio
from playwright.async_api import async_playwright
html, outdir, mode = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(outdir, exist_ok=True)
async def main():
    async with async_playwright() as p:
        b = await p.chromium.launch()
        pg = await b.new_page(viewport={"width":1200,"height":800}, device_scale_factor=float(os.environ.get("DSF","1")))
        await pg.goto("file://"+html)
        await pg.wait_for_timeout(500)
        if mode == "keys":
            ts = [float(x) for x in sys.argv[4].split(",")]
        else:
            fps = int(mode); total = await pg.evaluate("TOTAL")
            ts = [i/fps for i in range(int(total*fps))]
        for i, t in enumerate(ts):
            await pg.evaluate(f"render({t})")
            await pg.screenshot(path=f"{outdir}/{i:05d}.png")
        await b.close()
asyncio.run(main())
