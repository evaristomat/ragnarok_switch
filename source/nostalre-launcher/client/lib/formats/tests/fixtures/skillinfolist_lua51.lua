-- Source of skillinfolist_lua51.lub, shaped like kRO's skillinfolist.lub: 32-bit luac 5.1.5, after
-- skillid_lua51.lub. Our own values.
SKILL_INFO_LIST = {
	[SKID.SM_BASH] = {
		"SM_BASH",
		SkillName = "Bash",
		MaxLv = 5,
		SpAmount = { 7, 7, 7, 12, 12 },
		bSeperateLv = true,
		AttackRange = { 1, 1, 1, 1, 1 },
	},
	[SKID.AL_HEAL] = {
		"AL_HEAL",
		SkillName = "Heal",
		MaxLv = 3,
		SpAmount = { 10, 15, 20 },
		bSeperateLv = true,
		NeedSkillList = { [JOBID.JT_CRUSADER] = { { SKID.SM_BASH, 1 } } },
	},
	-- a level picker, no SP per level
	[SKID.AL_TELEPORT] = { "AL_TELEPORT", SkillName = "Teleport", MaxLv = 2, bSeperateLv = true },
	-- every level cast at once: left out
	[SKID.SM_SWORD] = { "SM_SWORD", SkillName = "Sword Mastery", MaxLv = 3, SpAmount = { 0, 0, 0 }, bSeperateLv = false },
}
