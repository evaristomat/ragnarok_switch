#include <switch.h>
#include <string.h>
#include <stdio.h>
#include <sys/iosupport.h>

/* libnx's console device, independent of stdout's redirected file handle. */
extern const devoptab_t *__nx_get_console_dotab(void);

static void screen_text(const char *message) {
    const devoptab_t *device = __nx_get_console_dotab();
    if (device && device->write_r) device->write_r(_REENT, NULL, message, strlen(message));
}

static PadState pad;
static bool loading_console;

bool nostalre_full_memory(void) {
    return appletGetAppletType() == AppletType_Application;
}

void nostalre_loading_begin(void) {
    PrintConsole *console = consoleInit(NULL);
    loading_console = console && console->consoleInitialised;
    if (!loading_console) { fprintf(stderr, "[Startup] console renderer failed to initialize\n"); return; }
    screen_text("\x1b[2J\x1b[2;2HnostalRE - Nintendo Switch\n\nPreparando o jogo offline...\nA primeira execucao pode demorar alguns minutos.\n");
    consoleUpdate(NULL);
}

void nostalre_loading_status(const char *status) {
    if (!loading_console) return;
    screen_text("\x1b[7;1H\x1b[J");
    screen_text(status);
    consoleUpdate(NULL);
}

bool nostalre_loading_poll(void) {
    bool running = appletMainLoop();
    if (loading_console) consoleUpdate(NULL);
    return running;
}

void nostalre_loading_end(void) {
    if (loading_console) { consoleExit(NULL); loading_console = false; }
}

void nostalre_hid_init(void) {
    padConfigureInput(1, HidNpadStyleSet_NpadStandard);
    padInitializeDefault(&pad);
    hidInitializeTouchScreen();
    nwindowSetDimensions(nwindowGetDefault(), 1280, 720);
}

void nostalre_hid_poll(u64 *buttons, s32 *sticks) {
    padUpdate(&pad);
    *buttons = padGetButtons(&pad);
    HidAnalogStickState left = padGetStickPos(&pad, 0);
    HidAnalogStickState right = padGetStickPos(&pad, 1);
    sticks[0] = left.x; sticks[1] = left.y;
    sticks[2] = right.x; sticks[3] = right.y;
}

u32 nostalre_touch_poll(u32 *points) {
    HidTouchScreenState state = {0};
    if (hidGetTouchScreenStates(&state, 1) != 1) return 0;
    u32 count = state.count > 16 ? 16 : state.count;
    for (u32 i = 0; i < count; ++i) {
        points[i * 3] = state.touches[i].finger_id;
        points[i * 3 + 1] = state.touches[i].x;
        points[i * 3 + 2] = state.touches[i].y;
    }
    return count;
}

u32 nostalre_keyboard(char *text, size_t capacity) {
    SwkbdConfig keyboard;
    Result rc = swkbdCreate(&keyboard, 0);
    if (R_FAILED(rc)) {
        fprintf(stderr, "[Keyboard] swkbdCreate failed: 0x%08x\n", rc);
        return rc;
    }
    swkbdConfigMakePresetDefault(&keyboard);
    swkbdConfigSetGuideText(&keyboard, "nostalRE: digite o texto do campo selecionado");
    swkbdConfigSetStringLenMax(&keyboard, capacity - 1);
    rc = swkbdShow(&keyboard, text, capacity);
    fprintf(stderr, "[Keyboard] swkbdShow result: 0x%08x\n", rc);
    swkbdClose(&keyboard);
    return rc;
}

void nostalre_show_message(const char *message) {
    nostalre_loading_end();
    consoleInit(NULL);
    screen_text("\x1b[2J\x1b[2;2HnostalRE - Nintendo Switch\n\n");
    screen_text(message);
    screen_text("\n\nPressione + para sair.\n");
    while (appletMainLoop()) {
        padUpdate(&pad);
        if (padGetButtonsDown(&pad) & HidNpadButton_Plus) break;
        consoleUpdate(NULL);
        svcSleepThread(16000000);
    }
    consoleExit(NULL);
}
