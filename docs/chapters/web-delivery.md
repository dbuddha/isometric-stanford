# Web delivery

The browser consumes a static Deep Zoom Image descriptor and WebP tile pyramid.
OpenSeadragon owns pan, zoom, pinch, tile selection, and cache integration. The
application owns release metadata, accessibility, responsive controls, cache
budgets, missing-tile behavior, context recovery, attribution, and observability.

The viewer disables image smoothing above native scale and avoids a continuous
render loop. It adapts decoded cache capacity to the device memory budget,
prefers current-view requests, cancels stale work, and displays a recoverable
error instead of a blank canvas when tiles fail.

A custom Rust or WebAssembly viewer adds tile scheduling, gesture handling,
accessibility, browser compatibility, decode behavior, and years of edge cases.
It is not simpler unless profiling identifies an OpenSeadragon bottleneck that
cannot be fixed or bounded. Version 1 therefore keeps OpenSeadragon.
