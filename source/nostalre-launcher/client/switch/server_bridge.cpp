/* Embedded rAthena fatal exits must return to the launcher, not terminate the
 * Switch process and destroy graphics/services beneath other running threads.
 * Only embedded servers explicitly route their fatal exits to this function.
 * The C++ exception is caught here before it can cross the Rust FFI boundary.
 */
#include <cstdio>
#ifdef __SWITCH__
extern "C" const char __eh_frame_start[];
extern "C" void __register_frame(const void *);
extern "C" void *_Unwind_Find_FDE(const void *, void *);
#endif

namespace {
struct ServerExit { int code; };
}

extern "C" [[noreturn]] void nostalre_server_exit(int code) {
    throw ServerExit{code};
}

extern "C" int nostalre_server_call(int (*entry)(int, char **), int argc, char **argv) {
    try {
        return entry(argc, argv);
    } catch (const ServerExit &error) {
        std::fprintf(stderr, "[Server] %s requested exit(%d); returning to launcher\n",
                     argc > 0 && argv && argv[0] ? argv[0] : "unknown", error.code);
        return error.code;
    } catch (...) {
        std::fprintf(stderr, "[Server] uncaught C++ exception; returning to launcher\n");
        return 1;
    }
}

static int boundary_probe(int, char **) { nostalre_server_exit(37); }

extern "C" int nostalre_server_runtime_init(void) {
#ifdef __SWITCH__
    // The Rust-driven link does not include GCC's crtbegin frame registration.
    // devkitA64's static unwinder searches explicitly registered objects only.
    static bool registered = false;
    if (!registered) { __register_frame(__eh_frame_start); registered = true; }
    void *bases[3] = {};
    if (!_Unwind_Find_FDE(reinterpret_cast<const void *>(nostalre_server_call), bases)) {
        std::fprintf(stderr, "[Server] exception frame registration failed\n");
        return 1;
    }
#endif
    char name[] = "exception-boundary-test";
    char *args[] = {name, nullptr};
    int status = nostalre_server_call(boundary_probe, 1, args);
    if (status != 37) return 1;
    std::fprintf(stderr, "[Server] exception boundary self-test passed\n");
    return 0;
}
