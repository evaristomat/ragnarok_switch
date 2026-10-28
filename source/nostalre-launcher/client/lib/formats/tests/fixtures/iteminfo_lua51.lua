-- Source of iteminfo_lua51.lub, shaped like kRO's iteminfo: luac5.1, then size_t rewritten to 4 bytes (kRO's
-- layout). A list of more than 50 items takes two SETLIST blocks.
tbl = {
	[1201] = {
		unidentifiedDisplayName = "Dagger",
		identifiedDescriptionName = { "A short blade.", "Weight: 40" },
		slotCount = 3,
		ClassNum = 1,
		costume = false
	},
	[500001] = {
		identifiedDisplayName = "Boost Sword",
		identifiedResourceName = "boost_sword",
		identifiedDescriptionName = { "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31", "32", "33", "34", "35", "36", "37", "38", "39", "40", "41", "42", "43", "44", "45", "46", "47", "48", "49", "50", "51", "52" },
		ClassNum = 2
	}
}
main = function()
	for ItemID, DESC in pairs(tbl) do
		print(ItemID)
	end
	return true, "good"
end
