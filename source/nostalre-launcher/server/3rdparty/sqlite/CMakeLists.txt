
#
# SQLite stands in for the MySQL client library (ENABLE_SQLITE).
# Uses the bundled amalgamation (sqlite3.c) when present, else the system library.
# Exports the MYSQL_* variables so the server targets link it unchanged.
#
if( EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/sqlite3.c" )
	message( STATUS "Configuring for local SQLite" )
	add_library( sqlite3 STATIC "${CMAKE_CURRENT_SOURCE_DIR}/sqlite3.c" )
	target_compile_definitions( sqlite3 PRIVATE
		SQLITE_DQS=3
		SQLITE_THREADSAFE=1
		SQLITE_DEFAULT_WAL_SYNCHRONOUS=1
		SQLITE_OMIT_LOAD_EXTENSION
		SQLITE_OMIT_DEPRECATED )
	if(NOSTALRE_SWITCH)
        target_compile_definitions(sqlite3 PRIVATE SQLITE_OS_UNIX=1 SQLITE_DISABLE_LFS=1
            SQLITE_OMIT_WAL SQLITE_MAX_MMAP_SIZE=0 SQLITE_ENABLE_LOCKING_STYLE=0)
    endif()
    set_target_properties( sqlite3 PROPERTIES POSITION_INDEPENDENT_CODE ON )
	target_link_libraries( sqlite3 ${CMAKE_THREAD_LIBS_INIT} ${CMAKE_DL_LIBS} )
	set( SQLITE_LIBRARIES sqlite3 )
	set( SQLITE_INCLUDE_DIRS "${CMAKE_CURRENT_SOURCE_DIR}" )
else()
	find_package( SQLite3 REQUIRED )
	message( STATUS "Configuring for system SQLite ${SQLite3_VERSION}" )
	set( SQLITE_LIBRARIES ${SQLite3_LIBRARIES} )
	set( SQLITE_INCLUDE_DIRS ${SQLite3_INCLUDE_DIRS} )
endif()

set( MYSQL_LIBRARIES ${SQLITE_LIBRARIES} CACHE INTERNAL "" )
set( MYSQL_INCLUDE_DIRS ${SQLITE_INCLUDE_DIRS} CACHE INTERNAL "" )
set( WITH_MYSQL ON CACHE INTERNAL "" )
