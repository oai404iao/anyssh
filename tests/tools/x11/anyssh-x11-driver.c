#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/XTest.h>
#include <X11/keysym.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void put_u16(FILE *file, uint16_t value) {
  fputc(value & 0xff, file);
  fputc((value >> 8) & 0xff, file);
}

static void put_u32(FILE *file, uint32_t value) {
  fputc(value & 0xff, file);
  fputc((value >> 8) & 0xff, file);
  fputc((value >> 16) & 0xff, file);
  fputc((value >> 24) & 0xff, file);
}

static unsigned char color_channel(unsigned long pixel, unsigned long mask) {
  unsigned shift = 0;
  unsigned long maximum;
  unsigned long value;

  if (!mask) {
    return 0;
  }

  while (((mask >> shift) & 1UL) == 0UL) {
    shift++;
  }

  maximum = mask >> shift;
  value = (pixel & mask) >> shift;
  return (unsigned char)((value * 255UL + maximum / 2UL) / maximum);
}

static int screenshot(Display *display, Window root, const char *path) {
  XWindowAttributes attributes;
  XImage *image;
  FILE *file;
  uint32_t row_size;
  uint32_t pixel_size;
  unsigned char padding[3] = {0, 0, 0};

  if (!XGetWindowAttributes(display, root, &attributes)) {
    return 1;
  }

  image = XGetImage(display, root, 0, 0, attributes.width, attributes.height,
                    AllPlanes, ZPixmap);
  if (!image) {
    return 1;
  }

  file = fopen(path, "wb");
  if (!file) {
    XDestroyImage(image);
    return 1;
  }

  row_size = (uint32_t)((attributes.width * 3 + 3) & ~3);
  pixel_size = row_size * (uint32_t)attributes.height;

  fwrite("BM", 1, 2, file);
  put_u32(file, 54 + pixel_size);
  put_u16(file, 0);
  put_u16(file, 0);
  put_u32(file, 54);
  put_u32(file, 40);
  put_u32(file, (uint32_t)attributes.width);
  put_u32(file, (uint32_t)attributes.height);
  put_u16(file, 1);
  put_u16(file, 24);
  put_u32(file, 0);
  put_u32(file, pixel_size);
  put_u32(file, 2835);
  put_u32(file, 2835);
  put_u32(file, 0);
  put_u32(file, 0);

  for (int y = attributes.height - 1; y >= 0; y--) {
    for (int x = 0; x < attributes.width; x++) {
      unsigned long pixel = XGetPixel(image, x, y);
      unsigned char bgr[3] = {
          color_channel(pixel, image->blue_mask),
          color_channel(pixel, image->green_mask),
          color_channel(pixel, image->red_mask),
      };
      fwrite(bgr, 1, 3, file);
    }
    fwrite(padding, 1, row_size - (uint32_t)(attributes.width * 3), file);
  }

  fclose(file);
  XDestroyImage(image);
  return 0;
}

static int window_has_rendered_content(Display *display, Window window) {
  XWindowAttributes attributes;
  XImage *image;
  unsigned long pixel;
  unsigned int brightness;

  if (!XGetWindowAttributes(display, window, &attributes) ||
      attributes.width < 1 || attributes.height < 1) {
    return 0;
  }

  image = XGetImage(display, window, attributes.width / 2,
                    attributes.height / 2, 1, 1, AllPlanes, ZPixmap);
  if (!image) {
    return 0;
  }

  pixel = XGetPixel(image, 0, 0);
  brightness = color_channel(pixel, image->red_mask) +
               color_channel(pixel, image->green_mask) +
               color_channel(pixel, image->blue_mask);
  XDestroyImage(image);
  return brightness > 3 && brightness < 720;
}

static int list_windows(Display *display, Window parent, int depth,
                        const char *window_match, Window *matched_window) {
  Window root_return;
  Window parent_return;
  Window *children = NULL;
  unsigned int child_count = 0;
  int found = 0;

  if (!XQueryTree(display, parent, &root_return, &parent_return, &children,
                  &child_count)) {
    return 0;
  }

  for (unsigned int index = 0; index < child_count; index++) {
    XWindowAttributes attributes;
    char *name = NULL;

    if (XGetWindowAttributes(display, children[index], &attributes) &&
        attributes.map_state == IsViewable) {
      XFetchName(display, children[index], &name);
      printf("window=0x%lx depth=%d geometry=%dx%d+%d+%d name=%s\n",
             children[index], depth, attributes.width, attributes.height,
             attributes.x, attributes.y, name ? name : "");
      if (strcmp(window_match, "*") == 0 ||
          (name && strstr(name, window_match))) {
        found = 1;
        if (*matched_window == None) {
          *matched_window = children[index];
        }
      }
      if (name) {
        XFree(name);
      }
    }

    if (list_windows(display, children[index], depth + 1, window_match,
                     matched_window)) {
      found = 1;
    }
  }

  if (children) {
    XFree(children);
  }
  return found;
}

static void send_key(Display *display, KeySym symbol, int shift) {
  KeyCode code = XKeysymToKeycode(display, symbol);
  KeyCode shift_code = XKeysymToKeycode(display, XK_Shift_L);

  if (!code) {
    fprintf(stderr, "No keycode for keysym %lu\n", symbol);
    exit(3);
  }

  if (shift) {
    XTestFakeKeyEvent(display, shift_code, True, CurrentTime);
  }
  XTestFakeKeyEvent(display, code, True, CurrentTime);
  XTestFakeKeyEvent(display, code, False, CurrentTime);
  if (shift) {
    XTestFakeKeyEvent(display, shift_code, False, CurrentTime);
  }
  XFlush(display);
  usleep(35000);
}

static void send_ctrl_key(Display *display, KeySym symbol) {
  KeyCode code = XKeysymToKeycode(display, symbol);
  KeyCode control_code = XKeysymToKeycode(display, XK_Control_L);

  if (!code || !control_code) {
    fprintf(stderr, "No keycode for control shortcut\n");
    exit(3);
  }

  XTestFakeKeyEvent(display, control_code, True, CurrentTime);
  XTestFakeKeyEvent(display, code, True, CurrentTime);
  XTestFakeKeyEvent(display, code, False, CurrentTime);
  XTestFakeKeyEvent(display, control_code, False, CurrentTime);
  XFlush(display);
  usleep(35000);
}

static void type_character(Display *display, char character) {
  if (character >= 'a' && character <= 'z') {
    send_key(display, XK_a + (character - 'a'), 0);
    return;
  }
  if (character >= 'A' && character <= 'Z') {
    send_key(display, XK_a + (character - 'A'), 1);
    return;
  }
  if (character >= '0' && character <= '9') {
    send_key(display, XK_0 + (character - '0'), 0);
    return;
  }

  switch (character) {
    case ' ':
      send_key(display, XK_space, 0);
      break;
    case '-':
      send_key(display, XK_minus, 0);
      break;
    case '_':
      send_key(display, XK_minus, 1);
      break;
    case '.':
      send_key(display, XK_period, 0);
      break;
    case '/':
      send_key(display, XK_slash, 0);
      break;
    case ':':
      send_key(display, XK_semicolon, 1);
      break;
    case ';':
      send_key(display, XK_semicolon, 0);
      break;
    case '\\':
      send_key(display, XK_backslash, 0);
      break;
    case '|':
      send_key(display, XK_backslash, 1);
      break;
    case '\'':
      send_key(display, XK_apostrophe, 0);
      break;
    default:
      fprintf(stderr, "Unsupported character: 0x%02x\n",
              (unsigned char)character);
      exit(4);
  }
}

static int usage(const char *program) {
  fprintf(stderr,
          "usage: %s probe [SCREENSHOT.bmp] | click X Y | type TEXT | "
          "enter | ctrl-a | ctrl-l | backspace | space | tab\n",
          program);
  return 1;
}

int main(int argc, char **argv) {
  Display *display = XOpenDisplay(NULL);
  Window root;
  int result = 0;

  if (!display) {
    fprintf(stderr, "Cannot open DISPLAY\n");
    return 2;
  }

  root = DefaultRootWindow(display);

  if ((argc == 2 || argc == 3) && strcmp(argv[1], "probe") == 0) {
    const char *window_match = getenv("ANYSSH_X11_WINDOW_MATCH");
    Window matched_window = None;
    int found = list_windows(display, root, 0,
                             window_match ? window_match : "AnySSH",
                             &matched_window);
    if (found && !window_match &&
        !window_has_rendered_content(display, matched_window)) {
      found = 0;
    }
    if (argc == 3 && screenshot(display, root, argv[2]) != 0) {
      result = 5;
    } else if (!found) {
      result = 6;
    }
  } else if (argc == 4 && strcmp(argv[1], "click") == 0) {
    XTestFakeMotionEvent(display, -1, atoi(argv[2]), atoi(argv[3]),
                         CurrentTime);
    XTestFakeButtonEvent(display, 1, True, CurrentTime);
    XTestFakeButtonEvent(display, 1, False, CurrentTime);
    XFlush(display);
  } else if (argc == 3 && strcmp(argv[1], "type") == 0) {
    for (size_t index = 0; argv[2][index] != '\0'; index++) {
      type_character(display, argv[2][index]);
    }
  } else if (argc == 2 && strcmp(argv[1], "enter") == 0) {
    send_key(display, XK_Return, 0);
  } else if (argc == 2 && strcmp(argv[1], "ctrl-a") == 0) {
    send_ctrl_key(display, XK_a);
  } else if (argc == 2 && strcmp(argv[1], "ctrl-l") == 0) {
    send_ctrl_key(display, XK_l);
  } else if (argc == 2 && strcmp(argv[1], "backspace") == 0) {
    send_key(display, XK_BackSpace, 0);
  } else if (argc == 2 && strcmp(argv[1], "space") == 0) {
    send_key(display, XK_space, 0);
  } else if (argc == 2 && strcmp(argv[1], "tab") == 0) {
    send_key(display, XK_Tab, 0);
  } else {
    result = usage(argv[0]);
  }

  XCloseDisplay(display);
  return result;
}
