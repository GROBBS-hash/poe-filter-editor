-- The editor's Path of Building worker (28 Nov 2026, GROBBS: "real Path of Building numbers").
-- Runs inside the user's own Path of Building (its lua51.dll, working folder = the PoB install), started by the
-- editor with `--pob-worker`. One JSON request per line on stdin; every answer is one line starting "@@POB@@" on
-- stdout (PoB prints its own messages too, the editor skips those).
--   {"cmd":"load","xml":"<PathOfBuilding>...","stage":"Swap at Merciless Lab (level 68-90)"}
--   {"cmd":"compare","slot":"Helmet","items":["Rarity: RARE\n..."]}
arg = {}
package.path = "lua/?.lua;lua/?/init.lua;" .. package.path
package.cpath = "./?.dll;" .. package.cpath
SG_DEF = WORKER_DIR .. "/_SimpleGraphic.def.lua"
POST_DEF = WORKER_DIR .. "/inflate.lua"

local json
local function reply(t)
	io.write("@@POB@@", json.encode(t), "\n")
	io.flush()
end

dofile(WORKER_DIR .. "/HeadlessWrapper.lua")
json = require("dkjson")
if STARTUP_ERROR then
	reply({ error = "Path of Building didn't start: " .. tostring(STARTUP_ERROR) })
	return
end

-- The numbers the editor shows.
local KEYS = { "Life", "EnergyShield", "Mana", "Armour", "Evasion", "FireResist", "ColdResist", "LightningResist",
	"ChaosResist", "CombinedDPS", "TotalEHP", "Str", "Dex", "Int", "Spirit", "Ward" }
local function summary(o)
	local t = {}
	for _, k in ipairs(KEYS) do
		local v = o[k]
		if type(v) == "number" and v == v and v ~= math.huge and v ~= -math.huge then t[k] = v end
	end
	return t
end

-- The item set and tree named like the stage (the editor's stages are PoB's item sets).
local function pickStage(title)
	if not title or title == "" then return end
	local it = build.itemsTab
	for _, id in ipairs(it.itemSetOrderList) do
		local s = it.itemSets[id]
		if s and s.title == title then it:SetActiveItemSet(id) break end
	end
	for i, spec in ipairs(build.treeTab.specList) do
		if spec.title == title then build.treeTab:SetActiveSpec(i) break end
	end
	build.buildFlag = true
	runCallback("OnFrame")
end

reply({ ready = true })
for line in io.lines() do
	local ok, err = pcall(function()
		local req = json.decode(line)
		if not req then error("not JSON") end
		if req.cmd == "load" then
			loadBuildFromXML(req.xml, "build")
			pickStage(req.stage)
			local calc = build.calcsTab:GetMiscCalculator()
			reply({ ok = true, base = summary(calc({})) })
		elseif req.cmd == "compare" then
			if not build or not build.calcsTab then error("no build loaded") end
			local calc = build.calcsTab:GetMiscCalculator()
			local out = { base = summary(calc({})), items = {} }
			for i, text in ipairs(req.items or {}) do
				local good, res = pcall(function()
					local item = new("Item"):Item(text)
					if not item.base then return { error = "PoB doesn't know this item" } end
					return summary(calc({ repSlotName = req.slot, repItem = item }))
				end)
				out.items[i] = good and res or { error = tostring(res) }
			end
			reply(out)
		elseif req.cmd == "ping" then
			reply({ ok = true })
		else
			error("unknown command " .. tostring(req.cmd))
		end
	end)
	if not ok then reply({ error = tostring(err) }) end
end
