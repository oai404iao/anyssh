import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  type CSSProperties,
  type MutableRefObject,
} from "react";
import { FitAddon } from "@xterm/addon-fit";
import { LigaturesAddon } from "@xterm/addon-ligatures";
import { UnicodeGraphemesAddon } from "@xterm/addon-unicode-graphemes";
import { Terminal, type ITheme } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import type { AmbiguousWidth, TerminalPalette } from "../lib/appearance-bridge";

export interface TerminalHandle {
  focus(): void;
  reset(): void;
  write(data: string | Uint8Array, callback?: () => void): void;
}

interface TerminalPaneProps {
  appearance: TerminalAppearance;
  onInput(input: string): void;
  onResize(columns: number, rows: number): void;
  visible?: boolean;
}

export interface TerminalAppearance {
  fontFamily: string;
  fontLoadRevision: number;
  fontSize: number;
  lineHeight: number;
  ligaturesEnabled: boolean;
  ambiguousWidth: AmbiguousWidth;
  palette: TerminalPalette;
}

export const TerminalPane = forwardRef<TerminalHandle, TerminalPaneProps>(
  function TerminalPane(
    { appearance, onInput, onResize, visible = true },
    forwardedRef,
  ) {
    const mountRef = useRef<HTMLDivElement>(null);
    const terminalRef = useRef<Terminal | null>(null);
    const fitRef = useRef<(() => void) | null>(null);
    const unicodeAddonRef = useRef<UnicodeGraphemesAddon | null>(null);
    const ligaturesAddonRef = useRef<LigaturesAddon | null>(null);
    const initialAppearanceRef = useRef(appearance);
    const inputHandlerRef = useLatest(onInput);
    const resizeHandlerRef = useLatest(onResize);
    const visibleRef = useLatest(visible);

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
      const initialAppearance = initialAppearanceRef.current;

      const terminal = new Terminal({
        // Required by the experimental Unicode grapheme provider. Keep this
        // scoped to the terminal adapter so it can be removed or replaced.
        allowProposedApi: true,
        allowTransparency: true,
        convertEol: false,
        cursorBlink: true,
        cursorStyle: "bar",
        fontFamily: initialAppearance.fontFamily,
        fontSize: initialAppearance.fontSize,
        lineHeight: initialAppearance.lineHeight,
        minimumContrastRatio: 4.5,
        scrollback: 10_000,
        theme: terminalTheme(initialAppearance.palette),
      });
      const fitAddon = new FitAddon();
      const unicodeGraphemesAddon = new UnicodeGraphemesAddon();

      terminal.loadAddon(unicodeGraphemesAddon);
      unicodeAddonRef.current = unicodeGraphemesAddon;
      terminal.loadAddon(fitAddon);
      terminal.open(mount);
      setAmbiguousWidth(
        unicodeGraphemesAddon,
        initialAppearance.ambiguousWidth,
      );
      if (initialAppearance.ligaturesEnabled) {
        const ligaturesAddon = new LigaturesAddon();
        terminal.loadAddon(ligaturesAddon);
        ligaturesAddonRef.current = ligaturesAddon;
      }
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
        if (!visibleRef.current) return;
        try {
          fitAddon.fit();
        } catch {
          // The WebView can briefly report a zero-sized mount during layout changes.
        }
      };
      fitRef.current = fit;

      const observer = new ResizeObserver(fit);
      observer.observe(mount);
      queueMicrotask(fit);

      return () => {
        observer.disconnect();
        inputDisposable.dispose();
        resizeDisposable.dispose();
        terminal.dispose();
        terminalRef.current = null;
        fitRef.current = null;
        unicodeAddonRef.current = null;
        ligaturesAddonRef.current = null;
      };
    }, [inputHandlerRef, resizeHandlerRef, visibleRef]);

    useEffect(() => {
      if (!visible) return;
      queueMicrotask(() => fitRef.current?.());
    }, [visible]);

    useEffect(() => {
      const terminal = terminalRef.current;
      if (!terminal) return;

      terminal.options.fontFamily = appearance.fontFamily;
      terminal.options.fontSize = appearance.fontSize;
      terminal.options.lineHeight = appearance.lineHeight;
      terminal.options.theme = terminalTheme(appearance.palette);
      const unicodeAddon = unicodeAddonRef.current;
      if (unicodeAddon) {
        setAmbiguousWidth(unicodeAddon, appearance.ambiguousWidth);
      }

      if (appearance.ligaturesEnabled && !ligaturesAddonRef.current) {
        const ligaturesAddon = new LigaturesAddon();
        terminal.loadAddon(ligaturesAddon);
        ligaturesAddonRef.current = ligaturesAddon;
      } else if (!appearance.ligaturesEnabled && ligaturesAddonRef.current) {
        ligaturesAddonRef.current.dispose();
        ligaturesAddonRef.current = null;
      }

      terminal.refresh(0, terminal.rows - 1);
      if (visible) queueMicrotask(() => fitRef.current?.());
    }, [appearance, visible]);

    return (
      <div
        className="terminal-surface"
        style={
          {
            "--terminal-background": appearance.palette.background,
          } as CSSProperties
        }
      >
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

function terminalTheme(palette: TerminalPalette): ITheme {
  return { ...palette };
}

function setAmbiguousWidth(
  addon: UnicodeGraphemesAddon,
  width: AmbiguousWidth,
): void {
  // The pinned experimental addon registers both providers but does not expose
  // its `ambiguousCharsAreWide` option. Keep this implementation detail scoped
  // to the Terminal Adapter so an upstream public API can replace it later.
  const providers = addon as UnicodeGraphemesAddon & {
    _provider15?: { ambiguousCharsAreWide: boolean };
    _provider15Graphemes?: { ambiguousCharsAreWide: boolean };
  };
  const wide = width === "wide";
  if (providers._provider15) {
    providers._provider15.ambiguousCharsAreWide = wide;
  }
  if (providers._provider15Graphemes) {
    providers._provider15Graphemes.ambiguousCharsAreWide = wide;
  }
}
