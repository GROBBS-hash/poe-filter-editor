# Path of Building files

`HeadlessWrapper.lua` and `_SimpleGraphic.def.lua` come from Path of Building Community
(github.com/PathOfBuildingCommunity/PathOfBuilding, `src/`, fetched 28 Nov 2026). The wrapper has two small changes,
marked "(PoB Editor)": it loads the stubs from `SG_DEF` / `POST_DEF`, and reports a start-up error instead of
waiting for Enter. `inflate.lua` and `worker.lua` are ours. The editor runs these inside your own installed copy of
Path of Building (its lua51.dll), see `src-tauri/src/pob_worker.rs`.

Path of Building's licence:

```
Path of Building Community:

*******************************************************************************

Copyright (c) 2016 David Gowor

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

```
