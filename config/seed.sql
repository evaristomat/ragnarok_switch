-- SPDX-License-Identifier: GPL-3.0-or-later
-- Schema derived from the bundled rAthena server. No player data.
-- Login rows are synthetic defaults for this offline edition.
PRAGMA journal_mode=DELETE;
BEGIN TRANSACTION;
CREATE TABLE `acc_reg_num` (
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `key` TEXT NOT NULL default '' CHECK (length(`key`) <= 32),
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN 0 AND 4294967295),
  `value` INTEGER NOT NULL default '0' CHECK (`value` BETWEEN -9223372036854775808 AND 9223372036854775807),
  PRIMARY KEY (`account_id`, `key`, `index`)
);
CREATE TABLE `acc_reg_str` (
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `key` TEXT NOT NULL default '' CHECK (length(`key`) <= 32),
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN 0 AND 4294967295),
  `value` TEXT COLLATE NOCASE NOT NULL default '0' CHECK (length(`value`) <= 254),
  PRIMARY KEY (`account_id`, `key`, `index`)
);
CREATE TABLE `achievement` (
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `id` INTEGER NOT NULL DEFAULT 0 CHECK (`id` >= 0),
  `count1` INTEGER NOT NULL default '0' CHECK (`count1` BETWEEN 0 AND 4294967295),
  `count2` INTEGER NOT NULL default '0' CHECK (`count2` BETWEEN 0 AND 4294967295),
  `count3` INTEGER NOT NULL default '0' CHECK (`count3` BETWEEN 0 AND 4294967295),
  `count4` INTEGER NOT NULL default '0' CHECK (`count4` BETWEEN 0 AND 4294967295),
  `count5` INTEGER NOT NULL default '0' CHECK (`count5` BETWEEN 0 AND 4294967295),
  `count6` INTEGER NOT NULL default '0' CHECK (`count6` BETWEEN 0 AND 4294967295),
  `count7` INTEGER NOT NULL default '0' CHECK (`count7` BETWEEN 0 AND 4294967295),
  `count8` INTEGER NOT NULL default '0' CHECK (`count8` BETWEEN 0 AND 4294967295),
  `count9` INTEGER NOT NULL default '0' CHECK (`count9` BETWEEN 0 AND 4294967295),
  `count10` INTEGER NOT NULL default '0' CHECK (`count10` BETWEEN 0 AND 4294967295),
  `completed` TEXT,
  `rewarded` TEXT,
  PRIMARY KEY (`char_id`, `id`)
);
CREATE TABLE `atcommandlog` (
  `atcommand_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`atcommand_id` BETWEEN 0 AND 16777215),
  `atcommand_date` TEXT NOT NULL,
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `char_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`char_name`) <= 25),
  `map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`map`) <= 11),
  `command` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`command`) <= 255)
);
CREATE TABLE `auction` (
  `auction_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`auction_id` >= 0),
  `seller_id` INTEGER NOT NULL default '0' CHECK (`seller_id` BETWEEN 0 AND 4294967295),
  `seller_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`seller_name`) <= 30),
  `buyer_id` INTEGER NOT NULL default '0' CHECK (`buyer_id` BETWEEN 0 AND 4294967295),
  `buyer_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`buyer_name`) <= 30),
  `price` INTEGER NOT NULL default '0' CHECK (`price` BETWEEN 0 AND 4294967295),
  `buynow` INTEGER NOT NULL default '0' CHECK (`buynow` BETWEEN 0 AND 4294967295),
  `hours` INTEGER NOT NULL default '0' CHECK (`hours` BETWEEN -32768 AND 32767),
  `timestamp` INTEGER NOT NULL default '0' CHECK (`timestamp` BETWEEN 0 AND 4294967295),
  `nameid` INTEGER NOT NULL default '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `item_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`item_name`) <= 50),
  `type` INTEGER NOT NULL default '0' CHECK (`type` BETWEEN -32768 AND 32767),
  `refine` INTEGER NOT NULL default '0' CHECK (`refine` BETWEEN 0 AND 255),
  `attribute` INTEGER NOT NULL default '0' CHECK (`attribute` BETWEEN 0 AND 255),
  `card0` INTEGER NOT NULL default '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL default '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL default '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL default '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `unique_id` INTEGER NOT NULL default '0' CHECK (`unique_id` >= 0),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255)
);
CREATE TABLE `barter` (
  `name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`name`) <= 50),
  `index` INTEGER NOT NULL DEFAULT 0 CHECK (`index` BETWEEN 0 AND 65535),
  `amount` INTEGER NOT NULL DEFAULT 0 CHECK (`amount` BETWEEN 0 AND 65535),
  PRIMARY KEY (`name`, `index`)
);
CREATE TABLE `bonus_script` (
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `script` TEXT COLLATE NOCASE NOT NULL DEFAULT '',
  `tick` INTEGER NOT NULL DEFAULT '0' CHECK (`tick` BETWEEN -9223372036854775808 AND 9223372036854775807),
  `flag` INTEGER NOT NULL DEFAULT '0' CHECK (`flag` BETWEEN 0 AND 65535),
  `type` INTEGER NOT NULL DEFAULT '0' CHECK (`type` BETWEEN 0 AND 255),
  `icon` INTEGER NOT NULL DEFAULT '-1' CHECK (`icon` BETWEEN -32768 AND 32767)
);
CREATE TABLE `branchlog` (
  `branch_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`branch_id` BETWEEN 0 AND 16777215),
  `branch_date` TEXT NOT NULL,
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN -2147483648 AND 2147483647),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN -2147483648 AND 2147483647),
  `char_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`char_name`) <= 25),
  `map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`map`) <= 11)
);
CREATE TABLE `buyingstore_items` (
  `buyingstore_id` INTEGER NOT NULL DEFAULT 0 CHECK (`buyingstore_id` BETWEEN 0 AND 4294967295),
  `index` INTEGER NOT NULL DEFAULT 0 CHECK (`index` BETWEEN 0 AND 65535),
  `item_id` INTEGER NOT NULL DEFAULT 0 CHECK (`item_id` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL DEFAULT 0 CHECK (`amount` BETWEEN 0 AND 65535),
  `price` INTEGER NOT NULL DEFAULT 0 CHECK (`price` BETWEEN 0 AND 4294967295),
  PRIMARY KEY (`buyingstore_id`, `index`)
);
CREATE TABLE `buyingstores` (
  `id` INTEGER NOT NULL DEFAULT 0 CHECK (`id` BETWEEN 0 AND 4294967295),
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `sex` TEXT COLLATE NOCASE NOT NULL DEFAULT 'M' CHECK (`sex` IN ('F','M')),
  `map` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`map`) <= 20),
  `x` INTEGER NOT NULL DEFAULT 0 CHECK (`x` BETWEEN 0 AND 65535),
  `y` INTEGER NOT NULL DEFAULT 0 CHECK (`y` BETWEEN 0 AND 65535),
  `title` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`title`) <= 80),
  `limit` INTEGER NOT NULL DEFAULT 0 CHECK (`limit` BETWEEN 0 AND 4294967295),
  `body_direction` TEXT COLLATE NOCASE NOT NULL DEFAULT '4' CHECK (length(`body_direction`) <= 1),
  `head_direction` TEXT COLLATE NOCASE NOT NULL DEFAULT '0' CHECK (length(`head_direction`) <= 1),
  `sit` TEXT COLLATE NOCASE NOT NULL DEFAULT '1' CHECK (length(`sit`) <= 1),
  `autotrade` INTEGER NOT NULL DEFAULT 0 CHECK (`autotrade` BETWEEN -128 AND 127),
  PRIMARY KEY (`id`)
);
CREATE TABLE `cart_inventory` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `nameid` INTEGER NOT NULL default '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL default '0' CHECK (`amount` BETWEEN -2147483648 AND 2147483647),
  `equip` INTEGER NOT NULL default '0' CHECK (`equip` BETWEEN 0 AND 4294967295),
  `identify` INTEGER NOT NULL default '0' CHECK (`identify` BETWEEN -32768 AND 32767),
  `refine` INTEGER NOT NULL default '0' CHECK (`refine` BETWEEN 0 AND 255),
  `attribute` INTEGER NOT NULL default '0' CHECK (`attribute` BETWEEN -128 AND 127),
  `card0` INTEGER NOT NULL default '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL default '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL default '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL default '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `expire_time` INTEGER NOT NULL default '0' CHECK (`expire_time` BETWEEN 0 AND 4294967295),
  `bound` INTEGER NOT NULL default '0' CHECK (`bound` BETWEEN 0 AND 255),
  `unique_id` INTEGER NOT NULL default '0' CHECK (`unique_id` >= 0),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255)
);
CREATE TABLE `cashlog` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `time` TEXT NOT NULL,
  `char_id` INTEGER NOT NULL DEFAULT '0' CHECK (`char_id` BETWEEN -2147483648 AND 2147483647),
  `type` TEXT COLLATE NOCASE NOT NULL DEFAULT 'S' CHECK (`type` IN ('T','V','P','M','S','N','D','C','A','E','I','B','$')),
  `cash_type` TEXT COLLATE NOCASE NOT NULL DEFAULT 'O' CHECK (`cash_type` IN ('O','K','C')),
  `amount` INTEGER NOT NULL DEFAULT '0' CHECK (`amount` BETWEEN -2147483648 AND 2147483647),
  `map` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`map`) <= 11)
);
CREATE TABLE `char` (
  `char_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_num` INTEGER NOT NULL default '0' CHECK (`char_num` BETWEEN -128 AND 127),
  `name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`name`) <= 30),
  `class` INTEGER NOT NULL default '0' CHECK (`class` BETWEEN 0 AND 65535),
  `base_level` INTEGER NOT NULL default '1' CHECK (`base_level` BETWEEN 0 AND 65535),
  `job_level` INTEGER NOT NULL default '1' CHECK (`job_level` BETWEEN 0 AND 65535),
  `base_exp` INTEGER NOT NULL default '0' CHECK (`base_exp` >= 0),
  `job_exp` INTEGER NOT NULL default '0' CHECK (`job_exp` >= 0),
  `zeny` INTEGER NOT NULL default '0' CHECK (`zeny` BETWEEN 0 AND 4294967295),
  `str` INTEGER NOT NULL default '0' CHECK (`str` BETWEEN 0 AND 65535),
  `agi` INTEGER NOT NULL default '0' CHECK (`agi` BETWEEN 0 AND 65535),
  `vit` INTEGER NOT NULL default '0' CHECK (`vit` BETWEEN 0 AND 65535),
  `int` INTEGER NOT NULL default '0' CHECK (`int` BETWEEN 0 AND 65535),
  `dex` INTEGER NOT NULL default '0' CHECK (`dex` BETWEEN 0 AND 65535),
  `luk` INTEGER NOT NULL default '0' CHECK (`luk` BETWEEN 0 AND 65535),
  `pow` INTEGER NOT NULL default '0' CHECK (`pow` BETWEEN 0 AND 65535),
  `sta` INTEGER NOT NULL default '0' CHECK (`sta` BETWEEN 0 AND 65535),
  `wis` INTEGER NOT NULL default '0' CHECK (`wis` BETWEEN 0 AND 65535),
  `spl` INTEGER NOT NULL default '0' CHECK (`spl` BETWEEN 0 AND 65535),
  `con` INTEGER NOT NULL default '0' CHECK (`con` BETWEEN 0 AND 65535),
  `crt` INTEGER NOT NULL default '0' CHECK (`crt` BETWEEN 0 AND 65535),
  `max_hp` INTEGER NOT NULL default '0' CHECK (`max_hp` BETWEEN 0 AND 4294967295),
  `hp` INTEGER NOT NULL default '0' CHECK (`hp` BETWEEN 0 AND 4294967295),
  `max_sp` INTEGER NOT NULL default '0' CHECK (`max_sp` BETWEEN 0 AND 4294967295),
  `sp` INTEGER NOT NULL default '0' CHECK (`sp` BETWEEN 0 AND 4294967295),
  `max_ap` INTEGER NOT NULL default '0' CHECK (`max_ap` BETWEEN 0 AND 4294967295),
  `ap` INTEGER NOT NULL default '0' CHECK (`ap` BETWEEN 0 AND 4294967295),
  `status_point` INTEGER NOT NULL default '0' CHECK (`status_point` BETWEEN 0 AND 4294967295),
  `skill_point` INTEGER NOT NULL default '0' CHECK (`skill_point` BETWEEN 0 AND 4294967295),
  `trait_point` INTEGER NOT NULL default '0' CHECK (`trait_point` BETWEEN 0 AND 4294967295),
  `option` INTEGER NOT NULL default '0' CHECK (`option` BETWEEN -2147483648 AND 2147483647),
  `karma` INTEGER NOT NULL default '0' CHECK (`karma` BETWEEN -128 AND 127),
  `manner` INTEGER NOT NULL default '0' CHECK (`manner` BETWEEN -32768 AND 32767),
  `party_id` INTEGER NOT NULL default '0' CHECK (`party_id` BETWEEN 0 AND 4294967295),
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `pet_id` INTEGER NOT NULL default '0' CHECK (`pet_id` BETWEEN 0 AND 4294967295),
  `homun_id` INTEGER NOT NULL default '0' CHECK (`homun_id` BETWEEN 0 AND 4294967295),
  `elemental_id` INTEGER NOT NULL default '0' CHECK (`elemental_id` BETWEEN 0 AND 4294967295),
  `hair` INTEGER NOT NULL default '0' CHECK (`hair` BETWEEN 0 AND 255),
  `hair_color` INTEGER NOT NULL default '0' CHECK (`hair_color` BETWEEN 0 AND 65535),
  `clothes_color` INTEGER NOT NULL default '0' CHECK (`clothes_color` BETWEEN 0 AND 65535),
  `body` INTEGER NOT NULL default '0' CHECK (`body` BETWEEN 0 AND 65535),
  `weapon` INTEGER NOT NULL default '0' CHECK (`weapon` BETWEEN 0 AND 65535),
  `shield` INTEGER NOT NULL default '0' CHECK (`shield` BETWEEN 0 AND 65535),
  `head_top` INTEGER NOT NULL default '0' CHECK (`head_top` BETWEEN 0 AND 65535),
  `head_mid` INTEGER NOT NULL default '0' CHECK (`head_mid` BETWEEN 0 AND 65535),
  `head_bottom` INTEGER NOT NULL default '0' CHECK (`head_bottom` BETWEEN 0 AND 65535),
  `robe` INTEGER NOT NULL DEFAULT '0' CHECK (`robe` BETWEEN 0 AND 65535),
  `last_map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`last_map`) <= 11),
  `last_x` INTEGER NOT NULL default '53' CHECK (`last_x` BETWEEN 0 AND 65535),
  `last_y` INTEGER NOT NULL default '111' CHECK (`last_y` BETWEEN 0 AND 65535),
  `last_instanceid` INTEGER NOT NULL default '0' CHECK (`last_instanceid` BETWEEN 0 AND 4294967295),
  `save_map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`save_map`) <= 11),
  `save_x` INTEGER NOT NULL default '53' CHECK (`save_x` BETWEEN 0 AND 65535),
  `save_y` INTEGER NOT NULL default '111' CHECK (`save_y` BETWEEN 0 AND 65535),
  `partner_id` INTEGER NOT NULL default '0' CHECK (`partner_id` BETWEEN 0 AND 4294967295),
  `online` INTEGER NOT NULL default '0' CHECK (`online` BETWEEN -128 AND 127),
  `father` INTEGER NOT NULL default '0' CHECK (`father` BETWEEN 0 AND 4294967295),
  `mother` INTEGER NOT NULL default '0' CHECK (`mother` BETWEEN 0 AND 4294967295),
  `child` INTEGER NOT NULL default '0' CHECK (`child` BETWEEN 0 AND 4294967295),
  `fame` INTEGER NOT NULL default '0' CHECK (`fame` BETWEEN 0 AND 4294967295),
  `rename` INTEGER NOT NULL default '0' CHECK (`rename` BETWEEN 0 AND 65535),
  `delete_date` INTEGER NOT NULL DEFAULT '0' CHECK (`delete_date` BETWEEN 0 AND 4294967295),
  `moves` INTEGER NOT NULL DEFAULT '0' CHECK (`moves` BETWEEN 0 AND 4294967295),
  `unban_time` INTEGER NOT NULL default '0' CHECK (`unban_time` BETWEEN 0 AND 4294967295),
  `font` INTEGER NOT NULL default '0' CHECK (`font` BETWEEN 0 AND 255),
  `uniqueitem_counter` INTEGER NOT NULL default '0' CHECK (`uniqueitem_counter` BETWEEN 0 AND 4294967295),
  `sex` TEXT COLLATE NOCASE NOT NULL DEFAULT 'M' CHECK (`sex` IN ('M','F')),
  `hotkey_rowshift` INTEGER NOT NULL default '0' CHECK (`hotkey_rowshift` BETWEEN 0 AND 255),
  `hotkey_rowshift2` INTEGER NOT NULL default '0' CHECK (`hotkey_rowshift2` BETWEEN 0 AND 255),
  `clan_id` INTEGER NOT NULL default '0' CHECK (`clan_id` BETWEEN 0 AND 4294967295),
  `last_login` TEXT DEFAULT NULL,
  `title_id` INTEGER NOT NULL default '0' CHECK (`title_id` BETWEEN 0 AND 4294967295),
  `show_equip` INTEGER NOT NULL default '0' CHECK (`show_equip` BETWEEN 0 AND 255),
  `inventory_slots` INTEGER NOT NULL default '100' CHECK (`inventory_slots` BETWEEN -32768 AND 32767),
  `body_direction` INTEGER NOT NULL default '0' CHECK (`body_direction` BETWEEN 0 AND 255),
  `disable_call` INTEGER NOT NULL default '0' CHECK (`disable_call` BETWEEN 0 AND 255),
  `disable_partyinvite` INTEGER NOT NULL default '0' CHECK (`disable_partyinvite` BETWEEN 0 AND 255),
  `disable_showcostumes` INTEGER NOT NULL default '0' CHECK (`disable_showcostumes` BETWEEN 0 AND 255),
  UNIQUE (`name`)
);
CREATE TABLE `char_configs` (
  `world_name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`world_name`) <= 32),
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `data` TEXT COLLATE NOCASE NOT NULL DEFAULT '',
  PRIMARY KEY (`world_name`, `account_id`, `char_id`)
);
CREATE TABLE `char_reg_num` (
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `key` TEXT NOT NULL default '' CHECK (length(`key`) <= 32),
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN 0 AND 4294967295),
  `value` INTEGER NOT NULL default '0' CHECK (`value` BETWEEN -9223372036854775808 AND 9223372036854775807),
  PRIMARY KEY (`char_id`, `key`, `index`)
);
CREATE TABLE `char_reg_str` (
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `key` TEXT NOT NULL default '' CHECK (length(`key`) <= 32),
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN 0 AND 4294967295),
  `value` TEXT COLLATE NOCASE NOT NULL default '0' CHECK (length(`value`) <= 254),
  PRIMARY KEY (`char_id`, `key`, `index`)
);
CREATE TABLE `charlog` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` >= 0),
  `time` TEXT NOT NULL,
  `char_msg` TEXT COLLATE NOCASE NOT NULL default 'char select' CHECK (length(`char_msg`) <= 255),
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_num` INTEGER NOT NULL default '0' CHECK (`char_num` BETWEEN -128 AND 127),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 23),
  `str` INTEGER NOT NULL default '0' CHECK (`str` BETWEEN 0 AND 4294967295),
  `agi` INTEGER NOT NULL default '0' CHECK (`agi` BETWEEN 0 AND 4294967295),
  `vit` INTEGER NOT NULL default '0' CHECK (`vit` BETWEEN 0 AND 4294967295),
  `int` INTEGER NOT NULL default '0' CHECK (`int` BETWEEN 0 AND 4294967295),
  `dex` INTEGER NOT NULL default '0' CHECK (`dex` BETWEEN 0 AND 4294967295),
  `luk` INTEGER NOT NULL default '0' CHECK (`luk` BETWEEN 0 AND 4294967295),
  `hair` INTEGER NOT NULL default '0' CHECK (`hair` BETWEEN -128 AND 127),
  `hair_color` INTEGER NOT NULL default '0' CHECK (`hair_color` BETWEEN -2147483648 AND 2147483647)
);
CREATE TABLE `chatlog` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -9223372036854775808 AND 9223372036854775807),
  `time` TEXT NOT NULL,
  `type` TEXT COLLATE NOCASE NOT NULL default 'O' CHECK (`type` IN ('O','W','P','G','M','C')),
  `type_id` INTEGER NOT NULL default '0' CHECK (`type_id` BETWEEN -2147483648 AND 2147483647),
  `src_charid` INTEGER NOT NULL default '0' CHECK (`src_charid` BETWEEN -2147483648 AND 2147483647),
  `src_accountid` INTEGER NOT NULL default '0' CHECK (`src_accountid` BETWEEN -2147483648 AND 2147483647),
  `src_map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`src_map`) <= 11),
  `src_map_x` INTEGER NOT NULL default '0' CHECK (`src_map_x` BETWEEN -32768 AND 32767),
  `src_map_y` INTEGER NOT NULL default '0' CHECK (`src_map_y` BETWEEN -32768 AND 32767),
  `dst_charname` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`dst_charname`) <= 25),
  `message` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`message`) <= 150)
);
CREATE TABLE `clan` (
  `clan_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`clan_id` BETWEEN 0 AND 4294967295),
  `name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`name`) <= 24),
  `master` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`master`) <= 24),
  `mapname` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`mapname`) <= 24),
  `max_member` INTEGER NOT NULL DEFAULT '0' CHECK (`max_member` BETWEEN 0 AND 65535)
);
INSERT INTO "clan" VALUES(1,'Swordman Clan','Raffam Oranpere','prontera',500);
INSERT INTO "clan" VALUES(2,'Arcwand Clan','Devon Aire','geffen',500);
INSERT INTO "clan" VALUES(3,'Golden Mace Clan','Berman Aire','prontera',500);
INSERT INTO "clan" VALUES(4,'Crossbow Clan','Shaam Rumi','payon',500);
CREATE TABLE `clan_alliance` (
  `clan_id` INTEGER NOT NULL DEFAULT '0' CHECK (`clan_id` BETWEEN 0 AND 4294967295),
  `opposition` INTEGER NOT NULL DEFAULT '0' CHECK (`opposition` BETWEEN 0 AND 4294967295),
  `alliance_id` INTEGER NOT NULL DEFAULT '0' CHECK (`alliance_id` BETWEEN 0 AND 4294967295),
  `name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`name`) <= 24),
  PRIMARY KEY (`clan_id`, `alliance_id`)
);
INSERT INTO "clan_alliance" VALUES(1,0,3,'Golden Mace Clan');
INSERT INTO "clan_alliance" VALUES(2,0,3,'Golden Mace Clan');
INSERT INTO "clan_alliance" VALUES(2,1,4,'Crossbow Clan');
INSERT INTO "clan_alliance" VALUES(3,0,1,'Swordman Clan');
INSERT INTO "clan_alliance" VALUES(3,0,2,'Arcwand Clan');
INSERT INTO "clan_alliance" VALUES(3,0,4,'Crossbow Clan');
INSERT INTO "clan_alliance" VALUES(4,0,3,'Golden Mace Clan');
INSERT INTO "clan_alliance" VALUES(4,1,2,'Arcwand Clan');
CREATE TABLE `db_roulette` (
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN -2147483648 AND 2147483647),
  `level` INTEGER NOT NULL DEFAULT 0 CHECK (`level` BETWEEN 0 AND 65535),
  `item_id` INTEGER NOT NULL DEFAULT 0 CHECK (`item_id` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL DEFAULT '1' CHECK (`amount` BETWEEN 0 AND 65535),
  `flag` INTEGER NOT NULL DEFAULT '1' CHECK (`flag` BETWEEN 0 AND 65535),
  PRIMARY KEY (`index`)
);
INSERT INTO "db_roulette" VALUES(0,1,675,1,1);
INSERT INTO "db_roulette" VALUES(1,1,671,1,0);
INSERT INTO "db_roulette" VALUES(2,1,678,1,0);
INSERT INTO "db_roulette" VALUES(3,1,604,1,0);
INSERT INTO "db_roulette" VALUES(4,1,522,1,0);
INSERT INTO "db_roulette" VALUES(5,1,12609,1,0);
INSERT INTO "db_roulette" VALUES(6,1,12523,1,0);
INSERT INTO "db_roulette" VALUES(7,1,985,1,0);
INSERT INTO "db_roulette" VALUES(8,1,984,1,0);
INSERT INTO "db_roulette" VALUES(9,2,675,1,1);
INSERT INTO "db_roulette" VALUES(10,2,671,1,0);
INSERT INTO "db_roulette" VALUES(11,2,603,1,0);
INSERT INTO "db_roulette" VALUES(12,2,608,1,0);
INSERT INTO "db_roulette" VALUES(13,2,607,1,0);
INSERT INTO "db_roulette" VALUES(14,2,12522,1,0);
INSERT INTO "db_roulette" VALUES(15,2,6223,1,0);
INSERT INTO "db_roulette" VALUES(16,2,6224,1,0);
INSERT INTO "db_roulette" VALUES(17,3,675,1,1);
INSERT INTO "db_roulette" VALUES(18,3,671,1,0);
INSERT INTO "db_roulette" VALUES(19,3,12108,1,0);
INSERT INTO "db_roulette" VALUES(20,3,617,1,0);
INSERT INTO "db_roulette" VALUES(21,3,12514,1,0);
INSERT INTO "db_roulette" VALUES(22,3,7444,1,0);
INSERT INTO "db_roulette" VALUES(23,3,969,1,0);
INSERT INTO "db_roulette" VALUES(24,4,675,1,1);
INSERT INTO "db_roulette" VALUES(25,4,671,1,0);
INSERT INTO "db_roulette" VALUES(26,4,616,1,0);
INSERT INTO "db_roulette" VALUES(27,4,12516,1,0);
INSERT INTO "db_roulette" VALUES(28,4,22777,1,0);
INSERT INTO "db_roulette" VALUES(29,4,6231,1,0);
INSERT INTO "db_roulette" VALUES(30,5,671,1,1);
INSERT INTO "db_roulette" VALUES(31,5,12246,1,0);
INSERT INTO "db_roulette" VALUES(32,5,12263,1,0);
INSERT INTO "db_roulette" VALUES(33,5,12831,1,0);
INSERT INTO "db_roulette" VALUES(34,5,6235,1,0);
INSERT INTO "db_roulette" VALUES(35,6,671,1,1);
INSERT INTO "db_roulette" VALUES(36,6,12766,1,0);
INSERT INTO "db_roulette" VALUES(37,6,6234,1,0);
INSERT INTO "db_roulette" VALUES(38,6,6233,1,0);
INSERT INTO "db_roulette" VALUES(39,7,671,1,1);
INSERT INTO "db_roulette" VALUES(40,7,6233,1,0);
INSERT INTO "db_roulette" VALUES(41,7,6233,1,0);
CREATE TABLE edition_identity (id INTEGER PRIMARY KEY CHECK(id=1), edition TEXT NOT NULL);
INSERT INTO "edition_identity" VALUES(1,'ragswitch-prerenewal');
CREATE TABLE `elemental` (
  `ele_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`ele_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `class` INTEGER NOT NULL default '0' CHECK (`class` BETWEEN 0 AND 16777215),
  `mode` INTEGER NOT NULL default '1' CHECK (`mode` BETWEEN 0 AND 4294967295),
  `hp` INTEGER NOT NULL default '0' CHECK (`hp` BETWEEN 0 AND 4294967295),
  `sp` INTEGER NOT NULL default '0' CHECK (`sp` BETWEEN 0 AND 4294967295),
  `max_hp` INTEGER NOT NULL default '0' CHECK (`max_hp` BETWEEN 0 AND 4294967295),
  `max_sp` INTEGER NOT NULL default '0' CHECK (`max_sp` BETWEEN 0 AND 4294967295),
  `atk1` INTEGER NOT NULL default '0' CHECK (`atk1` BETWEEN 0 AND 16777215),
  `atk2` INTEGER NOT NULL default '0' CHECK (`atk2` BETWEEN 0 AND 16777215),
  `matk` INTEGER NOT NULL default '0' CHECK (`matk` BETWEEN 0 AND 16777215),
  `aspd` INTEGER NOT NULL default '0' CHECK (`aspd` BETWEEN 0 AND 65535),
  `def` INTEGER NOT NULL default '0' CHECK (`def` BETWEEN 0 AND 65535),
  `mdef` INTEGER NOT NULL default '0' CHECK (`mdef` BETWEEN 0 AND 65535),
  `flee` INTEGER NOT NULL default '0' CHECK (`flee` BETWEEN 0 AND 65535),
  `hit` INTEGER NOT NULL default '0' CHECK (`hit` BETWEEN 0 AND 65535),
  `life_time` INTEGER NOT NULL default '0' CHECK (`life_time` BETWEEN -9223372036854775808 AND 9223372036854775807)
);
CREATE TABLE `feedinglog` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `time` TEXT NOT NULL,
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN -2147483648 AND 2147483647),
  `target_id` INTEGER NOT NULL DEFAULT 0 CHECK (`target_id` BETWEEN -2147483648 AND 2147483647),
  `target_class` INTEGER NOT NULL DEFAULT 0 CHECK (`target_class` BETWEEN -32768 AND 32767),
  `type` TEXT COLLATE NOCASE NOT NULL DEFAULT 'P' CHECK (`type` IN ('P','H','O')),
  `intimacy` INTEGER NOT NULL DEFAULT 0 CHECK (`intimacy` BETWEEN 0 AND 4294967295),
  `item_id` INTEGER NOT NULL DEFAULT 0 CHECK (`item_id` BETWEEN 0 AND 4294967295),
  `map` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`map`) <= 11),
  `x` INTEGER NOT NULL DEFAULT 0 CHECK (`x` BETWEEN 0 AND 65535),
  `y` INTEGER NOT NULL DEFAULT 0 CHECK (`y` BETWEEN 0 AND 65535)
);
CREATE TABLE `friends` (
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `friend_id` INTEGER NOT NULL default '0' CHECK (`friend_id` BETWEEN 0 AND 4294967295),
  PRIMARY KEY (`char_id`, `friend_id`)
);
CREATE TABLE `global_acc_reg_num` (
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `key` TEXT NOT NULL default '' CHECK (length(`key`) <= 32),
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN 0 AND 4294967295),
  `value` INTEGER NOT NULL default '0' CHECK (`value` BETWEEN -9223372036854775808 AND 9223372036854775807),
  PRIMARY KEY (`account_id`, `key`, `index`)
);
CREATE TABLE `global_acc_reg_str` (
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `key` TEXT NOT NULL default '' CHECK (length(`key`) <= 32),
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN 0 AND 4294967295),
  `value` TEXT COLLATE NOCASE NOT NULL default '0' CHECK (length(`value`) <= 254),
  PRIMARY KEY (`account_id`, `key`, `index`)
);
CREATE TABLE `guild` (
  `guild_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `master` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`master`) <= 24),
  `guild_lv` INTEGER NOT NULL default '0' CHECK (`guild_lv` BETWEEN 0 AND 255),
  `connect_member` INTEGER NOT NULL default '0' CHECK (`connect_member` BETWEEN 0 AND 255),
  `max_member` INTEGER NOT NULL default '0' CHECK (`max_member` BETWEEN 0 AND 255),
  `average_lv` INTEGER NOT NULL default '1' CHECK (`average_lv` BETWEEN 0 AND 65535),
  `exp` INTEGER NOT NULL default '0' CHECK (`exp` >= 0),
  `next_exp` INTEGER NOT NULL default '0' CHECK (`next_exp` >= 0),
  `skill_point` INTEGER NOT NULL default '0' CHECK (`skill_point` BETWEEN 0 AND 255),
  `mes1` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`mes1`) <= 60),
  `mes2` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`mes2`) <= 120),
  `emblem_len` INTEGER NOT NULL default '0' CHECK (`emblem_len` BETWEEN 0 AND 4294967295),
  `emblem_id` INTEGER NOT NULL default '0' CHECK (`emblem_id` BETWEEN 0 AND 4294967295),
  `emblem_data` BLOB,
  `last_master_change` TEXT,
  UNIQUE (`guild_id`, `char_id`)
);
CREATE TABLE `guild_alliance` (
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `opposition` INTEGER NOT NULL default '0' CHECK (`opposition` BETWEEN 0 AND 4294967295),
  `alliance_id` INTEGER NOT NULL default '0' CHECK (`alliance_id` BETWEEN 0 AND 4294967295),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  PRIMARY KEY (`guild_id`, `alliance_id`)
);
CREATE TABLE `guild_castle` (
  `castle_id` INTEGER NOT NULL default '0' CHECK (`castle_id` BETWEEN 0 AND 4294967295),
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `economy` INTEGER NOT NULL default '0' CHECK (`economy` BETWEEN 0 AND 4294967295),
  `defense` INTEGER NOT NULL default '0' CHECK (`defense` BETWEEN 0 AND 4294967295),
  `triggerE` INTEGER NOT NULL default '0' CHECK (`triggerE` BETWEEN 0 AND 4294967295),
  `triggerD` INTEGER NOT NULL default '0' CHECK (`triggerD` BETWEEN 0 AND 4294967295),
  `nextTime` INTEGER NOT NULL default '0' CHECK (`nextTime` BETWEEN 0 AND 4294967295),
  `payTime` INTEGER NOT NULL default '0' CHECK (`payTime` BETWEEN 0 AND 4294967295),
  `createTime` INTEGER NOT NULL default '0' CHECK (`createTime` BETWEEN 0 AND 4294967295),
  `visibleC` INTEGER NOT NULL default '0' CHECK (`visibleC` BETWEEN 0 AND 4294967295),
  `visibleG0` INTEGER NOT NULL default '0' CHECK (`visibleG0` BETWEEN 0 AND 4294967295),
  `visibleG1` INTEGER NOT NULL default '0' CHECK (`visibleG1` BETWEEN 0 AND 4294967295),
  `visibleG2` INTEGER NOT NULL default '0' CHECK (`visibleG2` BETWEEN 0 AND 4294967295),
  `visibleG3` INTEGER NOT NULL default '0' CHECK (`visibleG3` BETWEEN 0 AND 4294967295),
  `visibleG4` INTEGER NOT NULL default '0' CHECK (`visibleG4` BETWEEN 0 AND 4294967295),
  `visibleG5` INTEGER NOT NULL default '0' CHECK (`visibleG5` BETWEEN 0 AND 4294967295),
  `visibleG6` INTEGER NOT NULL default '0' CHECK (`visibleG6` BETWEEN 0 AND 4294967295),
  `visibleG7` INTEGER NOT NULL default '0' CHECK (`visibleG7` BETWEEN 0 AND 4294967295),
  PRIMARY KEY (`castle_id`)
);
CREATE TABLE `guild_emblems` (
  `world_name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`world_name`) <= 32),
  `guild_id` INTEGER NOT NULL DEFAULT 0 CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `file_type` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`file_type`) <= 255),
  `file_data` BLOB,
  `version` INTEGER NOT NULL default '0' CHECK (`version` BETWEEN 0 AND 4294967295),
  PRIMARY KEY (`world_name`, `guild_id`)
);
CREATE TABLE `guild_expulsion` (
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  `mes` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`mes`) <= 40),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  PRIMARY KEY (`guild_id`, `name`)
);
CREATE TABLE `guild_member` (
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `exp` INTEGER NOT NULL default '0' CHECK (`exp` >= 0),
  `position` INTEGER NOT NULL default '0' CHECK (`position` BETWEEN 0 AND 255),
  PRIMARY KEY (`guild_id`, `char_id`)
);
CREATE TABLE `guild_position` (
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `position` INTEGER NOT NULL default '0' CHECK (`position` BETWEEN 0 AND 255),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  `mode` INTEGER NOT NULL default '0' CHECK (`mode` BETWEEN 0 AND 65535),
  `exp_mode` INTEGER NOT NULL default '0' CHECK (`exp_mode` BETWEEN 0 AND 255),
  PRIMARY KEY (`guild_id`, `position`)
);
CREATE TABLE `guild_skill` (
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `id` INTEGER NOT NULL default '0' CHECK (`id` BETWEEN 0 AND 65535),
  `lv` INTEGER NOT NULL default '0' CHECK (`lv` BETWEEN 0 AND 255),
  PRIMARY KEY (`guild_id`, `id`)
);
CREATE TABLE `guild_storage` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN 0 AND 4294967295),
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `nameid` INTEGER NOT NULL default '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL default '0' CHECK (`amount` BETWEEN 0 AND 4294967295),
  `equip` INTEGER NOT NULL default '0' CHECK (`equip` BETWEEN 0 AND 4294967295),
  `identify` INTEGER NOT NULL default '0' CHECK (`identify` BETWEEN 0 AND 65535),
  `refine` INTEGER NOT NULL default '0' CHECK (`refine` BETWEEN 0 AND 255),
  `attribute` INTEGER NOT NULL default '0' CHECK (`attribute` BETWEEN 0 AND 255),
  `card0` INTEGER NOT NULL default '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL default '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL default '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL default '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `expire_time` INTEGER NOT NULL default '0' CHECK (`expire_time` BETWEEN 0 AND 4294967295),
  `bound` INTEGER NOT NULL default '0' CHECK (`bound` BETWEEN 0 AND 255),
  `unique_id` INTEGER NOT NULL default '0' CHECK (`unique_id` >= 0),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255)
);
CREATE TABLE `guild_storage_log` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `guild_id` INTEGER NOT NULL default '0' CHECK (`guild_id` BETWEEN 0 AND 4294967295),
  `time` TEXT NOT NULL,
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN -2147483648 AND 2147483647),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  `nameid` INTEGER NOT NULL default '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL default '1' CHECK (`amount` BETWEEN -2147483648 AND 2147483647),
  `identify` INTEGER NOT NULL default '0' CHECK (`identify` BETWEEN -32768 AND 32767),
  `refine` INTEGER NOT NULL default '0' CHECK (`refine` BETWEEN 0 AND 255),
  `attribute` INTEGER NOT NULL default '0' CHECK (`attribute` BETWEEN 0 AND 255),
  `card0` INTEGER NOT NULL default '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL default '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL default '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL default '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `expire_time` INTEGER NOT NULL default '0' CHECK (`expire_time` BETWEEN 0 AND 4294967295),
  `unique_id` INTEGER NOT NULL default '0' CHECK (`unique_id` >= 0),
  `bound` INTEGER NOT NULL default '0' CHECK (`bound` BETWEEN 0 AND 255),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255)
);
CREATE TABLE `homunculus` (
  `homun_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`homun_id` BETWEEN -2147483648 AND 2147483647),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `class` INTEGER NOT NULL default '0' CHECK (`class` BETWEEN 0 AND 16777215),
  `prev_class` INTEGER NOT NULL default '0' CHECK (`prev_class` BETWEEN -8388608 AND 8388607),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  `level` INTEGER NOT NULL default '0' CHECK (`level` BETWEEN -32768 AND 32767),
  `exp` INTEGER NOT NULL default '0' CHECK (`exp` >= 0),
  `intimacy` INTEGER NOT NULL default '0' CHECK (`intimacy` BETWEEN -2147483648 AND 2147483647),
  `hunger` INTEGER NOT NULL default '0' CHECK (`hunger` BETWEEN -32768 AND 32767),
  `str` INTEGER NOT NULL default '0' CHECK (`str` BETWEEN 0 AND 65535),
  `agi` INTEGER NOT NULL default '0' CHECK (`agi` BETWEEN 0 AND 65535),
  `vit` INTEGER NOT NULL default '0' CHECK (`vit` BETWEEN 0 AND 65535),
  `int` INTEGER NOT NULL default '0' CHECK (`int` BETWEEN 0 AND 65535),
  `dex` INTEGER NOT NULL default '0' CHECK (`dex` BETWEEN 0 AND 65535),
  `luk` INTEGER NOT NULL default '0' CHECK (`luk` BETWEEN 0 AND 65535),
  `hp` INTEGER NOT NULL default '0' CHECK (`hp` BETWEEN 0 AND 4294967295),
  `max_hp` INTEGER NOT NULL default '0' CHECK (`max_hp` BETWEEN 0 AND 4294967295),
  `sp` INTEGER NOT NULL default '0' CHECK (`sp` BETWEEN 0 AND 4294967295),
  `max_sp` INTEGER NOT NULL default '0' CHECK (`max_sp` BETWEEN 0 AND 4294967295),
  `skill_point` INTEGER NOT NULL default '0' CHECK (`skill_point` BETWEEN 0 AND 65535),
  `alive` INTEGER NOT NULL default '1' CHECK (`alive` BETWEEN -128 AND 127),
  `rename_flag` INTEGER NOT NULL default '0' CHECK (`rename_flag` BETWEEN -128 AND 127),
  `vaporize` INTEGER NOT NULL default '0' CHECK (`vaporize` BETWEEN -128 AND 127),
  `autofeed` INTEGER NOT NULL default '0' CHECK (`autofeed` BETWEEN -128 AND 127)
);
CREATE TABLE `hotkey` (
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `hotkey` INTEGER NOT NULL DEFAULT 0 CHECK (`hotkey` BETWEEN 0 AND 255),
  `type` INTEGER NOT NULL default '0' CHECK (`type` BETWEEN 0 AND 255),
  `itemskill_id` INTEGER NOT NULL default '0' CHECK (`itemskill_id` BETWEEN 0 AND 4294967295),
  `skill_lvl` INTEGER NOT NULL default '0' CHECK (`skill_lvl` BETWEEN 0 AND 255),
  PRIMARY KEY (`char_id`, `hotkey`)
);
CREATE TABLE `interlog` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `time` TEXT NOT NULL,
  `log` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`log`) <= 255)
);
CREATE TABLE `inventory` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `nameid` INTEGER NOT NULL default '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL default '0' CHECK (`amount` BETWEEN 0 AND 4294967295),
  `equip` INTEGER NOT NULL default '0' CHECK (`equip` BETWEEN 0 AND 4294967295),
  `identify` INTEGER NOT NULL default '0' CHECK (`identify` BETWEEN -32768 AND 32767),
  `refine` INTEGER NOT NULL default '0' CHECK (`refine` BETWEEN 0 AND 255),
  `attribute` INTEGER NOT NULL default '0' CHECK (`attribute` BETWEEN 0 AND 255),
  `card0` INTEGER NOT NULL default '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL default '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL default '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL default '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `expire_time` INTEGER NOT NULL default '0' CHECK (`expire_time` BETWEEN 0 AND 4294967295),
  `favorite` INTEGER NOT NULL default '0' CHECK (`favorite` BETWEEN 0 AND 255),
  `bound` INTEGER NOT NULL default '0' CHECK (`bound` BETWEEN 0 AND 255),
  `unique_id` INTEGER NOT NULL default '0' CHECK (`unique_id` >= 0),
  `equip_switch` INTEGER NOT NULL default '0' CHECK (`equip_switch` BETWEEN 0 AND 4294967295),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255)
);
CREATE TABLE `ipbanlist` (
  `list` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`list`) <= 15),
  `btime` TEXT NOT NULL,
  `rtime` TEXT NOT NULL,
  `reason` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`reason`) <= 255),
  PRIMARY KEY (`list`, `btime`)
);
CREATE TABLE `login` (
  `account_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `userid` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`userid`) <= 23),
  `user_pass` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`user_pass`) <= 32),
  `sex` TEXT COLLATE NOCASE NOT NULL default 'M' CHECK (`sex` IN ('M','F','S')),
  `email` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`email`) <= 39),
  `group_id` INTEGER NOT NULL default '0' CHECK (`group_id` BETWEEN -128 AND 127),
  `state` INTEGER NOT NULL default '0' CHECK (`state` BETWEEN 0 AND 4294967295),
  `unban_time` INTEGER NOT NULL default '0' CHECK (`unban_time` BETWEEN 0 AND 4294967295),
  `expiration_time` INTEGER NOT NULL default '0' CHECK (`expiration_time` BETWEEN 0 AND 4294967295),
  `logincount` INTEGER NOT NULL default '0' CHECK (`logincount` BETWEEN 0 AND 16777215),
  `lastlogin` TEXT,
  `last_ip` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`last_ip`) <= 100),
  `birthdate` TEXT,
  `character_slots` INTEGER NOT NULL default '0' CHECK (`character_slots` BETWEEN 0 AND 255),
  `pincode` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`pincode`) <= 4),
  `pincode_change` INTEGER NOT NULL DEFAULT '0' CHECK (`pincode_change` BETWEEN 0 AND 4294967295),
  `vip_time` INTEGER NOT NULL default '0' CHECK (`vip_time` BETWEEN 0 AND 4294967295),
  `old_group` INTEGER NOT NULL default '0' CHECK (`old_group` BETWEEN -128 AND 127),
  `web_auth_token` TEXT COLLATE NOCASE null CHECK (length(`web_auth_token`) <= 17),
  `web_auth_token_enabled` INTEGER NOT NULL default '0' CHECK (`web_auth_token_enabled` BETWEEN -128 AND 127),
  UNIQUE (`web_auth_token`)
);
CREATE TABLE `loginlog` (
  `time` TEXT NOT NULL,
  `ip` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`ip`) <= 15),
  `user` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`user`) <= 23),
  `rcode` INTEGER NOT NULL default '0' CHECK (`rcode` BETWEEN -128 AND 127),
  `log` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`log`) <= 255)
);
CREATE TABLE `mail` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` >= 0),
  `send_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`send_name`) <= 30),
  `send_id` INTEGER NOT NULL default '0' CHECK (`send_id` BETWEEN 0 AND 4294967295),
  `dest_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`dest_name`) <= 30),
  `dest_id` INTEGER NOT NULL default '0' CHECK (`dest_id` BETWEEN 0 AND 4294967295),
  `title` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`title`) <= 45),
  `message` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`message`) <= 500),
  `time` INTEGER NOT NULL default '0' CHECK (`time` BETWEEN 0 AND 4294967295),
  `status` INTEGER NOT NULL default '0' CHECK (`status` BETWEEN -128 AND 127),
  `zeny` INTEGER NOT NULL default '0' CHECK (`zeny` BETWEEN 0 AND 4294967295),
  `type` INTEGER NOT NULL default '0' CHECK (`type` BETWEEN -32768 AND 32767)
);
CREATE TABLE `mail_attachments` (
  `id` INTEGER NOT NULL CHECK (`id` >= 0),
  `index` INTEGER NOT NULL DEFAULT '0' CHECK (`index` BETWEEN 0 AND 65535),
  `nameid` INTEGER NOT NULL DEFAULT '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL DEFAULT '0' CHECK (`amount` BETWEEN 0 AND 4294967295),
  `refine` INTEGER NOT NULL DEFAULT '0' CHECK (`refine` BETWEEN 0 AND 255),
  `attribute` INTEGER NOT NULL DEFAULT '0' CHECK (`attribute` BETWEEN 0 AND 255),
  `identify` INTEGER NOT NULL DEFAULT '0' CHECK (`identify` BETWEEN -32768 AND 32767),
  `card0` INTEGER NOT NULL DEFAULT '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL DEFAULT '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL DEFAULT '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL DEFAULT '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `unique_id` INTEGER NOT NULL DEFAULT '0' CHECK (`unique_id` >= 0),
  `bound` INTEGER NOT NULL DEFAULT '0' CHECK (`bound` BETWEEN 0 AND 255),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255),
  PRIMARY KEY (`id`, `index`),
  FOREIGN KEY (`id`) REFERENCES `mail`(`id`) ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE TABLE `mapreg` (
  `varname` TEXT NOT NULL DEFAULT '' CHECK (length(`varname`) <= 32),
  `index` INTEGER NOT NULL default '0' CHECK (`index` BETWEEN 0 AND 4294967295),
  `value` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`value`) <= 255),
  PRIMARY KEY (`varname`, `index`)
);
CREATE TABLE `market` (
  `name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`name`) <= 50),
  `nameid` INTEGER NOT NULL DEFAULT 0 CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `price` INTEGER NOT NULL DEFAULT 0 CHECK (`price` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL DEFAULT 0 CHECK (`amount` BETWEEN -2147483648 AND 2147483647),
  `flag` INTEGER NOT NULL DEFAULT '0' CHECK (`flag` BETWEEN 0 AND 255),
  PRIMARY KEY (`name`, `nameid`)
);
CREATE TABLE `memo` (
  `memo_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`memo_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`map`) <= 11),
  `x` INTEGER NOT NULL default '0' CHECK (`x` BETWEEN 0 AND 65535),
  `y` INTEGER NOT NULL default '0' CHECK (`y` BETWEEN 0 AND 65535)
);
CREATE TABLE `mercenary` (
  `mer_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`mer_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `class` INTEGER NOT NULL default '0' CHECK (`class` BETWEEN 0 AND 16777215),
  `hp` INTEGER NOT NULL default '0' CHECK (`hp` BETWEEN 0 AND 4294967295),
  `sp` INTEGER NOT NULL default '0' CHECK (`sp` BETWEEN 0 AND 4294967295),
  `kill_counter` INTEGER NOT NULL DEFAULT 0 CHECK (`kill_counter` BETWEEN -2147483648 AND 2147483647),
  `life_time` INTEGER NOT NULL default '0' CHECK (`life_time` BETWEEN -9223372036854775808 AND 9223372036854775807)
);
CREATE TABLE `mercenary_owner` (
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `merc_id` INTEGER NOT NULL default '0' CHECK (`merc_id` BETWEEN 0 AND 4294967295),
  `arch_calls` INTEGER NOT NULL default '0' CHECK (`arch_calls` BETWEEN -2147483648 AND 2147483647),
  `arch_faith` INTEGER NOT NULL default '0' CHECK (`arch_faith` BETWEEN -2147483648 AND 2147483647),
  `spear_calls` INTEGER NOT NULL default '0' CHECK (`spear_calls` BETWEEN -2147483648 AND 2147483647),
  `spear_faith` INTEGER NOT NULL default '0' CHECK (`spear_faith` BETWEEN -2147483648 AND 2147483647),
  `sword_calls` INTEGER NOT NULL default '0' CHECK (`sword_calls` BETWEEN -2147483648 AND 2147483647),
  `sword_faith` INTEGER NOT NULL default '0' CHECK (`sword_faith` BETWEEN -2147483648 AND 2147483647),
  PRIMARY KEY (`char_id`)
);
CREATE TABLE `merchant_configs` (
  `world_name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`world_name`) <= 32),
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `store_type` INTEGER NOT NULL DEFAULT 0 CHECK (`store_type` BETWEEN 0 AND 255),
  `data` TEXT COLLATE NOCASE NOT NULL DEFAULT '',
  PRIMARY KEY (`world_name`, `account_id`, `char_id`, `store_type`)
);
CREATE TABLE `mvplog` (
  `mvp_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`mvp_id` BETWEEN 0 AND 16777215),
  `mvp_date` TEXT NOT NULL,
  `kill_char_id` INTEGER NOT NULL default '0' CHECK (`kill_char_id` BETWEEN -2147483648 AND 2147483647),
  `monster_id` INTEGER NOT NULL default '0' CHECK (`monster_id` BETWEEN -32768 AND 32767),
  `prize` INTEGER NOT NULL default '0' CHECK (`prize` BETWEEN 0 AND 4294967295),
  `mvpexp` INTEGER NOT NULL default '0' CHECK (`mvpexp` >= 0),
  `map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`map`) <= 11)
);
CREATE TABLE `npclog` (
  `npc_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`npc_id` BETWEEN 0 AND 16777215),
  `npc_date` TEXT NOT NULL,
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `char_name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`char_name`) <= 25),
  `map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`map`) <= 11),
  `mes` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`mes`) <= 255)
);
CREATE TABLE `party` (
  `party_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`party_id` BETWEEN 0 AND 4294967295),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  `exp` INTEGER NOT NULL default '0' CHECK (`exp` BETWEEN 0 AND 255),
  `item` INTEGER NOT NULL default '0' CHECK (`item` BETWEEN 0 AND 255),
  `leader_id` INTEGER NOT NULL default '0' CHECK (`leader_id` BETWEEN 0 AND 4294967295),
  `leader_char` INTEGER NOT NULL default '0' CHECK (`leader_char` BETWEEN 0 AND 4294967295)
);
CREATE TABLE `party_bookings` (
  `world_name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`world_name`) <= 32),
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `char_name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`char_name`) <= 23),
  `purpose` INTEGER NOT NULL DEFAULT '0' CHECK (`purpose` BETWEEN 0 AND 65535),
  `assist` INTEGER NOT NULL DEFAULT '0' CHECK (`assist` BETWEEN 0 AND 255),
  `damagedealer` INTEGER NOT NULL DEFAULT '0' CHECK (`damagedealer` BETWEEN 0 AND 255),
  `healer` INTEGER NOT NULL DEFAULT '0' CHECK (`healer` BETWEEN 0 AND 255),
  `tanker` INTEGER NOT NULL DEFAULT '0' CHECK (`tanker` BETWEEN 0 AND 255),
  `minimum_level` INTEGER NOT NULL DEFAULT 0 CHECK (`minimum_level` BETWEEN 0 AND 65535),
  `maximum_level` INTEGER NOT NULL DEFAULT 0 CHECK (`maximum_level` BETWEEN 0 AND 65535),
  `comment` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`comment`) <= 255),
  `created` TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  PRIMARY KEY (`world_name`, `account_id`, `char_id`)
);
CREATE TABLE `pet` (
  `pet_id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`pet_id` BETWEEN 0 AND 4294967295),
  `class` INTEGER NOT NULL default '0' CHECK (`class` BETWEEN 0 AND 16777215),
  `name` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`name`) <= 24),
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `level` INTEGER NOT NULL default '0' CHECK (`level` BETWEEN 0 AND 65535),
  `egg_id` INTEGER NOT NULL default '0' CHECK (`egg_id` BETWEEN 0 AND 4294967295),
  `equip` INTEGER NOT NULL default '0' CHECK (`equip` BETWEEN 0 AND 4294967295),
  `intimate` INTEGER NOT NULL default '0' CHECK (`intimate` BETWEEN 0 AND 65535),
  `hungry` INTEGER NOT NULL default '0' CHECK (`hungry` BETWEEN 0 AND 65535),
  `rename_flag` INTEGER NOT NULL default '0' CHECK (`rename_flag` BETWEEN 0 AND 255),
  `incubate` INTEGER NOT NULL default '0' CHECK (`incubate` BETWEEN 0 AND 4294967295),
  `autofeed` INTEGER NOT NULL default '0' CHECK (`autofeed` BETWEEN -128 AND 127)
);
CREATE TABLE `picklog` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `time` TEXT NOT NULL,
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN -2147483648 AND 2147483647),
  `type` TEXT COLLATE NOCASE NOT NULL default 'P' CHECK (`type` IN ('M','P','L','T','V','S','N','C','A','R','G','E','B','O','I','X','D','U','$','F','Y','Z','Q','H','J','W','0','1','2','3')),
  `nameid` INTEGER NOT NULL default '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL default '1' CHECK (`amount` BETWEEN -2147483648 AND 2147483647),
  `refine` INTEGER NOT NULL default '0' CHECK (`refine` BETWEEN 0 AND 255),
  `card0` INTEGER NOT NULL default '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL default '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL default '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL default '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `unique_id` INTEGER NOT NULL default '0' CHECK (`unique_id` >= 0),
  `map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`map`) <= 11),
  `bound` INTEGER NOT NULL default '0' CHECK (`bound` BETWEEN 0 AND 255),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255)
);
CREATE TABLE `quest` (
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `quest_id` INTEGER NOT NULL DEFAULT 0 CHECK (`quest_id` BETWEEN 0 AND 4294967295),
  `state` TEXT COLLATE NOCASE NOT NULL default '0' CHECK (`state` IN ('0','1','2')),
  `time` INTEGER NOT NULL default '0' CHECK (`time` BETWEEN 0 AND 4294967295),
  `count1` INTEGER NOT NULL default '0' CHECK (`count1` BETWEEN 0 AND 16777215),
  `count2` INTEGER NOT NULL default '0' CHECK (`count2` BETWEEN 0 AND 16777215),
  `count3` INTEGER NOT NULL default '0' CHECK (`count3` BETWEEN 0 AND 16777215),
  PRIMARY KEY (`char_id`, `quest_id`)
);
CREATE TABLE `sales` (
  `nameid` INTEGER NOT NULL DEFAULT 0 CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `start` TEXT NOT NULL,
  `end` TEXT NOT NULL,
  `amount` INTEGER NOT NULL DEFAULT 0 CHECK (`amount` BETWEEN -2147483648 AND 2147483647),
  PRIMARY KEY (`nameid`)
);
CREATE TABLE `sc_data` (
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `type` INTEGER NOT NULL DEFAULT 0 CHECK (`type` BETWEEN 0 AND 65535),
  `tick` INTEGER NOT NULL DEFAULT 0 CHECK (`tick` BETWEEN -9223372036854775808 AND 9223372036854775807),
  `val1` INTEGER NOT NULL default '0' CHECK (`val1` BETWEEN -2147483648 AND 2147483647),
  `val2` INTEGER NOT NULL default '0' CHECK (`val2` BETWEEN -2147483648 AND 2147483647),
  `val3` INTEGER NOT NULL default '0' CHECK (`val3` BETWEEN -2147483648 AND 2147483647),
  `val4` INTEGER NOT NULL default '0' CHECK (`val4` BETWEEN -2147483648 AND 2147483647),
  PRIMARY KEY (`char_id`, `type`)
);
CREATE TABLE `skill` (
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `id` INTEGER NOT NULL default '0' CHECK (`id` BETWEEN 0 AND 65535),
  `lv` INTEGER NOT NULL default '0' CHECK (`lv` BETWEEN 0 AND 255),
  `flag` INTEGER NOT NULL default 0 CHECK (`flag` BETWEEN 0 AND 255),
  PRIMARY KEY (`char_id`, `id`)
);
CREATE TABLE `skill_homunculus` (
  `homun_id` INTEGER NOT NULL DEFAULT 0 CHECK (`homun_id` BETWEEN -2147483648 AND 2147483647),
  `id` INTEGER NOT NULL DEFAULT 0 CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `lv` INTEGER NOT NULL DEFAULT 0 CHECK (`lv` BETWEEN -32768 AND 32767),
  PRIMARY KEY (`homun_id`, `id`)
);
CREATE TABLE `skillcooldown` (
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `skill` INTEGER NOT NULL DEFAULT '0' CHECK (`skill` BETWEEN 0 AND 65535),
  `tick` INTEGER NOT NULL DEFAULT 0 CHECK (`tick` BETWEEN -9223372036854775808 AND 9223372036854775807),
  PRIMARY KEY (`char_id`, `skill`)
);
CREATE TABLE `skillcooldown_homunculus` (
  `homun_id` INTEGER NOT NULL DEFAULT 0 CHECK (`homun_id` BETWEEN -2147483648 AND 2147483647),
  `skill` INTEGER NOT NULL DEFAULT '0' CHECK (`skill` BETWEEN 0 AND 65535),
  `tick` INTEGER NOT NULL DEFAULT 0 CHECK (`tick` BETWEEN -9223372036854775808 AND 9223372036854775807),
  PRIMARY KEY (`homun_id`, `skill`)
);
CREATE TABLE `skillcooldown_mercenary` (
  `mer_id` INTEGER NOT NULL DEFAULT 0 CHECK (`mer_id` BETWEEN 0 AND 4294967295),
  `skill` INTEGER NOT NULL DEFAULT '0' CHECK (`skill` BETWEEN 0 AND 65535),
  `tick` INTEGER NOT NULL DEFAULT 0 CHECK (`tick` BETWEEN -9223372036854775808 AND 9223372036854775807),
  PRIMARY KEY (`mer_id`, `skill`)
);
CREATE TABLE `storage` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN 0 AND 4294967295),
  `account_id` INTEGER NOT NULL default '0' CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `nameid` INTEGER NOT NULL default '0' CHECK (`nameid` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL default '0' CHECK (`amount` BETWEEN 0 AND 65535),
  `equip` INTEGER NOT NULL default '0' CHECK (`equip` BETWEEN 0 AND 4294967295),
  `identify` INTEGER NOT NULL default '0' CHECK (`identify` BETWEEN 0 AND 65535),
  `refine` INTEGER NOT NULL default '0' CHECK (`refine` BETWEEN 0 AND 255),
  `attribute` INTEGER NOT NULL default '0' CHECK (`attribute` BETWEEN 0 AND 255),
  `card0` INTEGER NOT NULL default '0' CHECK (`card0` BETWEEN 0 AND 4294967295),
  `card1` INTEGER NOT NULL default '0' CHECK (`card1` BETWEEN 0 AND 4294967295),
  `card2` INTEGER NOT NULL default '0' CHECK (`card2` BETWEEN 0 AND 4294967295),
  `card3` INTEGER NOT NULL default '0' CHECK (`card3` BETWEEN 0 AND 4294967295),
  `option_id0` INTEGER NOT NULL default '0' CHECK (`option_id0` BETWEEN -32768 AND 32767),
  `option_val0` INTEGER NOT NULL default '0' CHECK (`option_val0` BETWEEN -32768 AND 32767),
  `option_parm0` INTEGER NOT NULL default '0' CHECK (`option_parm0` BETWEEN -128 AND 127),
  `option_id1` INTEGER NOT NULL default '0' CHECK (`option_id1` BETWEEN -32768 AND 32767),
  `option_val1` INTEGER NOT NULL default '0' CHECK (`option_val1` BETWEEN -32768 AND 32767),
  `option_parm1` INTEGER NOT NULL default '0' CHECK (`option_parm1` BETWEEN -128 AND 127),
  `option_id2` INTEGER NOT NULL default '0' CHECK (`option_id2` BETWEEN -32768 AND 32767),
  `option_val2` INTEGER NOT NULL default '0' CHECK (`option_val2` BETWEEN -32768 AND 32767),
  `option_parm2` INTEGER NOT NULL default '0' CHECK (`option_parm2` BETWEEN -128 AND 127),
  `option_id3` INTEGER NOT NULL default '0' CHECK (`option_id3` BETWEEN -32768 AND 32767),
  `option_val3` INTEGER NOT NULL default '0' CHECK (`option_val3` BETWEEN -32768 AND 32767),
  `option_parm3` INTEGER NOT NULL default '0' CHECK (`option_parm3` BETWEEN -128 AND 127),
  `option_id4` INTEGER NOT NULL default '0' CHECK (`option_id4` BETWEEN -32768 AND 32767),
  `option_val4` INTEGER NOT NULL default '0' CHECK (`option_val4` BETWEEN -32768 AND 32767),
  `option_parm4` INTEGER NOT NULL default '0' CHECK (`option_parm4` BETWEEN -128 AND 127),
  `expire_time` INTEGER NOT NULL default '0' CHECK (`expire_time` BETWEEN 0 AND 4294967295),
  `bound` INTEGER NOT NULL default '0' CHECK (`bound` BETWEEN 0 AND 255),
  `unique_id` INTEGER NOT NULL default '0' CHECK (`unique_id` >= 0),
  `enchantgrade` INTEGER NOT NULL default '0' CHECK (`enchantgrade` BETWEEN 0 AND 255)
);
CREATE TABLE `user_configs` (
  `world_name` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`world_name`) <= 32),
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `data` TEXT COLLATE NOCASE NOT NULL DEFAULT '',
  PRIMARY KEY (`world_name`, `account_id`)
);
CREATE TABLE `vending_items` (
  `vending_id` INTEGER NOT NULL DEFAULT 0 CHECK (`vending_id` BETWEEN 0 AND 4294967295),
  `index` INTEGER NOT NULL DEFAULT 0 CHECK (`index` BETWEEN 0 AND 65535),
  `cartinventory_id` INTEGER NOT NULL DEFAULT 0 CHECK (`cartinventory_id` BETWEEN 0 AND 4294967295),
  `amount` INTEGER NOT NULL DEFAULT 0 CHECK (`amount` BETWEEN 0 AND 65535),
  `price` INTEGER NOT NULL DEFAULT 0 CHECK (`price` BETWEEN 0 AND 4294967295),
  PRIMARY KEY (`vending_id`, `index`)
);
CREATE TABLE `vendings` (
  `id` INTEGER NOT NULL DEFAULT 0 CHECK (`id` BETWEEN 0 AND 4294967295),
  `account_id` INTEGER NOT NULL DEFAULT 0 CHECK (`account_id` BETWEEN 0 AND 4294967295),
  `char_id` INTEGER NOT NULL DEFAULT 0 CHECK (`char_id` BETWEEN 0 AND 4294967295),
  `sex` TEXT COLLATE NOCASE NOT NULL DEFAULT 'M' CHECK (`sex` IN ('F','M')),
  `map` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`map`) <= 20),
  `x` INTEGER NOT NULL DEFAULT 0 CHECK (`x` BETWEEN 0 AND 65535),
  `y` INTEGER NOT NULL DEFAULT 0 CHECK (`y` BETWEEN 0 AND 65535),
  `title` TEXT COLLATE NOCASE NOT NULL DEFAULT '' CHECK (length(`title`) <= 80),
  `body_direction` TEXT COLLATE NOCASE NOT NULL DEFAULT '4' CHECK (length(`body_direction`) <= 1),
  `head_direction` TEXT COLLATE NOCASE NOT NULL DEFAULT '0' CHECK (length(`head_direction`) <= 1),
  `sit` TEXT COLLATE NOCASE NOT NULL DEFAULT '1' CHECK (length(`sit`) <= 1),
  `autotrade` INTEGER NOT NULL DEFAULT 0 CHECK (`autotrade` BETWEEN -128 AND 127),
  PRIMARY KEY (`id`)
);
CREATE TABLE `zenylog` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL CHECK (`id` BETWEEN -2147483648 AND 2147483647),
  `time` TEXT NOT NULL,
  `char_id` INTEGER NOT NULL default '0' CHECK (`char_id` BETWEEN -2147483648 AND 2147483647),
  `src_id` INTEGER NOT NULL default '0' CHECK (`src_id` BETWEEN -2147483648 AND 2147483647),
  `type` TEXT COLLATE NOCASE NOT NULL default 'S' CHECK (`type` IN ('T','V','P','M','S','N','D','C','A','E','I','B','K','J','X','0','2')),
  `amount` INTEGER NOT NULL default '0' CHECK (`amount` BETWEEN -2147483648 AND 2147483647),
  `map` TEXT COLLATE NOCASE NOT NULL default '' CHECK (length(`map`) <= 11)
);
CREATE INDEX `acc_reg_num_account_id` ON `acc_reg_num` (`account_id`);
CREATE INDEX `acc_reg_str_account_id` ON `acc_reg_str` (`account_id`);
CREATE INDEX `achievement_char_id` ON `achievement` (`char_id`);
CREATE INDEX `bonus_script_char_id` ON `bonus_script` (`char_id`);
CREATE INDEX `cart_inventory_char_id` ON `cart_inventory` (`char_id`);
CREATE INDEX `char_account_id` ON `char` (`account_id`);
CREATE INDEX `char_party_id` ON `char` (`party_id`);
CREATE INDEX `char_guild_id` ON `char` (`guild_id`);
CREATE INDEX `char_online` ON `char` (`online`);
CREATE INDEX `char_reg_num_char_id` ON `char_reg_num` (`char_id`);
CREATE INDEX `char_reg_str_char_id` ON `char_reg_str` (`char_id`);
CREATE INDEX `charlog_account_id` ON `charlog` (`account_id`);
CREATE INDEX `clan_alliance_alliance_id` ON `clan_alliance` (`alliance_id`);
CREATE INDEX `global_acc_reg_num_account_id` ON `global_acc_reg_num` (`account_id`);
CREATE INDEX `global_acc_reg_str_account_id` ON `global_acc_reg_str` (`account_id`);
CREATE INDEX `guild_char_id` ON `guild` (`char_id`);
CREATE INDEX `guild_alliance_alliance_id` ON `guild_alliance` (`alliance_id`);
CREATE INDEX `guild_castle_guild_id` ON `guild_castle` (`guild_id`);
CREATE INDEX `guild_member_char_id` ON `guild_member` (`char_id`);
CREATE INDEX `guild_storage_guild_id` ON `guild_storage` (`guild_id`);
CREATE INDEX `guild_storage_log_guild_id` ON `guild_storage_log` (`guild_id`);
CREATE INDEX `interlog_time` ON `interlog` (`time`);
CREATE INDEX `inventory_char_id` ON `inventory` (`char_id`);
CREATE INDEX `login_name` ON `login` (`userid`);
CREATE INDEX `memo_char_id` ON `memo` (`char_id`);
CREATE INDEX `storage_account_id` ON `storage` (`account_id`);
CREATE INDEX `atcommandlog_account_id` ON `atcommandlog` (`account_id`);
CREATE INDEX `atcommandlog_char_id` ON `atcommandlog` (`char_id`);
CREATE INDEX `branchlog_account_id` ON `branchlog` (`account_id`);
CREATE INDEX `branchlog_char_id` ON `branchlog` (`char_id`);
CREATE INDEX `cashlog_type` ON `cashlog` (`type`);
CREATE INDEX `chatlog_src_accountid` ON `chatlog` (`src_accountid`);
CREATE INDEX `chatlog_src_charid` ON `chatlog` (`src_charid`);
CREATE INDEX `loginlog_ip` ON `loginlog` (`ip`);
CREATE INDEX `npclog_account_id` ON `npclog` (`account_id`);
CREATE INDEX `npclog_char_id` ON `npclog` (`char_id`);
CREATE INDEX `picklog_type` ON `picklog` (`type`);
CREATE INDEX `zenylog_type` ON `zenylog` (`type`);
INSERT INTO login (account_id,userid,user_pass,sex,email,group_id,character_slots,birthdate) VALUES (1,'s1','p1','S','offline@example.invalid',0,9,'2000-01-01');
INSERT INTO login (account_id,userid,user_pass,sex,email,group_id,character_slots,birthdate) VALUES (2000000,'offline','offline','M','offline@example.invalid',99,9,'2000-01-01');
COMMIT;
