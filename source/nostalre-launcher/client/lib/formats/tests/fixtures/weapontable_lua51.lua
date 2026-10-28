-- Source of weapontable_lua51.lub, shaped like kRO's weapontable: luac5.1, then size_t rewritten to 4 bytes.
-- Keys are Weapon_IDs lookups; the Lance is named after its classic item, the other view only has a base.
Weapon_IDs = { WEAPONTYPE_SPEAR = 4, WEAPONTYPE_Lance = 52, WEAPONTYPE_Odd = 99 }
WeaponNameTable = {
	[Weapon_IDs.WEAPONTYPE_SPEAR] = "_spear",
	[Weapon_IDs.WEAPONTYPE_Lance] = "_1410",
	[Weapon_IDs.WEAPONTYPE_Odd] = "_odd"
}
Expansion_Weapon_IDs = {
	[Weapon_IDs.WEAPONTYPE_Lance] = Weapon_IDs.WEAPONTYPE_SPEAR,
	[Weapon_IDs.WEAPONTYPE_Odd] = Weapon_IDs.WEAPONTYPE_SPEAR
}
