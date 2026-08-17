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

The real 7,623 by 3,325 Candidate C pyramid has been exercised locally through
the production viewer. A Pixel 7 browser profile requested 23 WebP tiles and
transferred 1,258,408 bytes before the artwork-ready state; a 1,280 by 720
desktop profile requested eleven tiles and transferred 493,712 bytes. Both are
below the 2.5 MiB initial imagery budget. The respective cache limits remain 48
and 128 decoded tiles, reserving half of each total browser memory budget for
OpenSeadragon, Canvas, and browser overhead. A one-second hosted interaction
probe records frame cadence, longest frame gap, JavaScript heap, requests,
bytes, and cache policy as regression evidence. It is deliberately not treated
as fixed-device FPS qualification.

The assembled local preview was also inspected in Chromium through desktop and
390 by 844 mobile viewports. Desktop first contentful paint and largest
contentful paint were 264 milliseconds with 9.5 MiB of used JavaScript heap.
The mobile run reported 152 and 168 milliseconds respectively with 11.1 MiB of
used heap. These localhost measurements demonstrate responsive integration,
not field performance. The initial audit found a 0.13 mobile cumulative layout
shift because release evidence was inserted asynchronously. The viewer now
reserves that row before metadata arrives, and browser regression coverage
protects the behavior. The repeated mobile run reported 0.00 cumulative layout
shift.

The mobile page bounds the viewer to 62 dVH with a 26 to 34 rem range and does
not stretch grid rows into unused space. This keeps the wide artwork prominent
on portrait screens while preserving touch-sized controls and a complete
no-scroll Pixel 7 layout. Automated accessibility inspection reports no WCAG A
or AA violations. Fixed iPhone 12-class and physical Pixel 7 frame, decode
memory, recovery, and network measurements remain qualification work.

Ordinary pull requests run the recovery suite against an in-process one-pixel
lossless WebP DZI. Scheduled assurance generates the complete Candidate C hero DZI
and runs the same desktop and Pixel 7 browser suite against real tiles. The
release dry run assembles an inspectable static viewer bundle without deploying
it. `scripts/assemble_preview.py` refuses a qualified claim, stale world hash,
corrupt tile, incomplete pyramid, or viewer build containing pre-staged art.
It writes an independent `preview.json` that states
`unqualified-engineering-preview` and `published_release: false`. Public Pages
publication remains an owner decision after style approval.
