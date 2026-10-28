// Copyright (c) rAthena Dev Teams - Licensed under GNU GPL
// For more information, see LICENCE in the main folder

// Backend comparison harness: runs the MySQL constructs rathena relies on through the
// Sql/SqlStmt API and prints a deterministic transcript (lines starting with "|").
// Build it against both backends and diff the transcripts.
//
// Usage (from the server root): sql-compat-test <host> <port> <user> <password> <db> [--prepare <file>]
// --prepare compiles every "<where>\t<sql>" line of <file> instead of running the corpus.
// A freshly created database is expected (sql-files/main.sql, logs.sql, web.sql).

#include <cinttypes>
#include <cstdio>
#include <cstring>
#include <string>

#include <common/cli.hpp>
#include <common/core.hpp>
#include <common/showmsg.hpp>
#include <common/sql.hpp>

using namespace rathena::server_core;

namespace {

Sql* sql;

// Fixed instants so time functions produce comparable output
constexpr const char* T_PAST = "1700000000";    // 2023-11-14
constexpr const char* T_FUTURE = "4102444800";  // 2100-01-01

void out( const char* fmt, ... ){
	va_list ap;

	va_start( ap, fmt );
	fputs( "| ", stdout );
	vfprintf( stdout, fmt, ap );
	fputc( '\n', stdout );
	va_end( ap );
	fflush( stdout );
}

void print_result(){
	char* data;
	size_t len;
	uint32 cols = Sql_NumColumns( sql );

	out( "  rows=%" PRIu64 " cols=%u", Sql_NumRows( sql ), cols );
	while( Sql_NextRow( sql ) == SQL_SUCCESS ){
		std::string line = "  >";

		for( uint32 i = 0; i < cols; ++i ){
			Sql_GetData( sql, i, &data, &len );
			line += " [";
			line += data ? std::string( data, len ) : "\\N";
			line += "]";
		}
		out( "%s", line.c_str() );
	}
	Sql_FreeResult( sql );
}

/// Runs a query and prints its outcome; DML also prints affected rows and insert id.
void query( const char* label, const char* q, bool show_insert_id = false ){
	out( "## %s", label );

	if( Sql_QueryStr( sql, q ) == SQL_ERROR ){
		out( "  ERROR errno_dup=%d", Sql_GetError( sql ) == 1062 ? 1 : 0 );
		return;
	}
	if( Sql_NumColumns( sql ) > 0 ){
		print_result();
		return;
	}
	out( "  ok affected=%" PRIu64, Sql_NumRowsAffected( sql ) );
	if( show_insert_id )
		out( "  insert_id=%" PRIu64, Sql_LastInsertId( sql ) );
}

void test_schema_defaults(){
	query( "auto increment seed (login)", "INSERT INTO `login` (`userid`, `user_pass`, `sex`, `email`) VALUES ('alice', 'pw', 'F', 'a@a.com')", true );
	query( "auto increment next", "INSERT INTO `login` (`userid`, `user_pass`) VALUES ('bob', 'pw')", true );
	query( "implicit defaults", "SELECT * FROM `login` WHERE `userid` = 'bob'" );
	query( "char seed", "INSERT INTO `char` (`account_id`, `char_num`, `name`, `class`, `zeny`, `last_login`) VALUES ('2000000', '0', 'Alice', '0', '500', FROM_UNIXTIME(1700000000))", true );
	query( "char defaults", "SELECT `char_id`, `name`, `zeny`, `base_level`, `str`, `last_map`, `last_x`, `online`, `last_login`, `delete_date`, `title_id` FROM `char`" );
	query( "guild seed (composite pk)", "INSERT INTO `guild` (`name`, `char_id`, `master`) VALUES ('Knights', '150000', 'Alice')", true );
	query( "mail_attachments composite pk", "INSERT INTO `mail_attachments` (`id`, `index`, `nameid`, `amount`) VALUES ('1', '0', '501', '3'), ('1', '1', '502', '1')" );
	query( "mail_attachments read", "SELECT `id`, `index`, `nameid`, `amount` FROM `mail_attachments` ORDER BY `index`" );

	char names[1024];
	out( "## Sql_GetColumnNames(party)" );
	out( "  rc=%d names=%s", Sql_GetColumnNames( sql, "party", names, sizeof( names ), ',' ), names );
}

void test_strings(){
	const char* raw = "O'Brien \\ \"quoted\" 100%";
	char esc[128];

	Sql_EscapeString( sql, esc, raw );
	std::string q = std::string( "UPDATE `char` SET `name` = '" ) + esc + "' WHERE `char_id` = '150000'";
	query( "escaped string write", q.c_str() );
	query( "escaped string read", "SELECT `name`, LENGTH(`name`) FROM `char` WHERE `char_id` = '150000'" );

	Sql_EscapeString( sql, esc, raw );
	q = std::string( "SELECT `char_id` FROM `char` WHERE `name` = '" ) + esc + "'";
	query( "escaped string lookup", q.c_str() );
	query( "restore name", "UPDATE `char` SET `name` = 'Alice' WHERE `char_id` = '150000'" );

	// player text that looks like rewritable SQL must reach the table unchanged
	const char* player_text = "@me NOW() - INTERVAL 1 DAY and BINARY `userid` it's LIMIT 5";
	Sql_EscapeString( sql, esc, player_text );
	q = std::string( "INSERT INTO `atcommandlog` (`atcommand_date`, `account_id`, `char_id`, `char_name`, `map`, `command`) VALUES (NOW(), '2000000', '150000', 'Alice', 'prontera', '" ) + esc + "')";
	query( "player text with SQL keywords (write)", q.c_str() );
	query( "player text with SQL keywords (read)", "SELECT `command` FROM `atcommandlog`" );
	q = std::string( "UPDATE `char` SET `name` = 'Alice LIMIT 5' WHERE `char_id` = '150000' LIMIT 1" );
	query( "literal LIMIT kept, trailing LIMIT stripped", q.c_str() );
	query( "literal LIMIT read", "SELECT `name` FROM `char` WHERE `char_id` = '150000'" );
	query( "restore name again", "UPDATE `char` SET `name` = 'Alice' WHERE `char_id` = '150000'" );

	query( "case-insensitive match", "SELECT `account_id` FROM `login` WHERE `userid` = 'ALICE'" );
	query( "BINARY exact match", "SELECT `account_id` FROM `login` WHERE BINARY `userid` = 'alice'" );
	query( "BINARY case mismatch", "SELECT `account_id` FROM `login` WHERE BINARY `userid` = 'ALICE'" );
	query( "char name BINARY (char.cpp:1375)", "SELECT 1 FROM `char` WHERE BINARY `name` = 'alice' LIMIT 1" );
	query( "LIKE case-insensitive", "SELECT `name` FROM `char` WHERE `name` LIKE 'ali%'" );
	query( "quoted int compare", "SELECT COUNT(*) FROM `char` WHERE `online` = '0' AND `zeny` > '100'" );
	query( "CONCAT", "SELECT CONCAT('a', 'b', 'c'), CONCAT('x', NULL)" );
	query( "LEFT/MD5", "SELECT LEFT(MD5('rathena'), 10), MD5('abc')" );
	query( "web token shape (account.cpp:658)", "SELECT LENGTH(LEFT(SHA2(CONCAT(UUID(), RAND()), 256), 17)), LENGTH(LEFT(MD5(CONCAT(UUID(), RAND())), 17)), LENGTH(LEFT(CONCAT(UUID(), RAND()), 17))" );
	query( "COALESCE", "SELECT COALESCE(NULL, 'fallback'), COALESCE('v', 'fallback')" );
}

void test_time(){
	std::string q;

	query( "FROM_UNIXTIME roundtrip", "SELECT UNIX_TIMESTAMP(FROM_UNIXTIME(1700000000)), UNIX_TIMESTAMP(FROM_UNIXTIME(0)), FROM_UNIXTIME(1700000000) = FROM_UNIXTIME(1700000000)" );
	query( "UNIX_TIMESTAMP(NULL)", "SELECT UNIX_TIMESTAMP(NULL), COALESCE(UNIX_TIMESTAMP(NULL), 0)" );
	query( "NOW() vs UNIX_TIMESTAMP()", "SELECT ABS(UNIX_TIMESTAMP(NOW()) - UNIX_TIMESTAMP()) <= 1" );
	query( "NOW() shape", "SELECT LENGTH(NOW()), NOW() > '2020-01-01 00:00:00', NOW() < '2100-01-01 00:00:00'" );

	// char.cpp:2226 clan cleanup
	query( "NOW() - INTERVAL n DAY (clan cleanup)", "UPDATE `char` SET `clan_id` = '0' WHERE `online` = '0' AND `clan_id` <> '0' AND `last_login` IS NOT NULL AND `last_login` <= NOW() - INTERVAL 3 DAY" );
	query( "set clan", "UPDATE `char` SET `clan_id` = '1' WHERE `char_id` = '150000'" );
	query( "NOW() - INTERVAL n DAY affects old row", "UPDATE `char` SET `clan_id` = '0' WHERE `online` = '0' AND `clan_id` <> '0' AND `last_login` IS NOT NULL AND `last_login` <= NOW() - INTERVAL 3 DAY" );
	query( "clan after cleanup", "SELECT `clan_id` FROM `char` WHERE `char_id` = '150000'" );

	// ipban.cpp:83 / :48 / :103
	query( "ipban insert NOW() + INTERVAL n MINUTE", "INSERT INTO `ipbanlist`(`list`,`btime`,`rtime`,`reason`) VALUES ('10.0.0.*', NOW() , NOW() +  INTERVAL 5 MINUTE ,'Password error ban')" );
	query( "ipban active", "SELECT count(*) FROM `ipbanlist` WHERE `rtime` > NOW() AND (`list` = '10.*.*.*' OR `list` = '10.0.*.*' OR `list` = '10.0.0.*' OR `list` = '10.0.0.1')" );
	query( "ipban duration", "SELECT UNIX_TIMESTAMP(`rtime`) - UNIX_TIMESTAMP(`btime`) FROM `ipbanlist`" );
	query( "ipban expire none", "DELETE FROM `ipbanlist` WHERE `rtime` <= NOW()" );

	// loginlog.cpp:41
	query( "loginlog insert", "INSERT INTO `loginlog`(`time`,`ip`,`user`,`rcode`,`log`) VALUES (NOW(), '127.0.0.1', 'alice', '1', 'Incorrect Password.')" );
	query( "loginlog window", "SELECT count(*) FROM `loginlog` WHERE `ip` = '127.0.0.1' AND (`rcode` = '0' OR `rcode` = '1') AND `time` > NOW() - INTERVAL 5 MINUTE" );

	// int_mail.cpp:254 expiry uses int `time` column
	q = std::string( "INSERT INTO `mail` (`send_name`, `send_id`, `dest_name`, `dest_id`, `title`, `message`, `time`, `status`, `zeny`, `type`) VALUES ('Server', '0', 'Alice', '150000', 'Old', 'msg', '" ) + T_PAST + "', '0', '0', '0')";
	query( "mail insert old", q.c_str(), true );
	q = std::string( "INSERT INTO `mail` (`send_name`, `send_id`, `dest_name`, `dest_id`, `title`, `message`, `time`, `status`, `zeny`, `type`) VALUES ('Server', '0', 'Alice', '150000', 'New', 'msg', '" ) + T_FUTURE + "', '0', '0', '0')";
	query( "mail insert new", q.c_str(), true );
	query( "mail expiry (int_mail.cpp:254)", "DELETE FROM `mail` WHERE `type` = '0' AND `send_id` = '0' AND `time` <= UNIX_TIMESTAMP( NOW() - INTERVAL 7 DAY )" );
	query( "mail left", "SELECT `title` FROM `mail`" );

	// int_achievement.cpp:42 / :124 / :300
	query( "achievement insert", "INSERT INTO `achievement` (`char_id`, `id`, `count1`, `completed`) VALUES ('150000', '1', '5', FROM_UNIXTIME('1700000000'))" );
	query( "achievement read (int_achievement.cpp:42)", "SELECT `id`, COALESCE(UNIX_TIMESTAMP(`completed`),0), COALESCE(UNIX_TIMESTAMP(`rewarded`),0), `count1` FROM `achievement` WHERE `char_id` = '150000'" );
	query( "achievement reward (int_achievement.cpp:300)", "UPDATE `achievement` SET `rewarded` = FROM_UNIXTIME('1700000100') WHERE `char_id`='150000' AND `id` = '1' AND `completed` IS NOT NULL AND `rewarded` IS NULL" );
	query( "achievement reward again (no-op)", "UPDATE `achievement` SET `rewarded` = FROM_UNIXTIME('1700000100') WHERE `char_id`='150000' AND `id` = '1' AND `completed` IS NOT NULL AND `rewarded` IS NULL" );
	query( "achievement read again", "SELECT COALESCE(UNIX_TIMESTAMP(`completed`),0), COALESCE(UNIX_TIMESTAMP(`rewarded`),0) FROM `achievement`" );
	query( "datetime raw roundtrip", "SELECT `completed` = FROM_UNIXTIME(1700000000) FROM `achievement`" );

	// cashshop.cpp:283 / :192 / :415
	q = std::string( "INSERT INTO `sales`(`nameid`,`start`,`end`,`amount`) VALUES ( '501', FROM_UNIXTIME(" ) + T_PAST + "), FROM_UNIXTIME(" + T_FUTURE + "), '10' )";
	query( "sales insert", q.c_str() );
	query( "sales read", "SELECT `nameid`, UNIX_TIMESTAMP(`start`), UNIX_TIMESTAMP(`end`), `amount` FROM `sales` WHERE `end` > NOW()" );
	query( "sales expire none", "DELETE FROM `sales` WHERE `end` < FROM_UNIXTIME(1700000000)" );

	query( "FROM_UNIXTIME edges", "SELECT FROM_UNIXTIME(0) IS NULL, FROM_UNIXTIME(-1) IS NULL, UNIX_TIMESTAMP(FROM_UNIXTIME(1)), UNIX_TIMESTAMP(FROM_UNIXTIME(4294967295)), FROM_UNIXTIME(NULL) IS NULL" );
	query( "UNIX_TIMESTAMP edges", "SELECT UNIX_TIMESTAMP('0000-00-00 00:00:00'), UNIX_TIMESTAMP('garbage'), UNIX_TIMESTAMP('2023-11-14 22:13:20')" );
	query( "guild leader change read (int_guild.cpp:355)", "SELECT COALESCE(UNIX_TIMESTAMP(`last_master_change`),0) FROM `guild`" );
	query( "DATE column", "UPDATE `login` SET `birthdate` = '2000-01-02' WHERE `userid` = 'alice'" );
	query( "DATE read", "SELECT `birthdate` FROM `login` WHERE `userid` = 'alice'" );
}

void test_dml(){
	query( "UPDATE LIMIT 1 (char.cpp:123)", "UPDATE `char` SET `online`='1', `last_login`=FROM_UNIXTIME(1700000000) WHERE `char_id`='150000' LIMIT 1" );
	query( "UPDATE changed value", "UPDATE `char` SET `zeny` = '600' WHERE `char_id` = '150000'" );
	query( "UPDATE same value", "UPDATE `char` SET `zeny` = '600' WHERE `char_id` = '150000'" );
	query( "UPDATE no match", "UPDATE `char` SET `zeny` = '1' WHERE `char_id` = '1'" );
	query( "REPLACE new row", "REPLACE INTO `global_acc_reg_num` (`account_id`, `key`, `index`, `value`) VALUES ('2000000', 'flag', '0', '1')" );
	query( "REPLACE existing row", "REPLACE INTO `global_acc_reg_num` (`account_id`, `key`, `index`, `value`) VALUES ('2000000', 'flag', '0', '2')" );
	query( "REPLACE read", "SELECT `key`, `index`, `value` FROM `global_acc_reg_num`" );
	query( "multi-row insert", "INSERT INTO `inventory` (`char_id`, `nameid`, `amount`) VALUES ('150000', '501', '10'), ('150000', '502', '5'), ('150000', '503', '1')", true );
	query( "LIMIT offset,count", "SELECT `nameid` FROM `inventory` ORDER BY `nameid` LIMIT 1, 2" );
	query( "aggregate", "SELECT COUNT(*), SUM(`amount`), MAX(`nameid`) FROM `inventory` WHERE `char_id` = '150000'" );
	query( "arithmetic", "UPDATE `char` SET `zeny` = `zeny` + 50 WHERE `char_id` = '150000'" );
	query( "arithmetic read", "SELECT `zeny` FROM `char` WHERE `char_id` = '150000'" );
	query( "DELETE LIMIT 1", "DELETE FROM `inventory` WHERE `nameid` = '503' LIMIT 1" );
	query( "IN list", "SELECT `nameid` FROM `inventory` WHERE `nameid` IN ('501','503') ORDER BY `nameid`" );
	query( "JOIN (int_guild.cpp:413 shape)", "SELECT c.`name`, l.`userid` FROM `char` c JOIN `login` l ON c.`account_id` = l.`account_id`" );
	query( "empty result", "SELECT `char_id` FROM `char` WHERE `char_id` = '0'" );

	// item unique ids are (char_id << 32) | counter, written with PRIu64 and read with strtoull
	query( "unique_id write", "UPDATE `inventory` SET `unique_id` = '644245094400005' WHERE `nameid` = '501'" );
	query( "unique_id read", "SELECT `unique_id` FROM `inventory` WHERE `unique_id` = '644245094400005'" );
	query( "insert id, no auto increment", "INSERT INTO `global_acc_reg_str` (`account_id`, `key`, `index`, `value`) VALUES ('2000000', 'note', '0', 'x')", true );
	query( "insert id after UPDATE", "UPDATE `char` SET `zeny` = '1' WHERE `char_id` = '150000'", true );

	// char.cpp:1646 pet egg cleanup: multi-table DELETE and << / | precedence (egg card1=1, card2=1 -> pet 65537)
	query( "pets", "INSERT INTO `pet` (`pet_id`, `class`, `name`, `account_id`, `char_id`) VALUES ('65536', '1002', 'Wrong', '2000000', '150000'), ('65537', '1002', 'Egg', '2000000', '150000'), ('3', '1002', 'Free', '2000000', '150000')" );
	query( "egg", "INSERT INTO `inventory` (`char_id`, `nameid`, `amount`, `card0`, `card1`, `card2`) VALUES ('150000', '9001', '1', '256', '1', '1')" );
	query( "egg pet delete (char.cpp:1646)", "DELETE FROM `pet` USING `pet` JOIN `inventory` ON `pet_id` = (`card1` | (`card2` << 16)) WHERE `inventory`.char_id = '150000' AND card0 = 256" );
	query( "pets left", "SELECT `pet_id`, `name` FROM `pet` ORDER BY `pet_id`" );

	// int_party.cpp:268 leaderless party cleanup
	query( "party without leader", "INSERT INTO `party` (`name`, `leader_id`, `leader_char`) VALUES ('Orphan', '1', '1')" );
	query( "party cleanup (int_party.cpp:268)", "DELETE FROM `party` USING `party` LEFT JOIN `char` ON `party`.leader_id =`char`.account_id AND `party`.leader_char = `char`.char_id WHERE `char`.account_id IS NULL" );
	query( "parties left", "SELECT `name` FROM `party` ORDER BY `name`" );

	// char.cpp:1533 divorce: LIMIT 2 on a non-unique match
	query( "rings", "INSERT INTO `inventory` (`char_id`, `nameid`, `amount`) VALUES ('150000', '2634', '1'), ('150000', '2634', '1'), ('150000', '2634', '1')" );
	query( "delete rings LIMIT 2 (char.cpp:1533)", "DELETE FROM `inventory` WHERE (`nameid`='2634' OR `nameid`='2635') AND (`char_id`='150000' OR `char_id`='0') LIMIT 2" );
	query( "rings left", "SELECT COUNT(*) FROM `inventory` WHERE `nameid` = '2634'" );
	query( "UPDATE ORDER BY LIMIT", "UPDATE `inventory` SET `amount` = '7' WHERE `char_id` = '150000' ORDER BY `nameid` DESC LIMIT 1" );
	query( "UPDATE ORDER BY LIMIT read", "SELECT `nameid`, `amount` FROM `inventory` WHERE `amount` = '7'" );

	// mail_attachments references mail with ON DELETE CASCADE
	query( "mail for attachments", "INSERT INTO `mail` (`send_name`, `dest_id`, `title`, `message`, `time`) VALUES ('Server', '150000', 'Gift', 'x', '1700000000')", true );
	query( "attachment for mail", "INSERT INTO `mail_attachments` (`id`, `index`, `nameid`, `amount`) SELECT MAX(`id`), '0', '607', '1' FROM `mail`" );
	query( "attachments before delete", "SELECT COUNT(*) FROM `mail_attachments`" );
	query( "delete mail (cascade)", "DELETE FROM `mail` WHERE `title` = 'Gift'" );
	query( "attachments after delete", "SELECT COUNT(*) FROM `mail_attachments`" );

	// MySQL's SET form of INSERT/REPLACE (int_guild.cpp:577), values may contain , and =
	query( "REPLACE ... SET (int_guild.cpp:577)", "REPLACE INTO `guild_castle` SET `castle_id`='3', `guild_id`='1', `economy`='5', `defense`='7', `visibleG0`='1'" );
	query( "REPLACE ... SET read", "SELECT `castle_id`, `guild_id`, `economy`, `defense`, `visibleG0`, `visibleG1` FROM `guild_castle` WHERE `castle_id` = '3'" );
	query( "INSERT ... SET with , and = in a value", "INSERT INTO `party` SET `name` = 'a, b = c', `leader_id` = '2000000', `leader_char` = '150000'" );
	query( "INSERT ... SET read", "SELECT `name` FROM `party` WHERE `name` LIKE 'a,%'" );

	// strict mode: values that do not fit the column
	query( "VARCHAR too long", "UPDATE `login` SET `email` = 'abcdefghijklmnopqrstuvwxyz0123456789abcdefghij' WHERE `userid` = 'alice'" );
	query( "UNSIGNED negative", "UPDATE `char` SET `zeny` = -5 WHERE `char_id` = '150000'" );
	query( "SMALLINT UNSIGNED overflow", "UPDATE `char` SET `base_level` = 70000 WHERE `char_id` = '150000'" );
	query( "TINYINT overflow", "UPDATE `login` SET `group_id` = 300 WHERE `userid` = 'alice'" );
	query( "ENUM invalid", "UPDATE `login` SET `sex` = 'X' WHERE `userid` = 'alice'" );
	query( "values after bad writes", "SELECT l.`email`, l.`group_id`, l.`sex`, c.`zeny`, c.`base_level` FROM `login` l JOIN `char` c ON c.`account_id` = l.`account_id`" );

	// collation: trailing spaces and accents
	query( "trailing space match", "SELECT `userid` FROM `login` WHERE `userid` = 'alice '" );
	query( "accent match", "SELECT `userid` FROM `login` WHERE `userid` = 'alíce'" );

	query( "duplicate primary key", "INSERT INTO `login` (`account_id`, `userid`) VALUES ('2000000', 'dup')" );
	query( "duplicate unique key", "INSERT INTO `guild` (`guild_id`, `name`, `char_id`) VALUES ('1', 'Other', '1')" );

	query( "START TRANSACTION", "START TRANSACTION" );
	query( "tx insert", "INSERT INTO `party` (`name`, `leader_id`, `leader_char`) VALUES ('Rolled', '2000000', '150000')" );
	query( "ROLLBACK", "ROLLBACK" );
	query( "START TRANSACTION", "START TRANSACTION" );
	query( "tx insert", "INSERT INTO `party` (`name`, `leader_id`, `leader_char`) VALUES ('Kept', '2000000', '150000')", true );
	query( "COMMIT", "COMMIT" );
	query( "tx result", "SELECT `name` FROM `party`" );

	query( "INSERT DELAYED (log.cpp)", "INSERT DELAYED INTO `zenylog` (`time`, `char_id`, `src_id`, `type`, `amount`, `map`) VALUES (NOW(), '150000', '0', 'M', '100', 'prontera')" );
	query( "log read", "SELECT `char_id`, `type`, `amount`, `map` FROM `zenylog`" );
	out( "## Sql_SetEncoding" );
	out( "  rc=%d", Sql_SetEncoding( sql, "utf8" ) );
}

void test_stmt(){
	SqlStmt stmt( *sql );

	query( "create", "CREATE TABLE `compat_types` (`i8` TINYINT, `i16` SMALLINT, `i32` INT, `i64` BIGINT, `u32` INT UNSIGNED, `u64` BIGINT UNSIGNED, `s` VARCHAR(8), `b` BLOB, `n` INT, `f` DOUBLE)" );

	int8 i8 = -5;
	int16 i16 = -300;
	int32 i32 = -70000;
	int64 i64 = -5000000000LL;
	uint32 u32 = 4000000000U;
	uint64 u64 = 18446744073709551615ULL;
	char s[] = "hello";
	char b[] = { 'a', '\0', 'b', '\1' };
	double f = 1.25;

	int32 rc = stmt.PrepareStr( "INSERT INTO `compat_types` VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)" );
	out( "## stmt prepare insert" );
	out( "  rc=%d params=%zu", rc, stmt.NumParams() );
	stmt.BindParam( 0, SQLDT_INT8, &i8, 0 );
	stmt.BindParam( 1, SQLDT_INT16, &i16, 0 );
	stmt.BindParam( 2, SQLDT_INT32, &i32, 0 );
	stmt.BindParam( 3, SQLDT_INT64, &i64, 0 );
	stmt.BindParam( 4, SQLDT_UINT32, &u32, 0 );
	stmt.BindParam( 5, SQLDT_UINT64, &u64, 0 );
	stmt.BindParam( 6, SQLDT_STRING, s, strlen( s ) );
	stmt.BindParam( 7, SQLDT_BLOB, b, sizeof( b ) );
	stmt.BindParam( 8, SQLDT_NULL, nullptr, 0 );
	stmt.BindParam( 9, SQLDT_DOUBLE, &f, 0 );
	out( "## stmt execute insert" );
	out( "  rc=%d", stmt.Execute() );

	// reuse the statement with new buffer values (buffers are read at execute time)
	i32 = 42;
	strcpy( s, "world" );
	out( "## stmt execute again" );
	out( "  rc=%d", stmt.Execute() );

	query( "stmt rows via Sql", "SELECT `i8`, `i16`, `i32`, `i64`, `u32`, `u64`, `s`, HEX(`b`), `n`, `f` FROM `compat_types` ORDER BY `i32`" );

	int8 r8; int16 r16; int32 r32; int64 r64; uint32 ru32; uint64 ru64; double rf;
	char rs[9], rb[8];
	uint32 len_s, len_b;
	int8 null_n;
	int32 rn;

	rc = stmt.PrepareStr( "SELECT `i8`, `i16`, `i32`, `i64`, `u32`, `u64`, `s`, `b`, `n`, `f` FROM `compat_types` ORDER BY `i32`" );
	int32 exec_rc = stmt.Execute();
	out( "## stmt select" );
	out( "  prepare=%d execute=%d rows=%" PRIu64 " cols=%zu", rc, exec_rc, stmt.NumRows(), stmt.NumColumns() );
	stmt.BindColumn( 0, SQLDT_INT8, &r8 );
	stmt.BindColumn( 1, SQLDT_INT16, &r16 );
	stmt.BindColumn( 2, SQLDT_INT32, &r32 );
	stmt.BindColumn( 3, SQLDT_INT64, &r64 );
	stmt.BindColumn( 4, SQLDT_UINT32, &ru32 );
	stmt.BindColumn( 5, SQLDT_UINT64, &ru64 );
	stmt.BindColumn( 6, SQLDT_STRING, rs, sizeof( rs ), &len_s );
	stmt.BindColumn( 7, SQLDT_BLOB, rb, sizeof( rb ), &len_b );
	stmt.BindColumn( 8, SQLDT_INT32, &rn, 0, nullptr, &null_n );
	stmt.BindColumn( 9, SQLDT_DOUBLE, &rf );
	while( stmt.NextRow() == SQL_SUCCESS ){
		out( "  > %d %d %d %" PRId64 " %u %" PRIu64 " [%s] len_s=%u b=%02x%02x%02x%02x len_b=%u null=%d %.2f",
			r8, r16, r32, r64, ru32, ru64, rs, len_s, (uint8)rb[0], (uint8)rb[1], (uint8)rb[2], (uint8)rb[3], len_b, null_n, rf );
	}

	// string column bound into a too small buffer must fail like MySQL truncation
	char small[4];
	stmt.PrepareStr( "SELECT `s` FROM `compat_types`" );
	stmt.Execute();
	stmt.BindColumn( 0, SQLDT_STRING, small, sizeof( small ) );
	out( "## stmt truncation" );
	out( "  next=%d", stmt.NextRow() );

	// numeric strings bound into ints and int columns bound into strings
	char num_as_str[16];
	int32 str_as_num;
	stmt.PrepareStr( "SELECT `i32`, '123' FROM `compat_types` ORDER BY `i32` LIMIT 1" );
	stmt.Execute();
	stmt.BindColumn( 0, SQLDT_STRING, num_as_str, sizeof( num_as_str ) );
	stmt.BindColumn( 1, SQLDT_INT32, &str_as_num );
	rc = stmt.NextRow();
	out( "## stmt type conversion" );
	out( "  next=%d [%s] %d", rc, num_as_str, str_as_num );

	// insert id through a statement (int_mail.cpp style)
	char title[] = "stmt mail";
	stmt.PrepareStr( "INSERT INTO `mail` (`send_name`, `dest_id`, `title`, `message`, `time`) VALUES ('Server', '150000', ?, 'body', '1700000000')" );
	stmt.BindParam( 0, SQLDT_STRING, title, strlen( title ) );
	rc = stmt.Execute();
	out( "## stmt insert id" );
	out( "  rc=%d id=%" PRIu64, rc, stmt.LastInsertId() );

	// no rows
	stmt.PrepareStr( "SELECT `i8` FROM `compat_types` WHERE `i8` = 99" );
	rc = stmt.Execute();
	uint64 empty_rows = stmt.NumRows();
	out( "## stmt empty" );
	out( "  execute=%d rows=%" PRIu64 " next=%d", rc, empty_rows, stmt.NextRow() );

	// prepare error
	out( "## stmt bad sql" );
	out( "  rc=%d", stmt.PrepareStr( "SELEC nonsense" ) );

	query( "drop", "DROP TABLE `compat_types`" );
}

/// Prepares (compiles, never runs) every query of a file, one per line: "<where>\t<sql>".
void prepare_file( const char* path ){
	FILE* fp = fopen( path, "r" );
	char* line = nullptr;
	size_t capacity = 0;
	ssize_t len;
	SqlStmt stmt( *sql );

	if( fp == nullptr ){
		ShowError( "cannot open %s\n", path );
		return;
	}
	while( ( len = getline( &line, &capacity, fp ) ) != -1 ){
		char* tab = strchr( line, '\t' );

		if( tab == nullptr )
			continue;
		*tab = '\0';
		while( len > 0 && ( line[len - 1] == '\n' || line[len - 1] == '\r' ) )
			line[--len] = '\0';
		ShowInfo( "prepare %s\n", line );
		out( "%s %s", stmt.PrepareStr( tab + 1 ) == SQL_SUCCESS ? "OK " : "ERR", line );
	}
	free( line );
	fclose( fp );
}

class SqlCompatTest : public Core {
	protected:
		bool initialize( int32 argc, char* argv[] ) override {
			if( argc < 6 ){
				ShowError( "usage: sql-compat-test <host> <port> <user> <password> <db>\n" );
				return false;
			}

			sql = Sql_Malloc();
			if( Sql_Connect( sql, argv[3], argv[4], argv[1], (uint16)atoi( argv[2] ), argv[5] ) == SQL_ERROR ){
				ShowError( "connect failed\n" );
				return false;
			}

			if( argc > 7 && strcmp( argv[6], "--prepare" ) == 0 ){
				prepare_file( argv[7] );
			}else{
				test_schema_defaults();
				test_strings();
				test_time();
				test_dml();
				test_stmt();
				out( "## done" );
			}

			Sql_Free( sql );
			return true;
		}

	public:
		SqlCompatTest() : Core( e_core_type::TOOL ){
			this->set_run_once( true );
		}
};

}

// Hooks every rathena executable provides for the shared core
void display_helpscreen( bool ){}
int32 parse_console( const char* ){ return 0; }

int32 main( int32 argc, char* argv[] ){
	return main_core<SqlCompatTest>( argc, argv );
}
