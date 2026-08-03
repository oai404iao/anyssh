import {
  useCallback,
  useEffect,
  useRef,
  type FormEvent,
  type MutableRefObject,
} from "react";
import type { TerminalHandle } from "../../components/TerminalPane";
import type { HostSummary } from "../../lib/host-bridge";
import {
  confirmHostKey,
  connectSavedHost,
  connectSsh,
  disconnectSsh,
  respondAuthentication,
  resizeSsh,
  sendSshInput,
  startSshPortForward,
  stopSshPortForward,
  type SshClientEvent,
} from "../../lib/ssh-bridge";
import type { SessionTab } from "./session-model";

type UpdateSessionTab = (
  tabId: string,
  update: (current: SessionTab) => SessionTab,
  expectedGeneration?: number,
) => void;

interface SessionRuntimeOptions {
  activateSessionTab(tabId: string): void;
  activeTabIdRef: MutableRefObject<string>;
  hosts: HostSummary[];
  refreshRepository(): Promise<void>;
  sessionTabsRef: MutableRefObject<SessionTab[]>;
  terminalRefs: MutableRefObject<Map<string, TerminalHandle>>;
  updateSessionTab: UpdateSessionTab;
}

export function useSessionRuntime({
  activateSessionTab,
  activeTabIdRef,
  hosts,
  refreshRepository,
  sessionTabsRef,
  terminalRefs,
  updateSessionTab,
}: SessionRuntimeOptions) {
  const appMountedRef = useRef(false);

  useEffect(() => {
    const tabsRef = sessionTabsRef;
    appMountedRef.current = true;
    return () => {
      appMountedRef.current = false;
      for (const tab of tabsRef.current) {
        if (tab.sessionId) {
          void disconnectSsh(tab.sessionId).catch(() => undefined);
        }
      }
    };
  }, [sessionTabsRef]);

  const setActiveError = useCallback(
    (error: string | null) => {
      updateSessionTab(activeTabIdRef.current, (tab) => ({
        ...tab,
        error,
      }));
    },
    [activeTabIdRef, updateSessionTab],
  );

  const writeSystemLine = useCallback(
    (tabId: string, message: string) => {
      terminalRefs.current
        .get(tabId)
        ?.write(`\r\n\x1b[38;5;110m${message}\x1b[0m\r\n`);
    },
    [terminalRefs],
  );

  const handleClientEvent = useCallback(
    (tabId: string, generation: number, event: SshClientEvent) => {
      if (!appMountedRef.current) return;
      const currentTab = sessionTabsRef.current.find((tab) => tab.id === tabId);
      if (!currentTab || currentTab.generation !== generation) return;

      let shouldActivate = false;
      let systemLine: string | null = null;

      switch (event.type) {
        case "connecting":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "connecting",
              statusDetail: "Negotiating SSH transport…",
            }),
            generation,
          );
          break;
        case "hostKey":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "verifying",
              statusDetail:
                event.hop.kind === "target"
                  ? "Target host confirmation is required."
                  : `Jump host ${event.hop.index} confirmation is required.`,
              pendingHostKey: event,
            }),
            generation,
          );
          shouldActivate = true;
          break;
        case "hostKeyChanged": {
          const hop =
            event.hop.kind === "target"
              ? "Target host"
              : `Jump host ${event.hop.index}`;
          const message = `${hop} key changed for ${event.host}:${event.port}. Connection blocked.`;
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "error",
              statusDetail: message,
              error: message,
              pendingHostKey: null,
              pendingAuthentication: null,
              changedHostKey: event,
              portForwards: [],
              portForwardError: null,
              portForwardBusy: false,
              form: { ...tab.form, password: "" },
              passwordVisible: false,
            }),
            generation,
          );
          shouldActivate = true;
          systemLine = message;
          break;
        }
        case "authenticationChallenge": {
          const hop =
            event.hop.kind === "target"
              ? "Target host"
              : `Jump host ${event.hop.index}`;
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "authenticating",
              statusDetail: `${hop} requires additional authentication.`,
              pendingAuthentication: event,
            }),
            generation,
          );
          shouldActivate = true;
          break;
        }
        case "authenticated":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "authenticated",
              statusDetail: "Opening an interactive PTY…",
              pendingAuthentication: null,
            }),
            generation,
          );
          break;
        case "connected":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "connected",
              statusDetail: "Interactive shell is active.",
              error: null,
            }),
            generation,
          );
          if (activeTabIdRef.current === tabId) {
            terminalRefs.current.get(tabId)?.focus();
          }
          break;
        case "exitStatus":
          systemLine = `Remote process exited with status ${event.code}.`;
          break;
        case "error":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "error",
              statusDetail: event.message,
              error: event.message,
              pendingAuthentication: null,
              portForwards: [],
              portForwardError: null,
              portForwardBusy: false,
              form: { ...tab.form, password: "" },
              passwordVisible: false,
            }),
            generation,
          );
          systemLine = `Connection error: ${event.message}`;
          break;
        case "closed":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: tab.status === "error" ? tab.status : "closed",
              statusDetail: "The SSH session has ended.",
              sessionId: null,
              pendingHostKey: null,
              pendingAuthentication: null,
              portForwards: [],
              portForwardError: null,
              portForwardBusy: false,
              form: { ...tab.form, password: "" },
              passwordVisible: false,
            }),
            generation,
          );
          break;
      }

      if (systemLine) writeSystemLine(tabId, systemLine);
      if (shouldActivate) {
        const currentActiveTab = sessionTabsRef.current.find(
          (tab) => tab.id === activeTabIdRef.current,
        );
        const anotherTabOwnsVisibleAction =
          currentActiveTab !== undefined &&
          currentActiveTab.id !== tabId &&
          (currentActiveTab.pendingHostKey !== null ||
            currentActiveTab.pendingAuthentication !== null ||
            currentActiveTab.changedHostKey !== null);
        if (!anotherTabOwnsVisibleAction) activateSessionTab(tabId);
      }
    },
    [
      activateSessionTab,
      activeTabIdRef,
      sessionTabsRef,
      terminalRefs,
      updateSessionTab,
      writeSystemLine,
    ],
  );

  const startTabConnection = useCallback(
    async (tab: SessionTab) => {
      const selectedHost =
        hosts.find((host) => host.id === tab.selectedSavedHostId) ?? null;
      const port = Number(tab.form.port);
      if (
        !selectedHost &&
        (!tab.form.host.trim() ||
          !tab.form.username.trim() ||
          !Number.isInteger(port))
      ) {
        setActiveError("Host, port, and username are required.");
        return;
      }

      const tabId = tab.id;
      const generation = tab.generation + 1;
      const terminalSize = tab.terminalSize;
      const connectionForm = { ...tab.form };
      updateSessionTab(tabId, (current) => ({
        ...current,
        generation,
        status: "connecting",
        statusDetail: "Preparing connection…",
        sessionId: null,
        pendingHostKey: null,
        changedHostKey: null,
        pendingAuthentication: null,
        portForwards: [],
        portForwardError: null,
        portForwardBusy: false,
        passwordVisible: false,
        error: null,
        form: { ...current.form, password: "" },
      }));
      const terminal = terminalRefs.current.get(tabId);
      terminal?.reset();
      terminal?.write(
        `\x1b[1;36mAnySSH Phase 1\x1b[0m\r\nStarting ${
          selectedHost ? "saved Host" : "a secure"
        } SSH session…\r\n`,
      );

      try {
        const callbacks = {
          onEvent: (clientEvent: SshClientEvent) =>
            handleClientEvent(tabId, generation, clientEvent),
          onData: (data: Uint8Array) =>
            new Promise<void>((resolve) => {
              const targetTerminal = terminalRefs.current.get(tabId);
              if (targetTerminal) {
                targetTerminal.write(data, resolve);
              } else {
                resolve();
              }
            }),
        };
        const id = selectedHost
          ? await connectSavedHost(
              {
                hostId: selectedHost.id,
                columns: terminalSize.columns,
                rows: terminalSize.rows,
              },
              callbacks,
            )
          : await connectSsh(
              {
                host: connectionForm.host.trim(),
                port,
                authentication:
                  connectionForm.authenticationKind === "keyboardInteractive"
                    ? {
                        kind: "keyboardInteractive",
                        username: connectionForm.username.trim(),
                      }
                    : {
                        kind: "temporaryPassword",
                        username: connectionForm.username.trim(),
                        password: connectionForm.password,
                      },
                columns: terminalSize.columns,
                rows: terminalSize.rows,
              },
              callbacks,
            );

        const current = sessionTabsRef.current.find(
          (candidate) => candidate.id === tabId,
        );
        if (
          !appMountedRef.current ||
          !current ||
          current.generation !== generation ||
          current.status === "closed" ||
          current.status === "error"
        ) {
          await disconnectSsh(id).catch(() => undefined);
          return;
        }
        updateSessionTab(
          tabId,
          (currentTab) => ({
            ...currentTab,
            sessionId: id,
            form: { ...currentTab.form, password: "" },
            passwordVisible: false,
          }),
          generation,
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        handleClientEvent(tabId, generation, { type: "error", message });
      }
    },
    [
      handleClientEvent,
      hosts,
      sessionTabsRef,
      setActiveError,
      terminalRefs,
      updateSessionTab,
    ],
  );

  const handleConnect = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      const tab = sessionTabsRef.current.find(
        (candidate) => candidate.id === activeTabIdRef.current,
      );
      if (tab) await startTabConnection(tab);
    },
    [activeTabIdRef, sessionTabsRef, startTabConnection],
  );

  const handleHostKeyDecision = useCallback(
    async (accepted: boolean) => {
      const tab = sessionTabsRef.current.find(
        (candidate) => candidate.id === activeTabIdRef.current,
      );
      if (!tab?.sessionId || !tab.pendingHostKey) return;

      const { id: tabId, generation, sessionId } = tab;
      const requestId = tab.pendingHostKey.requestId;
      updateSessionTab(
        tabId,
        (current) => ({ ...current, pendingHostKey: null }),
        generation,
      );
      try {
        await confirmHostKey(sessionId, requestId, accepted);
        if (accepted) await refreshRepository();
        if (!accepted) {
          updateSessionTab(
            tabId,
            (current) => ({
              ...current,
              status: "closed",
              statusDetail: "Host key was rejected.",
            }),
            generation,
          );
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        handleClientEvent(tabId, generation, { type: "error", message });
      }
    },
    [
      activeTabIdRef,
      handleClientEvent,
      refreshRepository,
      sessionTabsRef,
      updateSessionTab,
    ],
  );

  const handleAuthenticationDecision = useCallback(
    async (responses: string[] | null) => {
      const tab = sessionTabsRef.current.find(
        (candidate) => candidate.id === activeTabIdRef.current,
      );
      if (!tab?.sessionId || !tab.pendingAuthentication) return;

      const { id: tabId, generation, sessionId } = tab;
      const requestId = tab.pendingAuthentication.requestId;
      updateSessionTab(
        tabId,
        (current) => ({ ...current, pendingAuthentication: null }),
        generation,
      );
      try {
        await respondAuthentication(sessionId, requestId, responses);
        if (responses === null) {
          updateSessionTab(
            tabId,
            (current) => ({
              ...current,
              status: "closed",
              statusDetail: "Additional authentication was cancelled.",
            }),
            generation,
          );
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        handleClientEvent(tabId, generation, { type: "error", message });
      }
    },
    [activeTabIdRef, handleClientEvent, sessionTabsRef, updateSessionTab],
  );

  const handleDisconnect = useCallback(async () => {
    const tab = sessionTabsRef.current.find(
      (candidate) => candidate.id === activeTabIdRef.current,
    );
    if (!tab?.sessionId) return;
    updateSessionTab(tab.id, (current) => ({
      ...current,
      pendingAuthentication: null,
      form: { ...current.form, password: "" },
      passwordVisible: false,
    }));
    await disconnectSsh(tab.sessionId);
  }, [activeTabIdRef, sessionTabsRef, updateSessionTab]);

  const handleTerminalInput = useCallback(
    (tabId: string, input: string) => {
      const tab = sessionTabsRef.current.find(
        (candidate) => candidate.id === tabId,
      );
      if (!tab?.sessionId || tab.status !== "connected") return;
      void sendSshInput(tab.sessionId, input);
    },
    [sessionTabsRef],
  );

  const handleTerminalResize = useCallback(
    (tabId: string, columns: number, rows: number) => {
      if (columns < 2 || rows < 1) return;
      const tab = sessionTabsRef.current.find(
        (candidate) => candidate.id === tabId,
      );
      if (!tab) return;
      updateSessionTab(tabId, (current) => ({
        ...current,
        terminalSize: { columns, rows },
      }));
      if (tab.sessionId && tab.status === "connected") {
        void resizeSsh(tab.sessionId, columns, rows);
      }
    },
    [sessionTabsRef, updateSessionTab],
  );

  const handleStartPortForward = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      const tabId = activeTabIdRef.current;
      const tab = sessionTabsRef.current.find((item) => item.id === tabId);
      if (!tab?.sessionId || tab.status !== "connected") {
        updateSessionTab(tabId, (current) => ({
          ...current,
          portForwardError: "Connect this session before starting a forward.",
        }));
        return;
      }

      const form = tab.portForwardForm;
      const bindPort = Number(form.bindPort);
      const destinationPort = Number(form.destinationPort);
      if (
        !Number.isInteger(bindPort) ||
        bindPort < 0 ||
        bindPort > 65_535 ||
        (form.kind !== "dynamic" &&
          (!form.destinationHost.trim() ||
            !Number.isInteger(destinationPort) ||
            destinationPort < 1 ||
            destinationPort > 65_535))
      ) {
        updateSessionTab(tabId, (current) => ({
          ...current,
          portForwardError: "Enter valid bind and destination ports.",
        }));
        return;
      }

      const sessionId = tab.sessionId;
      const generation = tab.generation;
      updateSessionTab(tabId, (current) => ({
        ...current,
        portForwardError: null,
        portForwardBusy: true,
      }));
      try {
        const summary = await startSshPortForward(sessionId, {
          kind: form.kind,
          bindHost: form.bindHost,
          bindPort,
          ...(form.kind === "dynamic"
            ? {}
            : {
                destinationHost: form.destinationHost.trim(),
                destinationPort,
              }),
        });
        updateSessionTab(tabId, (current) =>
          current.sessionId === sessionId && current.generation === generation
            ? {
                ...current,
                portForwards: [...current.portForwards, summary],
                portForwardBusy: false,
              }
            : current,
        );
      } catch (error) {
        updateSessionTab(tabId, (current) =>
          current.sessionId === sessionId && current.generation === generation
            ? {
                ...current,
                portForwardError:
                  error instanceof Error ? error.message : String(error),
                portForwardBusy: false,
              }
            : current,
        );
      }
    },
    [activeTabIdRef, sessionTabsRef, updateSessionTab],
  );

  const handleStopPortForward = useCallback(
    async (forwardId: string) => {
      const tabId = activeTabIdRef.current;
      const tab = sessionTabsRef.current.find((item) => item.id === tabId);
      if (!tab?.sessionId) return;
      const sessionId = tab.sessionId;
      try {
        await stopSshPortForward(sessionId, forwardId);
        updateSessionTab(tabId, (current) =>
          current.sessionId === sessionId
            ? {
                ...current,
                portForwards: current.portForwards.filter(
                  (forward) => forward.id !== forwardId,
                ),
                portForwardError: null,
              }
            : current,
        );
      } catch (error) {
        updateSessionTab(tabId, (current) => ({
          ...current,
          portForwardError:
            error instanceof Error ? error.message : String(error),
        }));
      }
    },
    [activeTabIdRef, sessionTabsRef, updateSessionTab],
  );

  return {
    handleAuthenticationDecision,
    handleConnect,
    handleDisconnect,
    handleHostKeyDecision,
    handleStartPortForward,
    handleStopPortForward,
    handleTerminalInput,
    handleTerminalResize,
    startTabConnection,
  };
}
