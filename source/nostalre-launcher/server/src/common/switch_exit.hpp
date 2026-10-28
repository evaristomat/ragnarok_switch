#pragma once

#ifdef __SWITCH__
// This declaration intentionally permits C++ unwinding. Calling libc's
// noexcept exit and rewriting its symbol would skip destructors at call sites.
extern "C" [[noreturn]] void nostalre_server_exit(int code);
#else
#define nostalre_server_exit exit
#endif
