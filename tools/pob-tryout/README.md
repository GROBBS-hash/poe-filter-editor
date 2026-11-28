# Path of Building without its window (try-out, 28 Nov 2026)

GROBBS asked for "real Path of Building numbers". This proves it works on his PC; nothing in the app uses it yet.

**What runs:** `pobhost` (src/main.rs) loads PoB's own `lua51.dll` (LuaJIT, so PoB's C modules like lua-utf8 load
too), sets the working folder to the PoB install and runs a Lua script. `swap.lua` runs PoB's `HeadlessWrapper.lua`,
loads a build's XML, picks a stage (item set + tree by name), prints the stage's numbers, then asks PoB what one item
in one slot would change (`build.calcsTab:GetMiscCalculator()` with `{ repSlotName = "Helmet", repItem = item }`, the
same call as PoB's item tooltips). `inflate.lua` gives the headless version real `Inflate` / `Deflate` through PoB's
`zlib1.dll` (without it timeless jewel data doesn't load: "Missing LUT: Elegant Hubris").

**Not in this folder (fetch them):** PoB's own `src/HeadlessWrapper.lua` and `src/_SimpleGraphic.def.lua` from
github.com/PathOfBuildingCommunity/PathOfBuilding (MIT). Patch the wrapper: `dofile("_SimpleGraphic.def.lua")` ->
`dofile(SG_DEF)` then `if POST_DEF then dofile(POST_DEF) end`. Scripts also need `arg = {}` (Main.lua reads it).

**Run:** build with cargo, put the build XML (decoded PoB code) as `lum.xml` next to the scripts, then
`POB_TRY=<this folder> pobhost.exe "D:\POB\Path of Building Community" <this folder>\swap.lua`.
`POB_STAGE` picks the stage by name (default "Swap at Merciless").

**Results on his Luminary build (PoB 2.68.1 at D:\POB):** start 2.3 s, build 0.9 s, each item swap 0.002 s.
"Swap at Merciless Lab": Life 4223, ES 1233; a Deicide Mask (+100 life, +20% chaos, +40% fire) instead of the
guide's Torturer's Mask: Life 4223, ES 1166, fire res +40. Fast enough to rank every listing of a trade search.
