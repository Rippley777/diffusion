/* Small Xdnd source used only by the Linux integration smoke test. */
#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static Window find_window(Display *d, Window parent) {
    char *name = NULL;
    if (XFetchName(d, parent, &name) && name) {
        int match = strcmp(name, "Diffusion") == 0;
        XFree(name);
        if (match) return parent;
    }
    Window root, par, *children = NULL;
    unsigned int count = 0;
    if (!XQueryTree(d, parent, &root, &par, &children, &count)) return 0;
    Window found = 0;
    for (unsigned int i = 0; i < count && !found; i++) found = find_window(d, children[i]);
    if (children) XFree(children);
    return found;
}
static void send_message(Display *d, Window target, Atom type, long a, long b, long c, long e, long f) {
    XEvent event = {0};
    event.xclient.type = ClientMessage;
    event.xclient.display = d;
    event.xclient.window = target;
    event.xclient.message_type = type;
    event.xclient.format = 32;
    event.xclient.data.l[0] = a; event.xclient.data.l[1] = b;
    event.xclient.data.l[2] = c; event.xclient.data.l[3] = e; event.xclient.data.l[4] = f;
    XSendEvent(d, target, False, NoEventMask, &event); XFlush(d);
}
int main(int argc, char **argv) {
    if (argc < 2) return 2;
    Display *d = XOpenDisplay(NULL);
    if (!d) return 3;
    Window target = 0;
    for (int i = 0; i < 100 && !target; i++) { target = find_window(d, DefaultRootWindow(d)); usleep(50000); }
    if (!target) return 4;
    Window source = XCreateSimpleWindow(d, DefaultRootWindow(d), 0, 0, 1, 1, 0, 0, 0);
    Atom selection = XInternAtom(d, "XdndSelection", False), uri = XInternAtom(d, "text/uri-list", False);
    Atom copy = XInternAtom(d, "XdndActionCopy", False), status = XInternAtom(d, "XdndStatus", False);
    Atom finished = XInternAtom(d, "XdndFinished", False);
    char payload[16384] = {0};
    for (int i = 1; i < argc; i++) {
        char *path = realpath(argv[i], NULL);
        if (!path || strlen(payload) + strlen(path) + 10 >= sizeof(payload)) return 5;
        strcat(payload, "file://"); strcat(payload, path); strcat(payload, "\r\n"); free(path);
    }
    XWarpPointer(d, None, DefaultRootWindow(d), 0, 0, 0, 0, 100, 100);
    XSetSelectionOwner(d, selection, source, CurrentTime);
    send_message(d, target, XInternAtom(d, "XdndEnter", False), source, 5L << 24, uri, 0, 0);
    send_message(d, target, XInternAtom(d, "XdndPosition", False), source, 0, (100L << 16) | 100, CurrentTime, copy);
    int dropped = 0;
    for (int i = 0; i < 500; i++) {
        while (XPending(d)) {
            XEvent event; XNextEvent(d, &event);
            if (event.type == ClientMessage && event.xclient.message_type == status && !dropped) {
                if (!(event.xclient.data.l[1] & 1)) return 6;
                send_message(d, target, XInternAtom(d, "XdndDrop", False), source, 0, CurrentTime, 0, 0);
                dropped = 1;
            } else if (event.type == SelectionRequest) {
                XSelectionRequestEvent *r = &event.xselectionrequest;
                Atom property = r->property == None ? r->target : r->property;
                XChangeProperty(d, r->requestor, property, uri, 8, PropModeReplace, (unsigned char *)payload, strlen(payload));
                XEvent reply = {0}; reply.xselection.type = SelectionNotify;
                reply.xselection.display = d; reply.xselection.requestor = r->requestor;
                reply.xselection.selection = r->selection; reply.xselection.target = r->target;
                reply.xselection.property = property; reply.xselection.time = r->time;
                XSendEvent(d, r->requestor, False, 0, &reply); XFlush(d);
            } else if (event.type == ClientMessage && event.xclient.message_type == finished) {
                XDestroyWindow(d, source); XCloseDisplay(d); puts("Native X11 file drop completed"); return 0;
            }
        }
        usleep(10000);
    }
    return 7;
}
