# User experience

The public experience is a static artwork viewer, not an interactive 3D scene.
Users can pan, pinch, zoom, use keyboard controls, open a compact help panel,
and read attribution. The viewer must remain responsive on desktop integrated
GPUs and iPhone 12-class or Pixel 7-class mobile devices.

Initial imagery is capped at 2.5 MB. Decoded tile cache targets are 96 MB on
mobile and 256 MB on desktop. Release qualification requires smooth ordinary
pan and pinch, no blank frames, correct missing-tile and context-loss recovery,
at least 55 FPS on target mobile devices, p75 INP no greater than 200 ms, and
visual LCP no greater than 2.5 seconds on representative 4G.

The review dashboard is a separate development surface. It overlays semantic
confidence, source disagreements, dirty bounds, render versions, and accepted
comparison evidence. It never changes final pixels in the browser.
