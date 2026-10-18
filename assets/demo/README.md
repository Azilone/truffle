# The demo

`index.html` is the whole animation: `render(t)` draws the frame at `t` seconds.

```bash
uv run --with playwright python -m playwright install chromium
DSF=1.6 uv run --with playwright python render.py "$PWD/index.html" frames 30
ffmpeg -framerate 30 -i frames/%05d.png -c:v libx264 -pix_fmt yuv420p -crf 18 ../demo.mp4
ffmpeg -framerate 30 -i frames/%05d.png -vf "fps=15,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=192:stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a:diff_mode=rectangle" ../demo.gif
```

Photos, from Wikimedia Commons:

- `dog1.jpg`: [Jack Russell Terrier Eddi running at the beach](https://commons.wikimedia.org/wiki/File:Jack_Russell_Terrier_Eddi_running_at_the_beach.JPG), Steve-65, CC BY-SA 3.0
- `dog2.jpg`: [Running dog at the beach](https://commons.wikimedia.org/wiki/File:Running_dog_at_the_beach.jpg), JurgenNL, CC BY-SA 3.0
- `dog3.jpg`: [Dog at Ocean Beach](https://commons.wikimedia.org/wiki/File:Dog_at_Ocean_Beach.jpg), Vaishakh1234, CC BY-SA 4.0
