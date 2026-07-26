import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  type MutableRefObject,
} from "react";
import { FitAddon } from "@xterm/addon-fit";
import { UnicodeGraphemesAddon } from "@xterm/addon-unicode-graphemes";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

export interface TerminalHandle {
  focus(): void;
  reset(): void;
  write(data: string | Uint8Array, callback?: () => void): void;
}

interface TerminalPaneProps {
  onInput(input: string): void;
  onResize(columns: number, rows: number): void;
}

export const TerminalPane = forwardRef<TerminalHandle, TerminalPaneProps>(
  function TerminalPane({ onInput, onResize }, forwardedRef) {
    const mountRef = useRef<HTMLDivElement>(null);
    const terminalRef = useRef<Terminal | null>(null);
    const inputHandlerRef = useLatest(onInput);
    const resizeHandlerRef = useLatest(onResize);

    useImperativeHandle(
      forwardedRef,
      () => ({
        focus: () => terminalRef.current?.focus(),
        reset: () => terminalRef.current?.reset(),
        write: (data, callback) => terminalRef.current?.write(data, callback),
      }),
      [],
    );

    useEffect(() => {
      const mount = mountRef.current;
      if (!mount) return;

      const terminal = new Terminal({
        // Required by the experimental Unicode grapheme provider. Keep this
        // scoped to the terminal adapter so it can be removed or replaced.
        allowProposedApi: true,
        allowTransparency: true,
        convertEol: false,
        cursorBlink: true,
        cursorStyle: "bar",
        fontFamily:
          '"AnySSH Nerd Mono", "Noto Emoji Variable", "Noto Sans Mono CJK SC", "SFMono-Regular", Consolas, monospace',
        fontSize: 13,
        lineHeight: 1.42,
        minimumContrastRatio: 4.5,
        scrollback: 10_000,
        theme: {
          background: "#090d16",
          foreground: "#c8d0df",
          cursor: "#6be6d2",
          cursorAccent: "#090d16",
          selectionBackground: "#294a50",
          black: "#11151f",
          red: "#ff7888",
          green: "#6be6d2",
          yellow: "#ffc66d",
          blue: "#7aa2f7",
          magenta: "#b29cff",
          cyan: "#6be6d2",
          white: "#c8d0df",
          brightBlack: "#667188",
          brightRed: "#ff9aa6",
          brightGreen: "#93f2e2",
          brightYellow: "#ffdb9e",
          brightBlue: "#a5c2ff",
          brightMagenta: "#c9bdff",
          brightCyan: "#9af4e5",
          brightWhite: "#f1f5ff",
        },
      });
      const fitAddon = new FitAddon();
      const unicodeGraphemesAddon = new UnicodeGraphemesAddon();

      terminal.loadAddon(unicodeGraphemesAddon);
      terminal.loadAddon(fitAddon);
      terminal.open(mount);
      terminalRef.current = terminal;
      terminal.write(
        "\x1b[1;36mAnySSH\x1b[0m\r\nSelect a host and open a secure session.\r\n",
      );

      const inputDisposable = terminal.onData((data) =>
        inputHandlerRef.current(data),
      );
      const resizeDisposable = terminal.onResize(({ cols, rows }) =>
        resizeHandlerRef.current(cols, rows),
      );

      const fit = () => {
        try {
          fitAddon.fit();
        } catch {
          // The WebView can briefly report a zero-sized mount during layout changes.
        }
      };

      const observer = new ResizeObserver(fit);
      observer.observe(mount);
      queueMicrotask(fit);

      return () => {
        observer.disconnect();
        inputDisposable.dispose();
        resizeDisposable.dispose();
        terminal.dispose();
        terminalRef.current = null;
      };
    }, [inputHandlerRef, resizeHandlerRef]);

    return (
      <div className="terminal-surface">
        <div
          aria-label="Interactive SSH terminal"
          className="terminal-mount"
          ref={mountRef}
          role="application"
        />
      </div>
    );
  },
);

function useLatest<T>(value: T): MutableRefObject<T> {
  const ref = useRef(value);
  useEffect(() => {
    ref.current = value;
  }, [value]);
  return ref;
}
