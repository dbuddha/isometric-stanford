# Web delivery

The browser consumes a static Deep Zoom Image descriptor and WebP tile pyramid.
OpenSeadragon owns pan, zoom, pinch, tile selection, and cache integration. The
application owns release metadata, accessibility, responsive controls, cache
budgets, missing-tile behavior, context recovery, attribution, and observability.

The cache policy treats phone-width, coarse-pointer, and low-memory devices as
constrained even when the nonstandard `deviceMemory` browser property is
unavailable. It permits 48 decoded 512-pixel RGBA tiles on constrained devices
and 128 on capable desktop screens. Those images consume at most 48 MiB and 128
MiB respectively, leaving half of the accepted 96 MiB mobile and 256 MiB
desktop budgets for OpenSeadragon, canvas or WebGL copies, and browser
bookkeeping. A phone-width screen begins at 2.25 times the full-map home zoom so
the wide hero scene is legible, while the Home control still returns to the
complete artwork.

The viewer disables image smoothing above native scale and avoids a continuous
render loop. It adapts decoded cache capacity to the device memory budget,
prefers current-view requests, cancels stale work, and displays a recoverable
error instead of a blank canvas when tiles fail.

The production configuration fixes OpenSeadragon to its Canvas drawer, avoiding
a WebGL dependency for this static indexed artwork. It retains keyboard pan and
zoom, touch pan and pinch, and explicit Home behavior. Tiles receive two
bounded retries. Exhausted tile retries and descriptor failures surface a
visible Retry control. After the first successful open, retry preserves the
user's viewport. A restored drawing context forces an immediate redraw. No
navigation-image CDN is requested because the application owns its controls.

A custom Rust or WebAssembly viewer adds tile scheduling, gesture handling,
accessibility, browser compatibility, decode behavior, and years of edge cases.
It is not simpler unless profiling identifies an OpenSeadragon bottleneck that
cannot be fixed or bounded. Version 1 therefore keeps OpenSeadragon.

The real 7,623 by 3,325 candidate pyramid has been exercised locally through
the production viewer. A 390 by 844 mobile viewport initially requested three
WebP tiles and transferred about 107 KiB; a 1,280 by 720 desktop viewport
requested eleven tiles and transferred about 293 KiB. Both remained visually
continuous through zooming, and automated accessibility inspection reported no
violations. These results prove DZI integration and leave ample room under the
2.5 MiB initial imagery budget. Fixed iPhone 12-class and Pixel 7-class frame,
memory, recovery, and network measurements remain qualification work.

Ordinary pull requests run the recovery suite against an in-process one-pixel
lossless WebP DZI. Scheduled assurance generates the complete locked hero DZI
and runs the same desktop and Pixel 7 browser suite against real tiles. The
release dry run assembles an inspectable static viewer bundle without deploying
it. Public Pages publication remains an owner decision after style approval.
