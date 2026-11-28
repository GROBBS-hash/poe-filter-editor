-- Try-out: Path of Building without its window. Loads the build, picks a stage, then asks PoB how a different
-- helmet changes the numbers (the same calculation as its item tooltips).
local here = os.getenv("POB_TRY")
arg = {}
package.path = "lua/?.lua;lua/?/init.lua;" .. package.path
package.cpath = "./?.dll;" .. package.cpath
SG_DEF = here .. "/_SimpleGraphic.def.lua"
POST_DEF = here .. "/inflate.lua"
local t0 = os.clock()
dofile(here .. "/Headless.lua")
local f = io.open(here .. "/lum.xml", "rb"); local xml = f:read("*a"); f:close()
loadBuildFromXML(xml, "Luminary")
print(string.format("ready in %.1f s", os.clock() - t0))

-- The stage: the item set and tree whose names contain WANT.
local want = os.getenv("POB_STAGE") or "Swap at Merciless"
local it = build.itemsTab
local picked = false
for _, id in ipairs(it.itemSetOrderList) do
	local s = it.itemSets[id]
	print("item set", id, s.title)
	if s.title and s.title:find(want, 1, true) and not picked then it:SetActiveItemSet(id); picked = true end
end
for i, spec in ipairs(build.treeTab.specList) do
	print("tree", i, spec.title)
	if spec.title and spec.title:find(want, 1, true) then build.treeTab:SetActiveSpec(i) end
end
build.buildFlag = true
runCallback("OnFrame")

local function show(label, o)
	print(string.format("%-22s Life %5d  ES %5d  Mana %5d  Fire %3d Cold %3d Light %3d Chaos %3d  DPS %.1f",
		label, o.Life or 0, o.EnergyShield or 0, o.Mana or 0, o.FireResist or 0, o.ColdResist or 0, o.LightningResist or 0, o.ChaosResist or 0, o.CombinedDPS or 0))
end
local calc0 = build.calcsTab:GetMiscCalculator()
show("stage as the guide has it", calc0({}))
for name, slot in pairs(it.slots) do if slot.selItemId and slot.selItemId ~= 0 and it.items[slot.selItemId] then print("  worn", name, it.items[slot.selItemId].name) end end
print("active set", it.activeItemSetId, "active tree", build.treeTab.activeSpec)
local helm = it.slots["Helmet"]
local sel = it.activeItemSet and it.activeItemSet["Helmet"] and it.activeItemSet["Helmet"].selItemId
print("helmet slot id", helm and helm.selItemId, "set id", sel)
local cur = it.items[sel or (helm and helm.selItemId) or 0]
print("helmet now:", cur and (cur.name .. " / " .. cur.baseName) or "none")

t0 = os.clock()
local calc = build.calcsTab:GetMiscCalculator()
local test = new("Item"):Item([[Rarity: RARE
Test Visage
Deicide Mask
Quality: 20
Item Level: 84
+100 to maximum Life
+20% to Chaos Resistance
16% increased Global Accuracy Rating
+40% to Fire Resistance]])
test:NormaliseQuality()
show("with a Deicide Mask", calc({ repSlotName = "Helmet", repItem = test }))
print(string.format("one swap worked out in %.3f s", os.clock() - t0))
