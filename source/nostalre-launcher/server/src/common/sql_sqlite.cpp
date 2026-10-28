#include "switch_exit.hpp"
// Copyright (c) rAthena Dev Teams - Licensed under GNU GPL
// For more information, see LICENCE in the main folder

// SQLite implementation of the Sql/SqlStmt API (built with ENABLE_SQLITE).
// Each Sql handle opens <RA_SQLITE_DIR or cwd>/<db>.db. Queries written for
// MySQL are translated by sql_rewrite() and MySQL-only functions are provided
// as SQLite user functions (see sql_register_functions).

#include "sql.hpp"

#include <algorithm>
#include <cctype>
#include <cinttypes>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <regex>
#include <string>
#include <vector>

#include <sqlite3.h>

#include "cbasetypes.hpp"
#include "cli.hpp"
#include "malloc.hpp"
#include "md5calc.hpp"
#include "random.hpp"
#include "showmsg.hpp"
#include "strlib.hpp"

int32 mysql_reconnect_type;
uint32 mysql_reconnect_count;

/// MySQL error number rathena checks for (duplicate key).
static constexpr uint32 MYSQL_ER_DUP_ENTRY = 1062;

struct SqlValue {
	int32 type = SQLITE_NULL;
	int64 i = 0;
	double d = 0;
	std::string s; // text form of every non-null value; raw bytes for blobs
};

using SqlRow = std::vector<SqlValue>;

struct SqlResult {
	size_t columns = 0;
	std::vector<SqlRow> rows;
	size_t next = 0; // index of the next row to fetch
};

/// Sql handle
struct Sql {
	StringBuf buf;
	sqlite3* db = nullptr;
	SqlResult result;
	SqlRow* row = nullptr;
	std::vector<char*> row_ptrs; // nul-terminated views of row for Sql_GetData
	int64 last_insert_id = 0;
	uint64 rows_affected = 0;
	uint32 last_error = 0;
};

struct SqlBind {
	SqlDataType type = SQLDT_NULL;
	void* buffer = nullptr;
	size_t buffer_len = 0;
	uint32* out_length = nullptr;
	int8* out_is_null = nullptr;
};

struct SqlStmtData {
	Sql* sql;
	sqlite3_stmt* stmt = nullptr;
	std::vector<SqlBind> params;
	std::vector<SqlBind> columns;
	SqlResult result;
	size_t column_count = 0;
	int64 last_insert_id = 0;
};

///////////////////////////////////////////////////////////////////////////////
// Query translation
///////////////////////////////////////////////////////////////////////////////

static bool sql_starts_with( const std::string& s, const char* prefix ){
	size_t i = s.find_first_not_of( " \t\r\n" );
	return i != std::string::npos && strncasecmp( s.c_str() + i, prefix, strlen( prefix ) ) == 0;
}

/// Moves i (at an opening quote) to the matching closing quote; a doubled quote stays inside.
static void sql_skip_quoted( const std::string& q, size_t& i ){
	char c = q[i];

	while( ++i < q.size() && ( q[i] != c || ( i + 1 < q.size() && q[i + 1] == c && ++i ) ) );
}

/// Splits q on sep outside string literals, quoted identifiers and parentheses.
static std::vector<std::string> sql_split_top_level( const std::string& q, char sep ){
	std::vector<std::string> parts;
	size_t start = 0;
	int32 depth = 0;

	for( size_t i = 0; i < q.size(); ++i ){
		char c = q[i];

		if( c == '\'' || c == '"' || c == '`' )
			sql_skip_quoted( q, i );
		else if( c == '(' )
			++depth;
		else if( c == ')' )
			--depth;
		else if( c == sep && depth == 0 ){
			parts.push_back( q.substr( start, i - start ) );
			start = i + 1;
		}
	}
	parts.push_back( q.substr( start ) );
	return parts;
}

/// Position of a keyword outside string literals, quoted identifiers and parentheses.
static size_t sql_find_keyword( const std::string& q, const char* word ){
	size_t len = strlen( word );
	int32 depth = 0;

	for( size_t i = 0; i < q.size(); ++i ){
		char c = q[i];

		if( c == '\'' || c == '"' || c == '`' ){
			sql_skip_quoted( q, i );
			continue;
		}
		if( c == '(' )
			++depth;
		else if( c == ')' )
			--depth;
		else if( depth == 0 && strncasecmp( q.c_str() + i, word, len ) == 0
			&& ( i == 0 || !( isalnum( (unsigned char)q[i - 1] ) || q[i - 1] == '_' ) )
			&& ( i + len == q.size() || !( isalnum( (unsigned char)q[i + len] ) || q[i + len] == '_' ) ) )
			return i;
	}
	return std::string::npos;
}

/// Table of an UPDATE/DELETE statement, e.g. "`char`".
static std::string sql_target_table( const std::string& q ){
	static const std::regex head( R"(^\s*(?:UPDATE|DELETE\s+FROM)\s+(`[^`]+`|\w+))", std::regex::icase );
	std::smatch m;

	return std::regex_search( q, m, head ) ? m[1].str() : "";
}

/// MySQL's INSERT/REPLACE INTO t SET a = x, b = y as INSERT/REPLACE INTO t (a, b) VALUES (x, y).
static std::string sql_rewrite_insert_set( const std::string& q ){
	static const std::regex head( R"(^\s*((?:INSERT|REPLACE)\s+INTO\s+(?:`[^`]+`|\w+))\s+SET\s)", std::regex::icase );
	std::smatch m;

	if( !std::regex_search( q, m, head ) )
		return q;

	std::string columns, values;
	for( const std::string& assignment : sql_split_top_level( q.substr( m.length( 0 ) ), ',' ) ){
		std::vector<std::string> sides = sql_split_top_level( assignment, '=' );
		if( sides.size() < 2 )
			return q;
		columns += ( columns.empty() ? "" : ", " ) + sides[0];
		values += ( values.empty() ? "" : ", " ) + assignment.substr( sides[0].size() + 1 );
	}
	return m[1].str() + " (" + columns + ") VALUES (" + values + ")";
}

/// MySQL statement forms SQLite lacks:
///   INSERT/REPLACE INTO t SET ...             (column = value list)
///   DELETE FROM t USING t JOIN ... WHERE c   (multi-table delete, as a rowid subquery)
///   UPDATE/DELETE ... WHERE c LIMIT n         (the amalgamation has no UPDATE/DELETE LIMIT)
static std::string sql_rewrite_statement( std::string q ){
	if( sql_starts_with( q, "INSERT" ) || sql_starts_with( q, "REPLACE" ) )
		return sql_rewrite_insert_set( q );
	if( !sql_starts_with( q, "UPDATE" ) && !sql_starts_with( q, "DELETE" ) )
		return q;

	std::string table = sql_target_table( q );
	if( table.empty() )
		return q;

	q.erase( q.find_last_not_of( " \t\r\n;" ) + 1 );

	size_t using_pos = sql_find_keyword( q, "USING" );
	if( sql_starts_with( q, "DELETE" ) && using_pos != std::string::npos )
		return "DELETE FROM " + table + " WHERE rowid IN (SELECT " + table + ".rowid FROM" + q.substr( using_pos + strlen( "USING" ) ) + ")";

	if( sql_find_keyword( q, "LIMIT" ) == std::string::npos )
		return q;

	// the filter starts at the first of WHERE / ORDER BY / LIMIT
	size_t filter = std::string::npos;
	for( const char* word : { "WHERE", "ORDER", "LIMIT" } )
		filter = std::min( filter, sql_find_keyword( q, word ) );

	return q.substr( 0, filter ) + "WHERE rowid IN (SELECT rowid FROM " + table + " " + q.substr( filter ) + ")";
}

/// Rewrites one piece of SQL code (never a string literal).
static std::string sql_rewrite_code( std::string code ){
	if( code.find( "BINARY" ) != std::string::npos ){
		static const std::regex binary( R"(\bBINARY\s+(`[^`]+`))" );
		code = std::regex_replace( code, binary, "$1 COLLATE BINARY" );
	}

	if( code.find( "INTERVAL" ) != std::string::npos ){
		static const std::regex interval( R"(NOW\(\)\s*([+-])\s*INTERVAL\s+(\d+)\s+(SECOND|MINUTE|HOUR|DAY|MONTH|YEAR)\b)" );
		code = std::regex_replace( code, interval, "DATETIME('now','localtime','$1$2 $3')" );
	}

	return code;
}

/// Rewrites the MySQL-only syntax used by rathena into SQLite syntax.
/// String literals are copied untouched: they can hold player text such as chat or commands.
static std::string sql_rewrite( const char* query ){
	std::string q( query );

	// IMMEDIATE takes the write lock up front: a deferred transaction that reads first can fail
	// with SQLITE_BUSY instead of waiting when another server process writes in between
	if( sql_starts_with( q, "START TRANSACTION" ) )
		return "BEGIN IMMEDIATE";

	if( sql_starts_with( q, "INSERT DELAYED" ) ){
		size_t pos = q.find_first_not_of( " \t\r\n" ) + strlen( "INSERT" );
		q.erase( pos, strlen( " DELAYED" ) );
	}

	q = sql_rewrite_statement( q );

	std::string out;
	size_t pos = 0;

	while( pos < q.size() ){
		size_t quote = q.find_first_of( "'\"", pos );

		out += sql_rewrite_code( q.substr( pos, quote - pos ) );
		if( quote == std::string::npos )
			break;

		size_t end = quote;
		sql_skip_quoted( q, end );
		out.append( q, quote, end + 1 - quote );
		pos = end + 1;
	}

	return out;
}

///////////////////////////////////////////////////////////////////////////////
// MySQL compatible functions
///////////////////////////////////////////////////////////////////////////////

static void sql_result_datetime( sqlite3_context* ctx, time_t t ){
	struct tm tm;
	char out[32];

	localtime_r( &t, &tm );
	strftime( out, sizeof( out ), "%Y-%m-%d %H:%M:%S", &tm );
	sqlite3_result_text( ctx, out, -1, SQLITE_TRANSIENT );
}

static void sql_fn_now( sqlite3_context* ctx, int, sqlite3_value** ){
	sql_result_datetime( ctx, time( nullptr ) );
}

static void sql_fn_from_unixtime( sqlite3_context* ctx, int, sqlite3_value** argv ){
	sqlite3_int64 t = sqlite3_value_int64( argv[0] );

	// MariaDB's TIMESTAMP range
	if( sqlite3_value_type( argv[0] ) == SQLITE_NULL || t <= 0 || t > 4294967295LL ){
		sqlite3_result_null( ctx );
		return;
	}
	sql_result_datetime( ctx, (time_t)t );
}

static void sql_fn_unix_timestamp( sqlite3_context* ctx, int argc, sqlite3_value** argv ){
	if( argc == 0 ){
		sqlite3_result_int64( ctx, (sqlite3_int64)time( nullptr ) );
		return;
	}

	switch( sqlite3_value_type( argv[0] ) ){
		case SQLITE_NULL:
			sqlite3_result_null( ctx );
			return;
		case SQLITE_INTEGER:
		case SQLITE_FLOAT:
			sqlite3_result_int64( ctx, sqlite3_value_int64( argv[0] ) );
			return;
	}

	struct tm tm = {};
	const char* text = (const char*)sqlite3_value_text( argv[0] );

	// MariaDB returns NULL for unparseable text, the zero date and anything up to the epoch
	if( sscanf( text, "%d-%d-%d %d:%d:%d", &tm.tm_year, &tm.tm_mon, &tm.tm_mday, &tm.tm_hour, &tm.tm_min, &tm.tm_sec ) < 3 || tm.tm_year == 0 ){
		sqlite3_result_null( ctx );
		return;
	}
	tm.tm_year -= 1900;
	tm.tm_mon -= 1;
	tm.tm_isdst = -1;

	time_t t = mktime( &tm );

	if( t <= 0 )
		sqlite3_result_null( ctx );
	else
		sqlite3_result_int64( ctx, (sqlite3_int64)t );
}

static void sql_fn_concat( sqlite3_context* ctx, int argc, sqlite3_value** argv ){
	std::string out;

	for( int i = 0; i < argc; ++i ){
		if( sqlite3_value_type( argv[i] ) == SQLITE_NULL ){
			sqlite3_result_null( ctx );
			return;
		}
		out.append( (const char*)sqlite3_value_text( argv[i] ), sqlite3_value_bytes( argv[i] ) );
	}
	sqlite3_result_text( ctx, out.c_str(), (int)out.size(), SQLITE_TRANSIENT );
}

static void sql_fn_left( sqlite3_context* ctx, int, sqlite3_value** argv ){
	if( sqlite3_value_type( argv[0] ) == SQLITE_NULL ){
		sqlite3_result_null( ctx );
		return;
	}
	std::string s( (const char*)sqlite3_value_text( argv[0] ), sqlite3_value_bytes( argv[0] ) );
	int64 n = sqlite3_value_int64( argv[1] );

	sqlite3_result_text( ctx, s.c_str(), (int)std::min<int64>( std::max<int64>( n, 0 ), (int64)s.size() ), SQLITE_TRANSIENT );
}

static void sql_fn_rand( sqlite3_context* ctx, int, sqlite3_value** ){
	sqlite3_result_double( ctx, rnd() / ( (double)SINT32_MAX + 1 ) );
}

static void sql_fn_uuid( sqlite3_context* ctx, int, sqlite3_value** ){
	unsigned char b[16];
	char out[37];

	sqlite3_randomness( sizeof( b ), b );
	b[6] = ( b[6] & 0x0F ) | 0x40;
	b[8] = ( b[8] & 0x3F ) | 0x80;
	snprintf( out, sizeof( out ), "%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x",
		b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15] );
	sqlite3_result_text( ctx, out, -1, SQLITE_TRANSIENT );
}

static void sql_fn_md5( sqlite3_context* ctx, int, sqlite3_value** argv ){
	char out[33];

	if( sqlite3_value_type( argv[0] ) == SQLITE_NULL ){
		sqlite3_result_null( ctx );
		return;
	}
	MD5_String( (const char*)sqlite3_value_text( argv[0] ), out );
	sqlite3_result_text( ctx, out, 32, SQLITE_TRANSIENT );
}

// Only used to generate random web auth tokens, so a 64 hex char MD5 chain is sufficient.
static void sql_fn_sha2( sqlite3_context* ctx, int, sqlite3_value** argv ){
	char first[33], second[33];

	if( sqlite3_value_type( argv[0] ) == SQLITE_NULL ){
		sqlite3_result_null( ctx );
		return;
	}
	MD5_String( (const char*)sqlite3_value_text( argv[0] ), first );
	MD5_String( first, second );
	std::string out = std::string( first, 32 ) + std::string( second, 32 );
	sqlite3_result_text( ctx, out.c_str(), (int)out.size(), SQLITE_TRANSIENT );
}

static int sql_register_functions( sqlite3* db ){
	struct s_fn {
		const char* name;
		int args;
		void ( *fn )( sqlite3_context*, int, sqlite3_value** );
	};
	static const s_fn functions[] = {
		{ "NOW", 0, sql_fn_now },
		{ "FROM_UNIXTIME", 1, sql_fn_from_unixtime },
		{ "UNIX_TIMESTAMP", 0, sql_fn_unix_timestamp },
		{ "UNIX_TIMESTAMP", 1, sql_fn_unix_timestamp },
		{ "CONCAT", -1, sql_fn_concat },
		{ "LEFT", 2, sql_fn_left },
		{ "RAND", 0, sql_fn_rand },
		{ "UUID", 0, sql_fn_uuid },
		{ "MD5", 1, sql_fn_md5 },
		{ "SHA2", 2, sql_fn_sha2 },
	};

	for( const s_fn& f : functions ){
		int rc = sqlite3_create_function( db, f.name, f.args, SQLITE_UTF8, nullptr, f.fn, nullptr, nullptr );
		if( rc != SQLITE_OK ) return rc;
	}
	return SQLITE_OK;
}

///////////////////////////////////////////////////////////////////////////////
// Shared helpers
///////////////////////////////////////////////////////////////////////////////

static uint32 sql_errno( sqlite3* db ){
	int32 code = sqlite3_extended_errcode( db );

	if( code == SQLITE_CONSTRAINT_UNIQUE || code == SQLITE_CONSTRAINT_PRIMARYKEY )
		return MYSQL_ER_DUP_ENTRY;
	return (uint32)code;
}

static void sql_read_value( sqlite3_stmt* stmt, int col, SqlValue& v ){
	v.type = sqlite3_column_type( stmt, col );
	switch( v.type ){
		case SQLITE_NULL:
			v.s.clear();
			return;
		case SQLITE_INTEGER:
			v.i = sqlite3_column_int64( stmt, col );
			break;
		case SQLITE_FLOAT:
			v.d = sqlite3_column_double( stmt, col );
			break;
		case SQLITE_BLOB:
			v.s.assign( (const char*)sqlite3_column_blob( stmt, col ), sqlite3_column_bytes( stmt, col ) );
			return;
	}
	v.s.assign( (const char*)sqlite3_column_text( stmt, col ), sqlite3_column_bytes( stmt, col ) );
}

/// Affected rows and insert id of the statement that just ran, as MySQL reports them:
/// 0 rows for statements that change nothing, and the first id of a multi-row insert.
static void sql_change_info( sqlite3* db, sqlite3_stmt* stmt, int64 total_before, uint64& rows, int64& insert_id ){
	bool changed = sqlite3_total_changes64( db ) != total_before;
	const char* sql = sqlite3_sql( stmt );
	if( sql == nullptr ){ rows = 0; insert_id = 0; return; }

	while( *sql == ' ' || *sql == '\t' || *sql == '\r' || *sql == '\n' )
		++sql;

	rows = changed ? (uint64)sqlite3_changes64( db ) : 0;
	if( changed && ( strncasecmp( sql, "INSERT", 6 ) == 0 || strncasecmp( sql, "REPLACE", 7 ) == 0 ) )
		insert_id = sqlite3_last_insert_rowid( db ) - (int64)rows + 1;
	else
		insert_id = 0;
}

/// Steps a statement to completion, buffering every row (like mysql_store_result).
static int32 sql_step_all( sqlite3* db, sqlite3_stmt* stmt, SqlResult& out ){
	out = SqlResult();
	out.columns = (size_t)sqlite3_column_count( stmt );

	for( ;; ){
		int32 rc = sqlite3_step( stmt );

		if( rc == SQLITE_DONE )
			return SQL_SUCCESS;
		if( rc != SQLITE_ROW ){
			ShowSQL( "DB error - SQLite %d (extended %d): %s\n", rc, sqlite3_extended_errcode( db ), sqlite3_errmsg( db ) );
			out = SqlResult();
			return SQL_ERROR;
		}

		SqlRow& row = out.rows.emplace_back( out.columns );

		for( size_t i = 0; i < out.columns; ++i ){
			sql_read_value( stmt, (int)i, row[i] );
		}
	}
}

///////////////////////////////////////////////////////////////////////////////
// Sql Handle
///////////////////////////////////////////////////////////////////////////////

Sql* Sql_Malloc(void)
{
	Sql* self;

	CREATE(self, Sql, 1);
	new (self) Sql();
	StringBuf_Init(&self->buf);
	return self;
}

uint32 Sql_GetError( Sql* self ){
	return self ? self->last_error : 0;
}

/// Opens <RA_SQLITE_DIR or cwd>/<db>.db; user, password, host and port are unused.
int32 Sql_Connect(Sql* self, const char* user, const char* passwd, const char* host, uint16 port, const char* db)
{
	if( self == nullptr )
		return SQL_ERROR;
	if( self->db != nullptr ){ self->last_error = SQLITE_MISUSE; return SQL_ERROR; }
	self->last_error = 0;

	StringBuf_Clear(&self->buf);

	const char* dir = getenv( "RA_SQLITE_DIR" );
	std::string path = std::string( dir != nullptr ? dir : "." ) + "/" + db + ".db";

	int rc = sqlite3_open_v2( path.c_str(), &self->db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX |
#ifdef __SWITCH__
        0, // the installer owns creation; a wrong/missing SD path must never create an empty save
#else
        SQLITE_OPEN_CREATE,
#endif
#ifdef __SWITCH__
        "nostalre"
#else
        nullptr
#endif
     );
	if( rc != SQLITE_OK ){
		self->last_error = self->db ? sql_errno(self->db) : (uint32)rc;
		ShowSQL( "Failed to open SQLite database '%s': %s\n", path.c_str(), sqlite3_errmsg( self->db ) );
		sqlite3_close_v2( self->db );
		self->db = nullptr;
		return SQL_ERROR;
	}

	// login, char and map are separate processes sharing one file
	rc = sqlite3_busy_timeout( self->db, 10000 );
	// foreign_keys: mail_attachments relies on ON DELETE CASCADE like on MySQL
	if( rc == SQLITE_OK ) rc = sqlite3_exec( self->db,
#ifdef __SWITCH__
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;"
#else
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;"
#endif
    , nullptr, nullptr, nullptr );
	if( rc == SQLITE_OK ) rc = sql_register_functions( self->db );
	if( rc != SQLITE_OK ){
		self->last_error = sql_errno(self->db);
		if( self->last_error == SQLITE_OK ) self->last_error = (uint32)rc;
		ShowSQL("SQLite connection setup failed: code=%d extended=%u (%s)\n", rc, self->last_error, sqlite3_errmsg(self->db));
		sqlite3_close_v2(self->db);
		self->db = nullptr;
		return SQL_ERROR;
	}

	return SQL_SUCCESS;
}

int32 Sql_GetTimeout(Sql* self, uint32* out_timeout)
{
	return SQL_ERROR;
}

int32 Sql_GetColumnNames(Sql* self, const char* table, char* out_buf, size_t buf_len, char sep)
{
	char* data;
	size_t len;
	size_t off = 0;

	if( self == nullptr || SQL_ERROR == Sql_Query(self, "SELECT `name` FROM pragma_table_info('%s')", table) )
		return SQL_ERROR;

	out_buf[off] = '\0';
	while( SQL_SUCCESS == Sql_NextRow(self) && SQL_SUCCESS == Sql_GetData(self, 0, &data, &len) )
	{
		len = strnlen(data, len);
		if( off + len + 2 > buf_len )
		{
			ShowDebug("Sql_GetColumns: output buffer is too small\n");
			*out_buf = '\0';
			return SQL_ERROR;
		}
		memcpy(out_buf+off, data, len);
		off += len;
		out_buf[off++] = sep;
	}
	out_buf[off] = '\0';
	Sql_FreeResult(self);
	return SQL_SUCCESS;
}

/// SQLite always stores UTF-8 text as given, there is no connection encoding.
int32 Sql_SetEncoding(Sql* self, const char* encoding)
{
	return self ? SQL_SUCCESS : SQL_ERROR;
}

int32 Sql_Ping(Sql* self)
{
	return ( self && self->db ) ? SQL_SUCCESS : SQL_ERROR;
}

size_t Sql_EscapeString(Sql* self, char *out_to, const char *from)
{
	return Sql_EscapeStringLen( self, out_to, from, strlen( from ) );
}

/// Doubles single quotes; backslashes have no special meaning in SQLite literals.
size_t Sql_EscapeStringLen(Sql* self, char *out_to, const char *from, size_t from_len)
{
	size_t n = 0;

	for( size_t i = 0; i < from_len; ++i ){
		if( from[i] == '\0' )
			continue;
		if( from[i] == '\'' )
			out_to[n++] = '\'';
		out_to[n++] = from[i];
	}
	out_to[n] = '\0';
	return n;
}

int32 Sql_Query(Sql* self, const char* query, ...)
{
	int32 res;
	va_list args;

	va_start(args, query);
	res = Sql_QueryV(self, query, args);
	va_end(args);

	return res;
}

int32 Sql_QueryV(Sql* self, const char* query, va_list args)
{
	if( self == nullptr )
		return SQL_ERROR;

	StringBuf_Clear(&self->buf);
	StringBuf_Vprintf(&self->buf, query, args);
	return Sql_QueryStr( self, StringBuf_Value( &self->buf ) );
}

int32 Sql_QueryStr(Sql* self, const char* query)
{
	if( self == nullptr || self->db == nullptr || query == nullptr )
		return SQL_ERROR;

	Sql_FreeResult(self);
	if( query != StringBuf_Value( &self->buf ) ){
		StringBuf_Clear(&self->buf);
		StringBuf_AppendStr(&self->buf, query);
	}

	std::string sql = sql_rewrite( query );
	sqlite3_stmt* stmt;

	self->last_error = 0;
	self->rows_affected = 0;
	self->last_insert_id = 0;
	if( sqlite3_prepare_v2( self->db, sql.c_str(), (int)sql.size(), &stmt, nullptr ) != SQLITE_OK ){
		ShowSQL( "DB error - %s\n", sqlite3_errmsg( self->db ) );
		self->last_error = sql_errno( self->db );
		return SQL_ERROR;
	}
	if( stmt == nullptr ) return SQL_SUCCESS; // whitespace/comments contain no statement

	int64 total_before = sqlite3_total_changes64( self->db );
	int32 res = sql_step_all( self->db, stmt, self->result );

	if( res == SQL_ERROR )
		self->last_error = sql_errno( self->db );
	sql_change_info( self->db, stmt, total_before, self->rows_affected, self->last_insert_id );
	sqlite3_finalize( stmt );
	return res;
}

uint64 Sql_LastInsertId(Sql* self)
{
	return self ? (uint64)self->last_insert_id : 0;
}

uint32 Sql_NumColumns(Sql* self)
{
	return self ? (uint32)self->result.columns : 0;
}

uint64 Sql_NumRows(Sql* self)
{
	return self ? (uint64)self->result.rows.size() : 0;
}

uint64 Sql_NumRowsAffected(Sql* self)
{
	return self ? self->rows_affected : 0;
}

int32 Sql_NextRow(Sql* self)
{
	if( self == nullptr )
		return SQL_ERROR;

	SqlResult& res = self->result;

	if( res.next >= res.rows.size() ){
		self->row = nullptr;
		return SQL_NO_DATA;
	}

	self->row = &res.rows[res.next++];
	self->row_ptrs.resize( res.columns );
	for( size_t i = 0; i < res.columns; ++i ){
		SqlValue& v = ( *self->row )[i];
		self->row_ptrs[i] = v.type == SQLITE_NULL ? nullptr : v.s.data();
	}
	return SQL_SUCCESS;
}

int32 Sql_GetData(Sql* self, size_t col, char** out_buf, size_t* out_len)
{
	if( self == nullptr || self->row == nullptr )
		return SQL_ERROR;

	if( col < self->result.columns ){
		if( out_buf ) *out_buf = self->row_ptrs[col];
		if( out_len ) *out_len = ( *self->row )[col].s.size();
	}else{// out of range - ignore
		if( out_buf ) *out_buf = nullptr;
		if( out_len ) *out_len = 0;
	}
	return SQL_SUCCESS;
}

void Sql_FreeResult(Sql* self)
{
	if( self ){
		self->result = SqlResult();
		self->row = nullptr;
	}
}

void Sql_ShowDebug_(Sql* self, const char* debug_file, const unsigned long debug_line)
{
	if( self == nullptr )
		ShowDebug("at %s:%lu - self is nullptr\n", debug_file, debug_line);
	else if( StringBuf_Length(&self->buf) > 0 )
		ShowDebug("at %s:%lu - %s\n", debug_file, debug_line, StringBuf_Value(&self->buf));
	else
		ShowDebug("at %s:%lu\n", debug_file, debug_line);
}

void Sql_Free(Sql* self)
{
	if( self )
	{
		Sql_FreeResult(self);
		if( self->db )
			sqlite3_close_v2( self->db );
		self->~Sql();
		aFree(self);
	}
}

///////////////////////////////////////////////////////////////////////////////
// Prepared Statements
///////////////////////////////////////////////////////////////////////////////

static bool sql_type_is_unsigned( SqlDataType type ){
	switch( type ){
		case SQLDT_UINT8: case SQLDT_UINT16: case SQLDT_UINT32: case SQLDT_UINT64:
		case SQLDT_UCHAR: case SQLDT_ULONG: case SQLDT_ULONGLONG:
			return true;
		default:
			return false;
	}
}

/// Byte size of an integer data type, 0 if not an integer.
static size_t sql_type_int_size( SqlDataType type ){
	switch( type ){
		case SQLDT_INT8: case SQLDT_UINT8: return 1;
		case SQLDT_INT16: case SQLDT_UINT16: return 2;
		case SQLDT_INT32: case SQLDT_UINT32: return 4;
		case SQLDT_INT64: case SQLDT_UINT64: return 8;
		case SQLDT_CHAR: case SQLDT_UCHAR: return sizeof( char );
		case SQLDT_LONG: case SQLDT_ULONG: return sizeof( long );
		case SQLDT_LONGLONG: case SQLDT_ULONGLONG: return sizeof( int64 );
		default: return 0;
	}
}

static int64 sql_load_int( const void* buffer, size_t size, bool is_unsigned ){
	switch( size ){
		case 1: return is_unsigned ? (int64)*(const uint8*)buffer : (int64)*(const int8*)buffer;
		case 2: return is_unsigned ? (int64)*(const uint16*)buffer : (int64)*(const int16*)buffer;
		case 4: return is_unsigned ? (int64)*(const uint32*)buffer : (int64)*(const int32*)buffer;
		default: return *(const int64*)buffer;
	}
}

static void sql_store_int( void* buffer, size_t size, int64 value ){
	switch( size ){
		case 1: *(int8*)buffer = (int8)value; break;
		case 2: *(int16*)buffer = (int16)value; break;
		case 4: *(int32*)buffer = (int32)value; break;
		default: *(int64*)buffer = value; break;
	}
}

static int64 sql_value_int( const SqlValue& v ){
	switch( v.type ){
		case SQLITE_INTEGER: return v.i;
		case SQLITE_FLOAT: return (int64)v.d;
		case SQLITE_NULL: return 0;
		default: return (int64)strtoull( v.s.c_str(), nullptr, 10 );
	}
}

static double sql_value_double( const SqlValue& v ){
	switch( v.type ){
		case SQLITE_INTEGER: return (double)v.i;
		case SQLITE_FLOAT: return v.d;
		case SQLITE_NULL: return 0;
		default: return strtod( v.s.c_str(), nullptr );
	}
}

static int32 sql_bind_param( sqlite3_stmt* stmt, int idx, const SqlBind& b ){
	if( size_t size = sql_type_int_size( b.type ); size > 0 )
		return sqlite3_bind_int64( stmt, idx, sql_load_int( b.buffer, size, sql_type_is_unsigned( b.type ) ) );

	switch( b.type ){
		case SQLDT_NULL:
			return sqlite3_bind_null( stmt, idx );
		case SQLDT_FLOAT:
			return sqlite3_bind_double( stmt, idx, *(const float*)b.buffer );
		case SQLDT_DOUBLE:
			return sqlite3_bind_double( stmt, idx, *(const double*)b.buffer );
		case SQLDT_STRING:
		case SQLDT_ENUM:
			return sqlite3_bind_text( stmt, idx, (const char*)b.buffer, (int)b.buffer_len, SQLITE_TRANSIENT );
		case SQLDT_BLOB:
			return sqlite3_bind_blob( stmt, idx, b.buffer, (int)b.buffer_len, SQLITE_TRANSIENT );
		default:
			ShowDebug( "SqlStmt: unsupported buffer type (%d)\n", b.type );
			return SQLITE_MISUSE;
	}
}

static bool sql_type_supported( SqlDataType type ){
	return type >= SQLDT_NULL && type < SQLDT_LASTID;
}

/// Copies a fetched value into a bound column; false when it does not fit.
static bool sql_fetch_column( const SqlValue& v, SqlBind& b ){
	bool is_null = v.type == SQLITE_NULL;
	size_t length = 0;

	if( b.out_is_null )
		*b.out_is_null = is_null ? 1 : 0;

	if( size_t size = sql_type_int_size( b.type ); size > 0 ){
		sql_store_int( b.buffer, size, sql_value_int( v ) );
		length = size;
	}else switch( b.type ){
		case SQLDT_NULL:
			break;
		case SQLDT_FLOAT:
			*(float*)b.buffer = (float)sql_value_double( v );
			length = sizeof( float );
			break;
		case SQLDT_DOUBLE:
			*(double*)b.buffer = sql_value_double( v );
			length = sizeof( double );
			break;
		case SQLDT_STRING:
		case SQLDT_ENUM:
			// buffer_len excludes the nul-terminator, see SqlStmt::BindColumn
			length = v.s.size();
			if( length > b.buffer_len )
				return false;
			memcpy( b.buffer, v.s.data(), length );
			memset( (char*)b.buffer + length, 0, b.buffer_len - length + 1 );
			break;
		case SQLDT_BLOB:
			length = v.s.size();
			if( length > b.buffer_len )
				return false;
			memcpy( b.buffer, v.s.data(), length );
			memset( (char*)b.buffer + length, 0, b.buffer_len - length );
			break;
		default:
			break;
	}

	if( b.out_length )
		*b.out_length = (uint32)length;
	return true;
}

SqlStmt::SqlStmt( Sql& sql ){
	StringBuf_Init( &this->buf );
	this->data = new SqlStmtData();
	this->data->sql = &sql;
}

int32 SqlStmt::Prepare(const char* query, ...){
	int32 res;
	va_list args;

	va_start(args, query);
	res = this->PrepareV(query, args);
	va_end(args);

	return res;
}

int32 SqlStmt::PrepareV(const char* query, va_list args){
	StringBuf_Clear( &this->buf );
	StringBuf_Vprintf( &this->buf, query, args );
	return this->PrepareStr( StringBuf_Value( &this->buf ) );
}

int32 SqlStmt::PrepareStr(const char* query){
	this->FreeResult();
	if( query != StringBuf_Value( &this->buf ) ){
		StringBuf_Clear( &this->buf );
		StringBuf_AppendStr( &this->buf, query );
	}

	SqlStmtData& d = *this->data;
	sqlite3* db = d.sql->db;
	std::string sql = sql_rewrite( query );

	sqlite3_finalize( d.stmt );
	d.stmt = nullptr;
	d.params.clear();
	d.column_count = 0;

	if( sqlite3_prepare_v2( db, sql.c_str(), (int)sql.size(), &d.stmt, nullptr ) != SQLITE_OK ){
		ShowSQL( "DB error - %s\n", sqlite3_errmsg( db ) );
		d.sql->last_error = sql_errno( db );
		return SQL_ERROR;
	}

	d.params.resize( (size_t)sqlite3_bind_parameter_count( d.stmt ) );
	d.column_count = (size_t)sqlite3_column_count( d.stmt );
	return SQL_SUCCESS;
}

size_t SqlStmt::NumParams(){
	return this->data->params.size();
}

int32 SqlStmt::BindParam(size_t idx, enum SqlDataType buffer_type, void* buffer, size_t buffer_len){
	if( !sql_type_supported( buffer_type ) ){
		ShowDebug( "SqlStmt::BindParam: unsupported buffer type (%d)\n", buffer_type );
		return SQL_ERROR;
	}

	if( idx < this->data->params.size() ){
		SqlBind& b = this->data->params[idx];

		b.type = buffer_type;
		b.buffer = buffer;
		b.buffer_len = buffer_len;
		return SQL_SUCCESS;
	}
	this->data->sql->last_error = SQLITE_RANGE;
	return SQL_ERROR;
}

int32 SqlStmt::Execute(){
	SqlStmtData& d = *this->data;
	sqlite3* db = d.sql->db;

	this->FreeResult();
	if( d.stmt == nullptr )
		return SQL_ERROR;

	sqlite3_reset( d.stmt );
	sqlite3_clear_bindings( d.stmt );
	for( size_t i = 0; i < d.params.size(); ++i ){
		int rc = sql_bind_param( d.stmt, (int)i + 1, d.params[i] );
		if( rc != SQLITE_OK ){
			d.sql->last_error = (uint32)rc;
			ShowSQL( "DB error - %s\n", sqlite3_errmsg( db ) );
			return SQL_ERROR;
		}
	}

	d.columns.assign( d.column_count, SqlBind() );

	int64 total_before = sqlite3_total_changes64( db );
	int32 res = sql_step_all( db, d.stmt, d.result );
	uint64 rows;

	d.sql->last_error = res == SQL_ERROR ? sql_errno( db ) : 0;
	sql_change_info( db, d.stmt, total_before, rows, d.last_insert_id );
	sqlite3_reset( d.stmt );
	return res;
}

uint64 SqlStmt::LastInsertId(){
	return (uint64)this->data->last_insert_id;
}

size_t SqlStmt::NumColumns(){
	return this->data->column_count;
}

int32 SqlStmt::BindColumn(size_t idx, enum SqlDataType buffer_type, void* buffer, size_t buffer_len, uint32* out_length, int8* out_is_null){
	if( buffer_type == SQLDT_STRING || buffer_type == SQLDT_ENUM )
	{
		if( buffer_len < 1 )
		{
			ShowDebug("SqlStmt::BindColumn: buffer_len(%" PRIuPTR ") is too small, no room for the nul-terminator\n", buffer_len);
			return SQL_ERROR;
		}
		--buffer_len;// nul-terminator
	}

	if( !sql_type_supported( buffer_type ) ){
		ShowDebug( "SqlStmt::BindColumn: unsupported buffer type (%d)\n", buffer_type );
		return SQL_ERROR;
	}

	SqlStmtData& d = *this->data;

	if( d.columns.size() < d.column_count )
		d.columns.assign( d.column_count, SqlBind() );

	if( idx < d.columns.size() ){
		d.columns[idx] = SqlBind{ buffer_type, buffer, buffer_len, out_length, out_is_null };
	}
	// out of range - ignore
	return SQL_SUCCESS;
}

uint64 SqlStmt::NumRows(){
	return (uint64)this->data->result.rows.size();
}

int32 SqlStmt::NextRow(){
	SqlResult& res = this->data->result;

	if( res.next >= res.rows.size() )
		return SQL_NO_DATA;

	SqlRow& row = res.rows[res.next++];

	for( size_t i = 0; i < this->data->columns.size() && i < row.size(); ++i ){
		SqlBind& b = this->data->columns[i];

		if( b.buffer == nullptr && b.type != SQLDT_NULL )
			continue;
		if( !sql_fetch_column( row[i], b ) ){
			this->ShowDebugTruncatedColumn( i );
			return SQL_ERROR;
		}
	}
	return SQL_SUCCESS;
}

void SqlStmt::ShowDebugTruncatedColumn( size_t i ){
	const char* name = this->data->stmt ? sqlite3_column_name( this->data->stmt, (int)i ) : nullptr;

	ShowSQL( "DB error - data of field '%s' was truncated.\n", name ? name : "?" );
	ShowDebug( "column - %lu, buffer length - %lu\n", (unsigned long)i, (unsigned long)this->data->columns[i].buffer_len );
}

void SqlStmt::FreeResult(){
	this->data->result = SqlResult();
}

void SqlStmt::ShowDebug_(const char* debug_file, const unsigned long debug_line){
#if !defined(SQL_REMOVE_SHOWDEBUG)
	if( StringBuf_Length( &this->buf ) > 0 ){
		ShowDebug("at %s:%lu - %s\n", debug_file, debug_line, StringBuf_Value(&this->buf));
	}else{
		ShowDebug("at %s:%lu\n", debug_file, debug_line);
	}
#endif
}

SqlStmt::~SqlStmt(){
	if( this->data != nullptr ){
		sqlite3_finalize( this->data->stmt );
		delete this->data;
		this->data = nullptr;
	}
}

///////////////////////////////////////////////////////////////////////////////
// Configuration
///////////////////////////////////////////////////////////////////////////////

// Reconnect settings are irrelevant for a local file, read them only to keep the config format.
static void Sql_inter_server_read(const char* cfgName, bool first) {
	char line[1024], w1[1024], w2[1024];
	FILE* fp;

	fp = fopen(cfgName, "r");
	if(fp == nullptr) {
		if( first ) {
			ShowFatalError("File not found: %s\n", cfgName);
			nostalre_server_exit(EXIT_FAILURE);
		} else
			ShowError("File not found: %s\n", cfgName);
		return;
	}

	while(fgets(line, sizeof(line), fp)) {
		if(sscanf(line, "%1023[^:]: %1023[^\r\n]", w1, w2) != 2)
			continue;

		if(!strcmpi(w1,"mysql_reconnect_type"))
			mysql_reconnect_type = atoi(w2);
		else if(!strcmpi(w1,"mysql_reconnect_count"))
			mysql_reconnect_count = atoi(w2);
		else if(!strcmpi(w1,"import"))
			Sql_inter_server_read(w2,false);
	}
	fclose(fp);
}

void Sql_Init(void) {
	Sql_inter_server_read(INTER_CONF_NAME,true);
}
