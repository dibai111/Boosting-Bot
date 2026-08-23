<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    Activity, BadgeCheck, BedDouble, Bot, Box, Check, ChevronDown, Cloud, Copy, Cpu, Download,
    ExternalLink, EyeOff, Info, LayoutGrid, LogIn, MessageSquare, Minus,
    Moon, Play, Radio, ShieldCheck, SlidersHorizontal, Swords, Square,
    Sun, Trash2, Upload, Users, X,
  } from "@lucide/svelte";
  import { api } from "@botting/user-api";
  import ShaderBackground from "../../shared-ui/ShaderBackground.svelte";
  import DiaTextReveal from "../../shared-ui/DiaTextReveal.svelte";
  import AnimatedThemeToggler from "../../shared-ui/AnimatedThemeToggler.svelte";
  import { ripple } from "../../shared-ui/ripple";
  import { inlineNoticeSurfaceEasing, modalSurfaceEasing, surfaceMotion, toastSurfaceEasing } from "../../shared-ui/motion";
  import { translate, type Locale, type MessageKey } from "./lib/i18n";
  import {
    gameKindForMode, loadFavoriteMatchModes, modeGroups,
    modeOptions, modeOptionsForGame,
  } from "./lib/features/matchmaking";
  import { createUserAppController, type UserAppState } from "./lib/features/controller";
  import {
    buildSessionBotOptions,
    formatSessionLog,
    mergeLifecycleLogEntries,
    type SessionBotOption,
  } from "./lib/log/session-log";
  import AccountCard from "./lib/components/account/AccountCard.svelte";
  import BotCard from "./lib/components/bot/BotCard.svelte";
  import MatchmakingOverlay from "./lib/components/matchmaking/MatchmakingOverlay.svelte";
  import MinecraftHead from "./lib/components/minecraft/MinecraftHead.svelte";
  import SelectMenu from "./lib/components/controls/SelectMenu.svelte";
  import type { Account, AuthKind, BotEvent, BotMatchPhase, BotPhase, GameKind, GameMode, MatchmakingPhase, MatchmakingSnapshot, SessionLogEntry } from "./lib/models/types";
  import { closeWindow, minimizeWindow, onFileDrop, setUserWindowMode, type FileDropEvent } from "./lib/adapters/window";

  type Page = "overview" | "bots" | "matchmaking" | "accounts" | "sessions" | "settings";
  type Theme = "light" | "dark";
  type SidebarMenu = "bots" | "sessions";
  type SessionLogFilter = "all" | "errors" | "matchmaking";
  type ShortcutAction = "stop" | "show_overlay";

  export let settings: Record<string, string> = {};
  let settingsSave = Promise.resolve();

  const isMatchmakingOverlay = new URLSearchParams(window.location.search).get("overlay") === "matchmaking";
  if (isMatchmakingOverlay) document.documentElement.dataset.userWindow = "overlay";
  const savedMatchMode = settings["botting-match-mode"] as GameMode | null;
  const initialMatchMode = modeOptions.some((option) => option.value === savedMatchMode)
    ? savedMatchMode!
    : "doubles";
  const idleMatchmaking: MatchmakingSnapshot = { phase: "idle", required_matches: 0, matched_bots: 0, bots: [] };
  const defaultStopShortcut = "F8";
  const defaultShowOverlayShortcut = "F9";
  const appEntryDurationMs = 1500;
  const appEntryCoverDurationMs = 180;
  const appWindowExpandDurationMs = 900;
  let locale: Locale = (settings["botting-locale"] as Locale) || "en";
  let theme: Theme = (settings["botting-theme"] as Theme) || "light";
  let page: Page = "overview";
  let loading = true;
  let appState: UserAppState;
  let accounts: Account[] = [];
  let accountEntryIds = new Set<string>();
  let selectedBotIds = new Set<string>();
  let selectedAccountIds = new Set<string>();
  let phases: Record<string, BotPhase> = {};
  // 停止命令送出後，忽略同一 bot 尚未排出的舊狀態事件。
  let stoppingBotIds = new Set<string>();
  let sessionLogs: SessionLogEntry[] = [];
  let nextSessionLogId = 1;
  let sessionBotLabels: Record<string, string> = {};
  let sessionBotOptions: SessionBotOption[] = [];
  let visibleSessionLogs: SessionLogEntry[] = [];
  let selectedSessionBotId: string | null = null;
  let onlineAccounts: Account[] = [];
  let openSidebarMenu: SidebarMenu | null = null;
  let sidebarMenuOpenTimer: number | null = null;
  let sidebarMenuCloseTimer: number | null = null;
  let sessionConsole: HTMLDivElement | null = null;
  let followSessionTail = true;
  let showChatLogs = settings["botting-show-chat-debug"] === "true";
  let sessionLogFilter: SessionLogFilter = "all";
  let sessionUnreadCount = 0;
  let addDialog: AuthKind | null = null;
  let formServer = "";
  let formCredential = "";
  let formError = "";
  let formCredentialError = "";
  let formServerError = "";
  let cookieDragging = false;
  let submitting = false;
  let deviceCode: { code: string; url: string } | null = null;
  let deviceLinkError = "";
  let copied = false;
  let exportLogPath = settings["botting-export-log-path"] || "";
  let appMemoryBytes: number | null = null;
  let stopShortcut = settings["botting-stop-shortcut"]?.trim() || defaultStopShortcut;
  let showOverlayShortcut = settings["botting-show-overlay-shortcut"]?.trim() || defaultShowOverlayShortcut;
  let recordingShortcut: ShortcutAction | null = null;
  let deleteDialogOpen = false;
  let deletingAccounts = false;
  let toast: { message: string; kind: "success" | "error" } | null = null;
  let toastTimer: number | null = null;
  let validityNow = Date.now();
  let currentPageTitle: MessageKey = "overview";
  let currentPageSubtitle: MessageKey = "overviewSub";
  let appEntryPhase: UserAppState["appEntryPhase"] = "idle";
  let matchmaking = idleMatchmaking;
  let matchMode: GameMode = initialMatchMode;
  let matchGame: GameKind = gameKindForMode(matchMode);
  let favoriteMatchModes = loadFavoriteMatchModes(settings);
  let matchLogPath = settings["botting-player-log-path"] || "";
  let verifyPresence = settings["botting-verify-presence"] !== "false";
  let verifyDuelPitch = settings["botting-verify-duel-pitch"] !== "false";
  let requiredMatches = 1;
  let matchmakingBusy = false;
  let matchActive = false;
  let overlayVisible = false;
  let overlayAnimationKey = 0;
  let logPathDialogOpen = false;
  let userModalOpen = false;

  function markAccountEntry(id: string): void {
    accountEntryIds = new Set(accountEntryIds).add(id);
  }

  function finishAccountEntry(id: string): void {
    if (!accountEntryIds.has(id)) return;
    const next = new Set(accountEntryIds);
    next.delete(id);
    accountEntryIds = next;
  }

  let t = (key: MessageKey) => translate(locale, key);
  $: t = (key: MessageKey) => translate(locale, key);

  // 直接讀取 phases，確保 Rust bot runtime 的狀態事件會觸發統計更新。
  $: onlineCount = accounts.filter((account) => phases[account.id] === "online").length;
  $: waitingCount = accounts.filter((account) => !phases[account.id] || phases[account.id] === "offline").length;
  $: currentPageTitle = page === "overview" ? "overview" : page;
  $: currentPageSubtitle = ({
    overview: "overviewSub", bots: "botsSub", accounts: "accountsSub",
    matchmaking: "matchmakingHelp", sessions: "sessionsSub", settings: "settingsSub",
  } as Record<Page, MessageKey>)[page];
  $: onlineAccounts = accounts.filter((account) => phases[account.id] === "online");
  $: sessionBotOptions = buildSessionBotOptions(accounts, sessionLogs, sessionBotLabels, t("bot"))
    .filter((option) => phases[option.id] === "online");
  $: if (selectedSessionBotId && phases[selectedSessionBotId] !== "online") selectedSessionBotId = null;
  $: selectedOnlineBotCount = accounts.filter((account) =>
    selectedBotIds.has(account.id) && phases[account.id] === "online",
  ).length;
  $: selectedBotCount = accounts.filter((account) => selectedBotIds.has(account.id)).length;
  $: selectedStartableBotCount = accounts.filter((account) => {
    if (!selectedBotIds.has(account.id)) return false;
    const phase = phaseFor(account.id);
    return phase === "offline" || phase === "error";
  }).length;
  $: selectedStoppableBotCount = accounts.filter((account) => {
    if (!selectedBotIds.has(account.id)) return false;
    const phase = phaseFor(account.id);
    return phase === "online" || phase === "starting" || phase === "authenticating"
      || phase === "connecting";
  }).length;
  $: phaseLabels = Object.fromEntries(
    accounts.map((account) => [account.id, phases[account.id] ?? "offline"]),
  ) as Record<string, BotPhase>;
  $: visibleSessionLogs = sessionLogs.filter((entry) => {
    if (!showChatLogs && /^\[(?:chat|match debug)]/.test(entry.message)) return false;
    const matchesBot = !selectedSessionBotId || entry.bot_id === selectedSessionBotId;
    if (!matchesBot) return false;
    if (sessionLogFilter === "errors") return entry.level === "error";
    if (sessionLogFilter === "matchmaking") return /match|queue|server|lobby|attempt|retry/i.test(entry.message);
    return true;
  });
  $: matchActive = matchmaking.phase !== "idle" && matchmaking.phase !== "failed";
  $: if (matchmaking.phase !== "idle" && matchmaking.mode) {
    matchMode = matchmaking.mode;
    matchGame = gameKindForMode(matchmaking.mode);
  }
  $: if (requiredMatches > Math.max(1, selectedOnlineBotCount)) requiredMatches = Math.max(1, selectedOnlineBotCount);
  $: userModalOpen = !isMatchmakingOverlay && (
    logPathDialogOpen
    || addDialog !== null
    || deleteDialogOpen
    || deviceCode !== null
  );

  function applyUserAppState(next: UserAppState): void {
    appState = next;
  }

  // Controller 是 operational state 的唯一 source；表單輸入和純視覺 state 留在此頁。
  $: if (appState) {
    appEntryPhase = appState.appEntryPhase;
    appMemoryBytes = appState.appMemoryBytes;
    accounts = appState.accounts;
    selectedBotIds = appState.selectedBotIds;
    phases = appState.phases;
    stoppingBotIds = appState.stoppingBotIds;
    matchmaking = appState.matchmaking;
    matchmakingBusy = appState.matchmakingBusy;
    overlayVisible = appState.overlayVisible;
    deviceCode = appState.deviceCode;
    deviceLinkError = appState.deviceLinkError;
    matchMode = appState.matchMode;
    matchGame = appState.matchGame;
    favoriteMatchModes = appState.favoriteMatchModes;
    verifyPresence = appState.verifyPresence;
    verifyDuelPitch = appState.verifyDuelPitch;
    logPathDialogOpen = appState.logPathDialogOpen;
  }

  const userApp = createUserAppController({
    api,
    appWindowExpandDurationMs,
    initialMatchMode,
    initialMatchGame: gameKindForMode(initialMatchMode),
    initialFavoriteMatchModes: favoriteMatchModes,
    initialVerifyPresence: verifyPresence,
    initialVerifyDuelPitch: verifyDuelPitch,
    initialMatchLogPath: matchLogPath,
    savePreference,
    setWindowMode: (mode) => setUserWindowMode(mode),
    tick,
    phaseLabel,
    addLog,
    showToast,
    translate: (key) => t(key as MessageKey),
    onChange: applyUserAppState,
  });

  onMount(() => {
    applyTheme(theme);
    let unlisten: Array<() => void> = [];
    if (isMatchmakingOverlay) {
      Promise.all([
        api.listAccounts(),
        api.matchmakingSnapshot(),
        api.onMatchmakingState(handleMatchmakingState),
        api.onMatchmakingOverlayVisibility((visible) => {
          if (visible) overlayAnimationKey += 1;
        }),
      ]).then(([savedAccounts, savedMatchmaking, stopListening, stopVisibilityListening]) => {
        userApp.setAccounts(savedAccounts);
        userApp.setMatchmakingSnapshot(savedMatchmaking);
        unlisten = [stopListening, stopVisibilityListening];
        loading = false;
      }).catch(() => { loading = false; });
      return () => unlisten.forEach((stop) => stop());
    }

    const validityTimer = window.setInterval(() => { validityNow = Date.now(); }, 30_000);
    const memoryTimer = window.setInterval(() => { void refreshMemory(); }, 5_000);
    window.addEventListener("keydown", recordShortcut);
    void api.configureMatchmakingShortcuts(stopShortcut, showOverlayShortcut).catch((error) => {
      showToast(shortcutErrorLabel(error), "error");
    });
    void refreshMemory();
    Promise.all([
      api.listAccounts(),
      api.defaultExportLogPath(),
      api.matchmakingSnapshot(),
      onFileDrop(handleNativeFileDrop),
      api.onBotEvent(handleBotEvent),
      api.onMatchmakingState(handleMatchmakingState),
      api.onMatchmakingDebug((diagnostic) => addLog(`[match debug] ${diagnostic.message}`, diagnostic.bot_id ?? null)),
      api.onMatchmakingShortcut(handleMatchmakingShortcut),
      api.onMatchmakingOverlayVisibility((visible) => { userApp.setOverlayVisible(visible); }),
    ])
      .then(async ([savedAccounts, defaultExportPath, savedMatchmaking, ...listeners]) => {
        userApp.setAccounts(savedAccounts);
        userApp.setMatchmakingSnapshot(savedMatchmaking);
        if (!exportLogPath) exportLogPath = defaultExportPath;
        unlisten = listeners;
        await userApp.showApp();
        loading = false;
      })
      .catch((error) => {
        showToast(String(error), "error");
        void userApp.showApp();
        loading = false;
      });
    return () => {
      window.clearInterval(validityTimer);
      window.clearInterval(memoryTimer);
      window.removeEventListener("keydown", recordShortcut);
      if (sidebarMenuOpenTimer !== null) window.clearTimeout(sidebarMenuOpenTimer);
      if (sidebarMenuCloseTimer !== null) window.clearTimeout(sidebarMenuCloseTimer);
      unlisten.forEach((stop) => stop());
    };
  });

  function phaseFor(id: string): BotPhase {
    return phases[id] ?? "offline";
  }

  function phaseLabel(phase: BotPhase): string {
    if (phase === "online") return t("online");
    if (phase === "starting" || phase === "authenticating") return t("starting");
    if (phase === "connecting") return t("connecting");
    if (phase === "stopping") return t("stopping");
    if (phase === "error") return t("error");
    return t("offline");
  }

  function matchmakingPhaseLabel(phase: MatchmakingPhase): string {
    const keys: Record<MatchmakingPhase, MessageKey> = {
      idle: "matchIdle",
      awaiting_player: "matchAwaiting",
      matching: "matchMatching",
      committed: "matchCommitted",
      in_game: "matchInGame",
      failed: "matchFailed",
    };
    return t(keys[phase]);
  }

  function matchmakingStatusIconKey(phase: MatchmakingPhase): "activity" | "ready" | "failed" {
    if (phase === "in_game") return "ready";
    if (phase === "failed") return "failed";
    return "activity";
  }

  function matchmakingModeLabel(mode: MatchmakingSnapshot["mode"]): string {
    return modeOptions.find((option) => option.value === mode)?.label ?? t("gameMode");
  }

  function toggleFavoriteMatchMode(value: string) {
    userApp.toggleFavorite(value);
  }

  function togglePresenceVerification() {
    userApp.togglePresenceVerification();
  }

  function toggleDuelPitchVerification() {
    userApp.toggleDuelPitchVerification();
  }

  function selectMatchGame(game: GameKind) {
    userApp.selectMatchGame(game);
  }

  function selectMatchMode(value: string) {
    userApp.selectMatchMode(value);
  }

  function handleMatchmakingState(next: MatchmakingSnapshot) {
    userApp.handleMatchmakingState(next);
  }

  function matchBotClass(phase: BotMatchPhase): string {
    if (phase === "matched" || phase === "afk") return "matched";
    if (phase === "unavailable") return "failed";
    if (phase === "queued" || phase === "returning") return "working";
    return "waiting";
  }

  function matchBotLabel(botId: string): string {
    const account = accounts.find((item) => item.id === botId);
    return account?.username ?? t("bot");
  }

  function sessionActor(entry: SessionLogEntry): string {
    return accounts.find((account) => account.id === entry.bot_id)?.username ?? entry.bot_label ?? t("bot");
  }

  function overviewStatus(phase: BotPhase): string {
    return phase === "online" ? t("lobby") : phaseLabel(phase);
  }

  function statusClass(phase: BotPhase): string {
    return phase === "online" ? "blue" : phase === "error" || phase === "connecting" ? "orange" : "gray";
  }

  function credentialValid(account: Account, phase: BotPhase): boolean {
    if (phase === "error") return false;
    if (account.auth_kind === "microsoft") return true;
    const expiresAt = account.session_expires_at
      ? new Date(account.session_expires_at).getTime()
      : 0;
    return expiresAt > validityNow;
  }

  function credentialStatusClass(account: Account, phase: BotPhase): string {
    return credentialValid(account, phase) ? "green" : "red";
  }

  function memoryLabel(): string {
    return appMemoryBytes === null ? "-" : `${(appMemoryBytes / 1_048_576).toFixed(1)} MB`;
  }

  async function refreshMemory() {
    await userApp.refreshMemory();
  }

  function setLocale(next: Locale) {
    locale = next;
    savePreference("botting-locale", next);
  }

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    applyTheme(theme);
  }

  function applyTheme(next: Theme) {
    savePreference("botting-theme", next);
    document.documentElement.dataset.theme = next;
  }

  function savePreference(key: string, value: string): void {
    settings = { ...settings, [key]: value };
    const nextSettings = settings;
    settingsSave = settingsSave
      .catch(() => undefined)
      .then(() => api.saveSettings(nextSettings));
  }

  function toggleSelection(selection: Set<string>, id: string): Set<string> {
    const next = new Set(selection);
    next.has(id) ? next.delete(id) : next.add(id);
    return next;
  }

  function addLog(
    message: string,
    botId: string | null = null,
    level: SessionLogEntry["level"] = "info",
  ) {
    const timestamp = new Date().toLocaleTimeString("en-GB", { hour12: false });
    let botLabel: string | null = null;
    if (botId) {
      const account = accounts.find((item) => item.id === botId);
      botLabel = account?.username ?? sessionBotLabels[botId] ?? t("bot");
      if (!sessionBotLabels[botId]) sessionBotLabels = { ...sessionBotLabels, [botId]: botLabel };
    }
    const nextEntry: SessionLogEntry = {
      id: nextSessionLogId++,
      timestamp,
      bot_id: botId,
      bot_label: botLabel,
      level,
      message,
    };
    const mergedEntry = mergeLifecycleLogEntries(sessionLogs.at(-1), nextEntry);
    sessionLogs = mergedEntry
      ? [...sessionLogs.slice(0, -1), mergedEntry]
      : [...sessionLogs.slice(-999), nextEntry];
    const matchesCurrentView = (!selectedSessionBotId || selectedSessionBotId === botId)
      && (sessionLogFilter === "all"
        || (sessionLogFilter === "errors" && level === "error")
        || (sessionLogFilter === "matchmaking" && /match|queue|server|lobby|attempt|retry/i.test(message)));
    if (!followSessionTail && matchesCurrentView) sessionUnreadCount += 1;
    if (followSessionTail && matchesCurrentView) {
      void scrollSessionToLatest();
    }
  }

  async function startSelected() {
    await userApp.startSelected();
  }

  async function stopAccounts(targets: Account[]) {
    if (!targets.length) return;

    // 先更新本地狀態，讓列表、Overview 統計及配對按鈕立即反映停止結果。
    await userApp.stopAccounts(targets);
  }

  async function stopSelected() {
    await userApp.stopSelected();
  }

  async function stopSession() {
    await userApp.stopSession();
  }

  async function startMatchmaking() {
    await userApp.startMatchmaking({
      selectedBotIds,
      matchMode,
      matchGame,
      favoriteMatchModes,
      verifyPresence,
      verifyDuelPitch,
      requiredMatches,
      matchLogPath,
    });
  }

  async function stopMatchmaking() {
    await userApp.stopMatchmaking();
  }

  async function toggleMatchmaking() {
    await userApp.toggleMatchmaking({
      selectedBotIds,
      matchMode,
      matchGame,
      favoriteMatchModes,
      verifyPresence,
      verifyDuelPitch,
      requiredMatches,
      matchLogPath,
    });
  }

  function continueWithLogPath() {
    userApp.continueWithLogPath(matchLogPath);
  }

  async function toggleMatchmakingOverlay() {
    await userApp.toggleMatchmakingOverlay();
  }

  async function hideMatchmakingOverlay() {
    await userApp.hideMatchmakingOverlay();
  }

  function handleMatchmakingShortcut(action: ShortcutAction) {
    userApp.handleMatchmakingShortcut(action);
  }

  async function saveServerFor(account: Account, serverAddress: string) {
    await userApp.saveServerFor(account, serverAddress);
  }

  function resetAddDialogForm() {
    formServer = "";
    formCredential = "";
    formError = "";
    formCredentialError = "";
    formServerError = "";
    cookieDragging = false;
  }

  function openAddDialog(kind: AuthKind) {
    resetAddDialogForm();
    addDialog = kind;
  }

  function closeAddDialog() {
    resetAddDialogForm();
    addDialog = null;
  }

  async function submitAccount() {
    const needsCredential = addDialog === "access_token" || addDialog === "cookie";
    formCredentialError = needsCredential && !formCredential.trim() ? t("credentialRequired") : "";
    formServerError = !formServer.trim() ? t("serverRequired") : "";
    formError = "";
    if (formCredentialError || formServerError) return;
    submitting = true;
    try {
      const account = await api.addAccount({
        username: addDialog === "microsoft" ? "Microsoft account" : addDialog === "cookie" ? "Cookie account" : "Token account",
        auth_kind: addDialog!,
        credential: formCredential.trim() || null,
        server_address: formServer.trim(),
      });
      markAccountEntry(account.id);
      userApp.setAccounts([...accounts, account]);
      userApp.setPhases({ ...phases, [account.id]: "offline" });
      closeAddDialog();
      page = "accounts";
    } catch (error) {
      formError = credentialErrorLabel(error);
    } finally {
      submitting = false;
    }
  }

  async function importCookieDrop(event: DragEvent) {
    event.preventDefault();
    cookieDragging = false;
    const file = event.dataTransfer?.files.item(0);
    if (file) {
      formCredential = await file.text();
      formCredentialError = "";
    }
  }

  async function handleNativeFileDrop(event: FileDropEvent) {
    if (event.type === "leave") {
      cookieDragging = false;
      return;
    }
    if (page === "matchmaking" && addDialog === null && event.type === "drop") {
      const path = event.paths[0];
      if (path?.toLowerCase().endsWith(".log")) {
        userApp.setMatchLogPath(path);
        savePreference("botting-player-log-path", path);
      }
      return;
    }
    if (addDialog !== "cookie") return;

    const zone = document.querySelector<HTMLElement>(".cookie-drop-zone");
    if (!zone) return;
    const bounds = zone.getBoundingClientRect();
    const scale = window.devicePixelRatio || 1;
    const overZone = event.position.x >= bounds.left * scale
      && event.position.x <= bounds.right * scale
      && event.position.y >= bounds.top * scale
      && event.position.y <= bounds.bottom * scale;

    if (event.type !== "drop") {
      cookieDragging = overZone;
      return;
    }

    cookieDragging = false;
    const path = overZone ? event.paths[0] : undefined;
    if (!path) return;
    try {
      formCredential = await api.readCookieFile(path);
      formCredentialError = "";
    } catch (error) {
      formCredentialError = String(error);
    }
  }

  function startCookieDrag(event: DragEvent) {
    event.preventDefault();
    cookieDragging = true;
  }

  function openDeleteDialog() {
    if (selectedAccountIds.size) deleteDialogOpen = true;
  }

  async function deleteSelected() {
    if (!selectedAccountIds.size) return;
    deletingAccounts = true;
    const ids = [...selectedAccountIds];
    try {
      await userApp.deleteAccounts(ids);
      selectedAccountIds = new Set();
      deleteDialogOpen = false;
    } finally {
      deletingAccounts = false;
    }
  }

  function handleBotEvent(event: BotEvent) {
    userApp.handleBotEvent(event);
  }

  function showSidebarMenuSoon(menu: SidebarMenu) {
    if (sidebarMenuCloseTimer !== null) window.clearTimeout(sidebarMenuCloseTimer);
    if (openSidebarMenu === menu) return;
    if (sidebarMenuOpenTimer !== null) window.clearTimeout(sidebarMenuOpenTimer);
    sidebarMenuOpenTimer = window.setTimeout(() => {
      openSidebarMenu = menu;
      sidebarMenuOpenTimer = null;
    }, 180);
  }

  function showSidebarMenu(menu: SidebarMenu) {
    if (sidebarMenuOpenTimer !== null) window.clearTimeout(sidebarMenuOpenTimer);
    if (sidebarMenuCloseTimer !== null) window.clearTimeout(sidebarMenuCloseTimer);
    openSidebarMenu = menu;
  }

  function hideSidebarMenuSoon() {
    if (sidebarMenuOpenTimer !== null) window.clearTimeout(sidebarMenuOpenTimer);
    sidebarMenuCloseTimer = window.setTimeout(() => {
      openSidebarMenu = null;
      sidebarMenuCloseTimer = null;
    }, 200);
  }

  function handleSidebarNavFocusOut(event: FocusEvent) {
    const group = event.currentTarget as HTMLElement | null;
    if (!group?.contains(event.relatedTarget as Node | null)) hideSidebarMenuSoon();
  }

  function openBots(keepMenuOpen = false) {
    page = "bots";
    openSidebarMenu = keepMenuOpen ? "bots" : null;
  }

  function openSession(botId: string | null, keepMenuOpen = false) {
    selectedSessionBotId = botId;
    page = "sessions";
    openSidebarMenu = keepMenuOpen ? "sessions" : null;
    followSessionTail = true;
    sessionUnreadCount = 0;
    void scrollSessionToLatest(true);
  }

  function sessionStatusClass(phase: BotPhase): string {
    if (phase === "online") return "green";
    if (phase === "error") return "red";
    if (phase === "starting" || phase === "authenticating" || phase === "connecting" || phase === "stopping") return "orange";
    return "gray";
  }

  function handleSessionScroll() {
    if (!sessionConsole) return;
    followSessionTail = sessionConsole.scrollHeight - sessionConsole.scrollTop - sessionConsole.clientHeight < 32;
    if (followSessionTail) sessionUnreadCount = 0;
  }

  function setSessionLogFilter(filter: SessionLogFilter) {
    sessionLogFilter = filter;
    sessionUnreadCount = 0;
    followSessionTail = true;
    void scrollSessionToLatest(true);
  }

  function toggleSessionFollow() {
    followSessionTail = !followSessionTail;
    if (followSessionTail) {
      sessionUnreadCount = 0;
      void scrollSessionToLatest(true);
    }
  }

  function toggleChatLogs() {
    showChatLogs = !showChatLogs;
    savePreference("botting-show-chat-debug", String(showChatLogs));
    if (showChatLogs && followSessionTail) void scrollSessionToLatest();
  }

  async function scrollSessionToLatest(force = false) {
    await tick();
    if (!sessionConsole || (!force && !followSessionTail)) return;
    sessionConsole.scrollTo({ top: sessionConsole.scrollHeight, behavior: force ? "smooth" : "auto" });
  }

  function showToast(message: string, kind: "success" | "error") {
    if (toastTimer !== null) window.clearTimeout(toastTimer);
    toast = { message, kind };
    toastTimer = window.setTimeout(() => {
      toast = null;
      toastTimer = null;
    }, 3200);
  }

  async function exportLog() {
    try {
      const savedPath = await api.exportLog(visibleSessionLogs.map(formatSessionLog).join("\n"), exportLogPath.trim());
      showToast(`${t("logExported")} ${savedPath}`, "success");
    } catch (error) {
      showToast(`${t("logExportFailed")}: ${String(error)}`, "error");
    }
  }

  function saveExportLogPath() {
    savePreference("botting-export-log-path", exportLogPath);
  }

  function beginShortcutRecording(action: ShortcutAction) {
    recordingShortcut = recordingShortcut === action ? null : action;
  }

  async function recordShortcut(event: KeyboardEvent) {
    if (!recordingShortcut) return;
    if (event.repeat) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      recordingShortcut = null;
      return;
    }
    if (["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight"].includes(event.code)) return;

    const modifiers = shortcutModifiers(event);
    const canBeUsedAlone = /^Key[A-Z]$/.test(event.code) || /^F(?:[1-9]|1[0-9]|2[0-4])$/.test(event.code);
    if (!modifiers.length && !canBeUsedAlone) {
      showToast(t("shortcutModifierRequired"), "error");
      return;
    }

    const shortcut = [...modifiers, event.code].join("+");
    const nextStopShortcut = recordingShortcut === "stop" ? shortcut : stopShortcut;
    const nextShowOverlayShortcut = recordingShortcut === "show_overlay" ? shortcut : showOverlayShortcut;
    if (shortcutIdentity(nextStopShortcut) === shortcutIdentity(nextShowOverlayShortcut)) {
      showToast(t("shortcutConflict"), "error");
      return;
    }

    recordingShortcut = null;
    try {
      await api.configureMatchmakingShortcuts(nextStopShortcut, nextShowOverlayShortcut);
      stopShortcut = nextStopShortcut;
      showOverlayShortcut = nextShowOverlayShortcut;
      savePreference("botting-stop-shortcut", stopShortcut);
      savePreference("botting-show-overlay-shortcut", showOverlayShortcut);
    } catch (error) {
      showToast(shortcutErrorLabel(error), "error");
    }
  }

  function shortcutModifiers(event: KeyboardEvent): string[] {
    return [
      event.ctrlKey ? "control" : "",
      event.altKey ? "alt" : "",
      event.shiftKey ? "shift" : "",
      event.metaKey ? "super" : "",
    ].filter(Boolean);
  }

  function shortcutIdentity(shortcut: string): string {
    return shortcut.toLowerCase().replaceAll("ctrl", "control").replaceAll(" ", "");
  }

  function shortcutLabel(shortcut: string): string {
    return shortcut.split("+").map((part) => {
      const names: Record<string, string> = { control: "Ctrl", alt: "Alt", shift: "Shift", super: "Win" };
      if (names[part.toLowerCase()]) return names[part.toLowerCase()];
      if (part.startsWith("Key")) return part.slice(3);
      if (part.startsWith("Digit")) return part.slice(5);
      return part;
    }).join(" + ");
  }

  function shortcutErrorLabel(error: unknown): string {
    const code = String(error);
    if (code.includes("shortcut_conflict")) return t("shortcutConflict");
    if (code.includes("shortcut_modifier_required")) return t("shortcutModifierRequired");
    if (code.includes("shortcut_invalid")) return t("shortcutInvalid");
    return code;
  }

  function credentialErrorLabel(error: unknown): string {
    const code = String(error);
    if (code.includes("account_already_exists")) return t("accountAlreadyAdded");
    if (code.includes("access_token_invalid")) return t("accessTokenInvalid");
    if (code.includes("access_token_expired")) return t("accessTokenExpired");
    return code;
  }

  async function copyCode() {
    if (!deviceCode) return;
    await navigator.clipboard.writeText(deviceCode.code);
    copied = true;
    setTimeout(() => copied = false, 1200);
  }

  async function openDeviceLogin() {
    if (!deviceCode) return;
    try {
      await api.openExternal(deviceCode.url);
      userApp.setDeviceLinkError("");
    } catch (error) {
      userApp.setDeviceLinkError(String(error));
    }
  }
</script>

{#if isMatchmakingOverlay}
  {#if loading}
    <main class="center-stage overlay-loading"><div class="loader"></div><span>{t("loading")}</span></main>
  {:else}
    <MatchmakingOverlay
      snapshot={matchmaking}
      {accounts}
      phaseLabel={matchmakingPhaseLabel}
      modeLabel={matchmakingModeLabel}
      attemptsLabel={t("attempts")}
      matchedLabel={t("matched")}
      minimumLabel={t("minimumShort")}
      playerServerLabel={t("playerServer")}
      queueingLabel={t("queueing")}
      hideLabel={t("hideOverlay")}
      animationKey={overlayAnimationKey}
      onHide={() => void hideMatchmakingOverlay()}
    />
  {/if}
{:else if loading}
  <main class="center-stage"><div class="loader"></div><span>{t("loading")}</span></main>
{:else}
  <main
    class="app-shell"
    class:entry-covered={appEntryPhase === "resizing" || appEntryPhase === "preparing" || appEntryPhase === "expanding"}
    class:modal-open={userModalOpen}
  >
    <ShaderBackground className="app-shader" />
    <header class="desktop-titlebar" data-tauri-drag-region>
      <span data-tauri-drag-region><img class="titlebar-brand-icon" src="/bedwars-boosting-icon.png" alt="" /> Botting</span>
      <div class="window-controls">
        <button class="window-control" title={t("minimizeWindow")} aria-label={t("minimizeWindow")} on:pointerdown|stopPropagation on:click={minimizeWindow}><Minus size={16} /></button>
        <button class="window-control window-close" title={t("closeWindow")} aria-label={t("closeWindow")} on:pointerdown|stopPropagation on:click={closeWindow}><X size={16} /></button>
      </div>
    </header>
    <aside class="sidebar">
      <div class="brand"><span class="brand-mark"><img src="/bedwars-boosting-icon.png" alt="" /></span><strong>Botting</strong><span class="user-badge">{t("user")}</span></div>
      <nav aria-label="Primary">
        <button class:active={page === "overview"} on:click={() => page = "overview"}><LayoutGrid size={19} />{t("overview")}</button>
        <div
          class="sidebar-nav-group"
          class:expanded={openSidebarMenu === "bots"}
          role="group"
          on:pointerenter={() => showSidebarMenuSoon("bots")}
          on:pointerleave={hideSidebarMenuSoon}
          on:focusin={() => showSidebarMenu("bots")}
          on:focusout={handleSidebarNavFocusOut}
        >
          <button class="sidebar-nav-trigger" class:active={page === "bots" || page === "matchmaking"} aria-expanded={openSidebarMenu === "bots"} on:click={() => openBots(true)}>
            <Bot size={19} />
            <span>{t("bots")}</span>
            <ChevronDown class="sidebar-nav-chevron" size={15} />
          </button>
          <div class="sidebar-nav-list" class:open={openSidebarMenu === "bots"} aria-hidden={openSidebarMenu !== "bots"}>
            <div class="sidebar-nav-list-inner compact">
              <button class="sidebar-nav-option" class:selected={page === "matchmaking"} tabindex={openSidebarMenu === "bots" ? 0 : -1} on:click={() => { page = "matchmaking"; openSidebarMenu = null; }}>
                <span class="sidebar-nav-status all"><Radio size={12} /></span>
                <span><strong>{t("matchmaking")}</strong><small>{selectedBotIds.size} {t("bots").toLowerCase()}</small></span>
              </button>
            </div>
          </div>
        </div>
        <button class:active={page === "accounts"} on:click={() => page = "accounts"}><Users size={19} />{t("accounts")}</button>
        <div
          class="sidebar-nav-group"
          class:expanded={openSidebarMenu === "sessions"}
          role="group"
          on:pointerenter={() => showSidebarMenuSoon("sessions")}
          on:pointerleave={hideSidebarMenuSoon}
          on:focusin={() => showSidebarMenu("sessions")}
          on:focusout={handleSidebarNavFocusOut}
        >
          <button class="sidebar-nav-trigger" class:active={page === "sessions"} aria-expanded={openSidebarMenu === "sessions"} on:click={() => openSession(null, true)}>
            <Activity size={19} />
            <span>{t("sessions")}</span>
            <ChevronDown class="sidebar-nav-chevron" size={15} />
          </button>
          <div class="sidebar-nav-list" class:open={openSidebarMenu === "sessions"} aria-hidden={openSidebarMenu !== "sessions"}>
            <div class="sidebar-nav-list-inner">
              <button class="sidebar-nav-option" class:selected={selectedSessionBotId === null && page === "sessions"} aria-label={`${t("sessions")}: ${t("allBots")}`} tabindex={openSidebarMenu === "sessions" ? 0 : -1} on:click={() => openSession(null)}>
                <span class="sidebar-nav-status all"><Activity size={12} /></span>
                <span><strong>{t("allBots")}</strong><small>{onlineAccounts.length} {t("bots").toLowerCase()}</small></span>
              </button>
              {#each sessionBotOptions as option}
                <button class="sidebar-nav-option session-bot-option" class:selected={selectedSessionBotId === option.id && page === "sessions"} aria-label={`${t("sessions")}: ${option.username}`} tabindex={openSidebarMenu === "sessions" ? 0 : -1} on:click={() => openSession(option.id)}>
                  <span class="session-head-wrap"><MinecraftHead username={option.username} size={24} /><i class="session-head-status {sessionStatusClass(phaseLabels[option.id] ?? "offline")}"></i></span>
                  <span><strong>{option.username}</strong><small>{phaseLabel(phaseLabels[option.id] ?? "offline")}</small></span>
                </button>
              {/each}
            </div>
          </div>
        </div>
        <button class:active={page === "settings"} on:click={() => page = "settings"}><SlidersHorizontal size={19} />{t("settings")}</button>
      </nav>
      <div class="sidebar-bottom">
        <span>v0.1.0</span>
        <span>{t("dev")}</span>
      </div>
    </aside>

    <section class="workspace" class:session-workspace={page === "sessions"}>
      <header class="topbar">
        <div><h1>{t(currentPageTitle)}</h1><p>{t(currentPageSubtitle)}</p></div>
        <div class="topbar-actions">
          <AnimatedThemeToggler
            {theme}
            onToggle={toggleTheme}
            lightLabel={t("lightTheme")}
            darkLabel={t("darkTheme")}
          ><Moon slot="light-icon" size={17} strokeWidth={2.1} /><Sun slot="dark-icon" size={17} strokeWidth={2.1} /></AnimatedThemeToggler>
        </div>
      </header>

      <div class="page-content" class:session-page-content={page === "sessions"}>
        {#key page}
        <div
          in:surfaceMotion={{ duration: 180, offsetY: 6, startScale: 1, easing: inlineNoticeSurfaceEasing }}
          class="page-view"
          class:session-page-view={page === "sessions"}
        >
        {#if page === "overview"}
          <section class="stat-grid">
            <article class="stat-card stat-card-live"><div><span>{t("botsOnline")}</span><strong>{onlineCount}</strong><small><span>{waitingCount} {t("waitingToStart")}</span></small></div><span class="stat-card-icon" aria-hidden="true"><Radio size={19} /></span></article>
            <article class="stat-card stat-card-ready"><div><span>{t("accountsReady")}</span><strong>{accounts.length}</strong><small><span>{t("loginDataAvailable")}</span></small></div><span class="stat-card-icon" aria-hidden="true"><BadgeCheck size={20} /></span></article>
            <article class="stat-card stat-card-runtime"><div><span>{t("appMemory")}</span><strong>{memoryLabel()}</strong><small><span>{t("runtimeManaged")}</span></small></div><span class="stat-card-icon" aria-hidden="true"><Cpu size={19} /></span></article>
          </section>
          <section class="section-heading overview-heading">
            <div><h2>{t("recentBots")}</h2><p>{t("eachBotAddress")}</p></div>
            <button class="dark-button" use:ripple={{ rippleColor: "#ADD8E6" }} on:click={() => page = "bots"}><Bot size={16} aria-hidden="true" />{t("manageBots")}</button>
          </section>
          <div class="overview-table data-table">
            <div class="table-row table-head"><span>{t("bot")}</span><span>{t("account")}</span><span>{t("serverAddress")}</span><span>{t("status")}</span></div>
            {#each accounts.slice(0, 3) as account}
              <div class="table-row"><MinecraftHead username={account.username} size={28} /><span>{account.username}</span><code>{account.server_address}</code><span class="status-label"><i class={statusClass(phaseLabels[account.id] ?? "offline")}></i>{overviewStatus(phaseLabels[account.id] ?? "offline")}</span></div>
            {:else}
              <div
                transition:surfaceMotion={{ duration: 180, exitDuration: 120, offsetY: 4, startScale: .99, easing: inlineNoticeSurfaceEasing }}
                class="empty-row"
              >
                {t("noAccounts")}
              </div>
            {/each}
          </div>

        {:else if page === "bots"}
          <section class="bot-toolbar" aria-label={t("botActions")}>
            <span class="bot-selection-count" class:has-selection={selectedBotCount > 0} aria-live="polite">
              <span class="bot-selection-value">{selectedBotCount} {t("bots").toLowerCase()} {t("selected")}</span>
              <span class="bot-selection-divider" aria-hidden="true"></span>
              <span class="bot-selection-note">{t("botsCreatedFromAccounts")}</span>
            </span>
            <div class="bot-toolbar-actions">
              <button
                type="button"
                class="bot-action bot-stop"
                disabled={!selectedStoppableBotCount}
                title={t("stopSelected")}
                on:click={stopSelected}
              ><Square size={15} />{t("stop")}</button>
              <button
                type="button"
                class="bot-action bot-start ripple-button"
                use:ripple={{ rippleColor: "#ADD8E6" }}
                disabled={!selectedStartableBotCount}
                title={t("startSelected")}
                on:click={startSelected}
              ><Play size={15} />{t("start")}</button>
            </div>
          </section>
          <div class="bot-card-grid">
            {#each accounts as account}
              <BotCard
                username={account.username}
                profileId={account.profile_id ?? ""}
                serverAddress={account.server_address}
                serverAddressLabel={t("serverAddress")}
                status={overviewStatus(phaseLabels[account.id] ?? "offline")}
                statusTone={statusClass(phaseLabels[account.id] ?? "offline")}
                selected={selectedBotIds.has(account.id)}
                onSelect={() => { const next = toggleSelection(selectedBotIds, account.id); userApp.setSelectedBotIds(next); }}
                onServerChange={(serverAddress) => saveServerFor(account, serverAddress)}
              />
            {/each}
          </div>

        {:else if page === "matchmaking"}
          <section class="matchmaking-panel" aria-label={t("matchmaking")}>
            <header class="matchmaking-header">
              <div class="matchmaking-heading">
                <span
                  class="matchmaking-status-mark"
                  class:working={matchmaking.phase === "awaiting_player" || matchmaking.phase === "matching" || matchmaking.phase === "committed"}
                  class:ready={matchmaking.phase === "in_game"}
                  class:failed={matchmaking.phase === "failed"}
                  aria-hidden="true"
                >
                  {#key matchmakingStatusIconKey(matchmaking.phase)}
                    <span
                      class="matchmaking-status-icon"
                      transition:surfaceMotion={{ duration: 170, exitDuration: 110, offsetY: 0, startScale: .94, easing: inlineNoticeSurfaceEasing }}
                    >
                      {#if matchmaking.phase === "in_game"}<Check size={15} />{:else if matchmaking.phase === "failed"}<X size={15} />{:else}<Activity size={15} />{/if}
                    </span>
                  {/key}
                </span>
                <div><strong>{matchmakingPhaseLabel(matchmaking.phase)}</strong>{#if matchmaking.message}<span>{matchmaking.message}</span>{/if}</div>
              </div>
              <div class="matchmaking-count"><strong>{matchmaking.matched_bots}/{matchmaking.bots.length || selectedOnlineBotCount}</strong><span>{t("matched")} · {t("minimumMatches")} {matchmaking.required_matches || requiredMatches}</span></div>
            </header>
            <div class="matchmaking-config">
              <div class="matchmaking-game-picker">
                <span>{t("gameType")}</span>
                <div class="matchmaking-game-tabs" role="tablist" aria-label={t("gameType")}>
                  {#each modeGroups as group}
                    <button
                      type="button"
                      class="matchmaking-game-tab"
                      role="tab"
                      class:selected={matchGame === group.value}
                      aria-selected={matchGame === group.value}
                      disabled={matchActive}
                      on:click={() => selectMatchGame(group.value)}
                    >
                      <span class="matchmaking-game-tab-icon" aria-hidden="true">
                        {#if group.value === "bedwars"}<BedDouble size={14} />{:else if group.value === "duels"}<Swords size={14} />{:else}<Cloud size={14} />{/if}
                      </span>
                      <span>{group.label}</span>
                    </button>
                  {/each}
                </div>
              </div>
              <label><span class="matchmaking-field-label">{t("gameMode")}</span><SelectMenu value={matchMode} ariaLabel={t("gameMode")} options={modeOptionsForGame(matchGame, favoriteMatchModes)} disabled={matchActive} maxVisibleOptions={6} favoriteValues={favoriteMatchModes} onToggleFavorite={toggleFavoriteMatchMode} addFavoriteLabel={t("addFavorite")} removeFavoriteLabel={t("removeFavorite")} onSelect={selectMatchMode} /></label>
              <label class="match-log-field"><span class="matchmaking-field-label">{t("playerLog")}</span><input bind:value={matchLogPath} disabled={matchActive} placeholder="C:\...\logs\latest.log" /></label>
              <label><span class="matchmaking-field-label">{t("minimumMatches")}</span><input type="number" min="1" max={Math.max(1, selectedOnlineBotCount)} bind:value={requiredMatches} disabled={matchActive} /></label>
              {#if matchGame === "duels"}
                <div
                  transition:surfaceMotion={{ duration: 180, exitDuration: 140, offsetY: 4, startScale: .99, easing: inlineNoticeSurfaceEasing }}
                  class="matchmaking-mode-notice"
                >
                  <Info size={15} />
                  <span>{t("duelsMatchmakingNotice")}</span>
                </div>
              {/if}
            </div>
            <div class="matchmaking-actions">
              <div class="matchmaking-action-control">
                <button class="matchmaking-command ripple-button" use:ripple={{ rippleColor: "#ADD8E6" }} class:active={matchActive} aria-pressed={matchActive} disabled={matchmakingBusy || (!matchActive && !selectedOnlineBotCount)} on:click={toggleMatchmaking}>
                  {#if matchActive}<Square size={16} />{t("stopMatching")}{:else}<Play size={16} />{t("startMatching")}{/if}
                </button>
                <button class="shortcut-key" class:recording={recordingShortcut === "stop"} title={t("shortcutHelp")} aria-label={`${matchActive ? t("stopMatching") : t("startMatching")}: ${t("shortcutHelp")}`} on:click={() => beginShortcutRecording("stop")}><kbd>{recordingShortcut === "stop" ? "…" : shortcutLabel(stopShortcut)}</kbd></button>
              </div>
              <div class="matchmaking-action-control">
                <button class="matchmaking-command ripple-button" use:ripple={{ rippleColor: "#ADD8E6" }} disabled={!matchActive} on:click={toggleMatchmakingOverlay}>{#if overlayVisible}<EyeOff size={16} />{t("hideOverlay")}{:else}<Radio size={16} />{t("showOverlay")}{/if}</button>
                <button class="shortcut-key" class:recording={recordingShortcut === "show_overlay"} title={t("shortcutHelp")} aria-label={`${overlayVisible ? t("hideOverlay") : t("showOverlay")}: ${t("shortcutHelp")}`} on:click={() => beginShortcutRecording("show_overlay")}><kbd>{recordingShortcut === "show_overlay" ? "…" : shortcutLabel(showOverlayShortcut)}</kbd></button>
              </div>
              {#if matchGame === "bedwars"}
                <label class="matchmaking-verification-toggle">
                  <span class="matchmaking-verification-copy"><ShieldCheck size={14} aria-hidden="true" /><span>{t("chatVerification")}</span></span>
                  <button type="button" class="inline-switch" class:on={verifyPresence} role="switch" aria-label={t("chatVerification")} aria-checked={verifyPresence} disabled={matchActive} on:click={togglePresenceVerification}>
                    <span aria-hidden="true"></span>
                  </button>
                </label>
              {/if}
              {#if matchGame === "duels"}
                <label class="matchmaking-verification-toggle">
                  <span class="matchmaking-verification-copy"><ShieldCheck size={14} aria-hidden="true" /><span>{t("pitchVerification")}</span></span>
                  <button type="button" class="inline-switch" class:on={verifyDuelPitch} role="switch" aria-label={t("pitchVerification")} aria-checked={verifyDuelPitch} disabled={matchActive} on:click={toggleDuelPitchVerification}>
                    <span aria-hidden="true"></span>
                  </button>
                </label>
              {/if}
              <span class="matchmaking-player"><span class="matchmaking-player-dot" class:active={matchmaking.phase !== "idle" && matchmaking.phase !== "failed"} aria-hidden="true"></span><Radio size={15} /><span class="matchmaking-player-copy">{matchmaking.player_server || matchmakingPhaseLabel(matchmaking.phase)}</span></span>
            </div>
            {#if matchmaking.bots.length}
              <div class="matchmaking-bots" aria-live="polite">
                {#each matchmaking.bots as botState (botState.bot_id)}
                  <div class="matchmaking-bot {matchBotClass(botState.phase)}" title={botState.phase === "queued" || botState.phase === "returning" ? t("queueing") : botState.message || botState.phase}>
                    <span class="matchmaking-result">
                      {#if botState.phase === "matched" || botState.phase === "afk"}<Check size={15} />
                      {:else if botState.phase === "unavailable"}<X size={15} />
                      {:else}<Activity size={14} />{/if}
                    </span>
                    <span>
                      <strong class="bot-identity"><MinecraftHead username={matchBotLabel(botState.bot_id)} size={24} /><span>{matchBotLabel(botState.bot_id)}</span></strong>
                      <small>
                        {#if botState.phase === "queued" || botState.phase === "returning"}
                          {t("queueing")}<span class="queueing-dots" aria-hidden="true"></span>
                        {:else}
                          {botState.server || botState.phase}
                        {/if}
                        · {botState.attempts} {t("attempts")}
                      </small>
                    </span>
                  </div>
                {/each}
              </div>
            {/if}
          </section>

        {:else if page === "accounts"}
          <section class="section-heading account-heading page-section-heading">
            <div><h2>{t("accountList")}</h2><p>{t("accountCreateHelp")}</p></div>
            <div class="heading-actions account-actions">
              <button class="account-action account-action-primary dark-button" use:ripple={{ rippleColor: "#ADD8E6" }} on:click={() => openAddDialog("microsoft")}>
                <span class="account-action-icon" aria-hidden="true"><LogIn size={16} /></span>
                <span>{t("microsoftLogin")}</span>
              </button>
              <div class="account-import-actions" role="group" aria-label={t("accountList")}>
                <button class="account-action account-action-secondary" use:ripple={{ rippleColor: "#ADD8E6" }} on:click={() => openAddDialog("access_token")}>
                  <span class="account-action-icon" aria-hidden="true"><Upload size={16} /></span>
                  <span>{t("importAccessToken")}</span>
                </button>
                <button class="account-action account-action-secondary" use:ripple={{ rippleColor: "#ADD8E6" }} on:click={() => openAddDialog("cookie")}>
                  <span class="account-action-icon" aria-hidden="true"><Upload size={16} /></span>
                  <span>{t("importCookie")}</span>
                </button>
              </div>
              <button class="account-action account-action-danger danger-button" use:ripple={{ rippleColor: "#FECACA" }} disabled={!selectedAccountIds.size} on:click={openDeleteDialog}>
                <span class="account-action-icon" aria-hidden="true"><Trash2 size={16} /></span>
                <span>{t("deleteSelected")}</span>
              </button>
            </div>
          </section>
          <div class="account-card-grid" aria-label={t("accountList")}>
            {#each accounts as account (account.id)}
              <div
                class="account-card-motion"
                in:surfaceMotion={{
                  duration: accountEntryIds.has(account.id) ? 180 : 0,
                  exitDuration: 160,
                  offsetY: accountEntryIds.has(account.id) ? 4 : 0,
                  startScale: accountEntryIds.has(account.id) ? .99 : 1,
                  easing: inlineNoticeSurfaceEasing,
                }}
                on:introend={() => finishAccountEntry(account.id)}
                out:surfaceMotion={{ duration: 180, exitDuration: 160, offsetY: -6, startScale: .985, easing: inlineNoticeSurfaceEasing }}
              >
                <AccountCard
                  username={account.username}
                  profileId={account.profile_id ?? ""}
                  usernameLabel={t("minecraftUsername")}
                  loginMethod={account.auth_kind === "microsoft" ? t("microsoftAuth") : account.auth_kind === "access_token" ? t("accessTokenAuth") : t("cookieAuth")}
                  loginMethodLabel="LOGIN"
                  status={credentialValid(account, phaseLabels[account.id] ?? "offline") ? t("valid") : t("invalid")}
                  statusTone={credentialStatusClass(account, phaseLabels[account.id] ?? "offline")}
                  selected={selectedAccountIds.has(account.id)}
                  on:click={() => selectedAccountIds = toggleSelection(selectedAccountIds, account.id)}
                />
              </div>
            {:else}
              <div
                transition:surfaceMotion={{ duration: 180, exitDuration: 120, offsetY: 4, startScale: .99, easing: inlineNoticeSurfaceEasing }}
                class="empty-row"
              >
                {t("noAccounts")}
              </div>
            {/each}
          </div>

        {:else if page === "sessions"}
          <div class="session-toolbar" aria-label={t("sessionLogToolbar")}>
            <div class="session-filter-group" role="group" aria-label={t("sessionLogFilter")}>
              <button class:active={sessionLogFilter === "all"} on:click={() => setSessionLogFilter("all")}>{t("allLogs")}</button>
              <button class:active={sessionLogFilter === "errors"} on:click={() => setSessionLogFilter("errors")}>{t("errorLogs")}</button>
              <button class:active={sessionLogFilter === "matchmaking"} on:click={() => setSessionLogFilter("matchmaking")}>{t("matchmakingLogs")}</button>
            </div>
            <button class="session-chat-toggle" class:active={showChatLogs} on:click={toggleChatLogs} aria-pressed={showChatLogs}><MessageSquare size={15} />{showChatLogs ? t("hideChatDebug") : t("showChatDebug")}</button>
            <button class="session-follow-toggle" class:active={followSessionTail} on:click={toggleSessionFollow} aria-pressed={followSessionTail}>{followSessionTail ? t("pauseLog") : t("resumeLog")}</button>
            {#if sessionUnreadCount}
              <button
                transition:surfaceMotion={{ duration: 160, exitDuration: 120, offsetY: -4, startScale: .99, easing: inlineNoticeSurfaceEasing }}
                class="session-unread"
                on:click={() => { followSessionTail = true; sessionUnreadCount = 0; void scrollSessionToLatest(true); }}
              >
                {t("newLogEvents")} · {sessionUnreadCount}
              </button>
            {/if}
            <div class="session-toolbar-actions"><button class="session-export-button" on:click={exportLog} disabled={!visibleSessionLogs.length}><Download size={17} />{t("exportLog")}</button><button class="session-stop-button dark-button" use:ripple={{ rippleColor: "#ADD8E6" }} on:click={stopSession}><Square size={17} />{t("stopSession")}</button></div>
          </div>
          <div class="session-console" class:session-console-empty={!visibleSessionLogs.length} bind:this={sessionConsole} on:scroll={handleSessionScroll} aria-live="polite">
            {#if visibleSessionLogs.length}
              <div class="session-log-lines">
                {#each visibleSessionLogs as entry (entry.id)}
                  <div class="session-log-line" class:error-line={entry.level === "error"}>
                    <time>{entry.timestamp}</time>
                    {#if entry.bot_id}<span class="session-log-actor"><MinecraftHead username={sessionActor(entry)} size={24} /></span>{:else}<span class="session-log-system" role="img" aria-label="Botting" title="Botting"><img src="/bedwars-boosting-icon.png" alt="" /></span>{/if}
                    <span>{entry.message}</span>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="session-empty-state" role="status">
                <span class="session-empty-icon" aria-hidden="true"><Activity size={22} /></span>
                <strong>{t("sessionIdle")}</strong>
                <small>{t("noSessionEvents")}</small>
              </div>
            {/if}
          </div>

        {:else}
          <section class="settings-panel">
            <label><span class="settings-label">{t("language")}</span><SelectMenu value={locale} ariaLabel={t("language")} options={[{ value: "en", label: "English" }, { value: "zh-CN", label: "简体中文" }, { value: "zh-TW", label: "繁體中文" }]} onSelect={(value) => setLocale(value as Locale)} /></label>
            <label><span class="settings-label">{t("exportLogPath")}</span><input bind:value={exportLogPath} on:input={saveExportLogPath} placeholder={t("defaultDownloadsFolder")} /></label>
          </section>
        {/if}
        </div>
        {/key}
      </div>
    </section>
  </main>
{/if}

{#if !isMatchmakingOverlay}
  <div
    class="app-entry-reveal"
    class:active={appEntryPhase === "preparing" || appEntryPhase === "expanding" || appEntryPhase === "playing"}
    class:preparing={appEntryPhase === "preparing"}
    class:expanding={appEntryPhase === "expanding"}
    class:playing={appEntryPhase === "playing"}
    style={`--app-entry-cover-duration: ${appEntryCoverDurationMs}ms; --app-window-expand-duration: ${appWindowExpandDurationMs}ms; --app-entry-duration: ${appEntryDurationMs}ms;`}
    aria-hidden={appEntryPhase !== "preparing" && appEntryPhase !== "expanding" && appEntryPhase !== "playing"}
    aria-live="polite"
    on:animationend={(event: AnimationEvent) => {
      if (event.target === event.currentTarget && event.animationName === "entry-reveal-sequence") {
        userApp.finishAppEntry();
      }
    }}
  >
    <div class="app-entry-surface">
      <div class="app-entry-brand">
        <span class="app-entry-icon" aria-hidden="true"><img src="/bedwars-boosting-icon.png" alt="" /></span>
        <DiaTextReveal
          className="app-entry-wordmark"
          text="Botting"
          paused={appEntryPhase !== "playing"}
          revealDuration={700}
          letterDelay={50}
          gradientDuration={1500}
          colors={theme === "dark" ? ["#C084FC", "#FB923C", "#FFFFFF"] : ["#258DE9", "#389DF6", "#CCE4FF"]}
        />
      </div>
    </div>
  </div>
{/if}

{#if logPathDialogOpen && !isMatchmakingOverlay}
  <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 0, startScale: 1, easing: modalSurfaceEasing }} class="modal-backdrop" class:modal-active={logPathDialogOpen} role="presentation" on:click={(event) => event.currentTarget === event.target && userApp.closeLogPathDialog()}>
    <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 8, startScale: .985 }} class="modal log-path-modal" role="dialog" aria-modal="true" aria-labelledby="log-path-title" tabindex="-1">
      <header>
        <div class="confirm-heading"><span class="matchmaking-dialog-icon"><Radio size={19} /></span><div><h2 id="log-path-title">{t("logPathRequiredTitle")}</h2><p>{t("logPathRequiredHelp")}</p></div></div>
        <button class="icon-button" title={t("cancel")} on:click={() => userApp.closeLogPathDialog()}><X size={19} /></button>
      </header>
      <div class="modal-body"><label>{t("playerLog")}<input bind:value={matchLogPath} placeholder="C:\...\logs\latest.log" on:keydown={(event) => event.key === "Enter" && continueWithLogPath()} /></label></div>
      <footer><button on:click={() => userApp.closeLogPathDialog()}>{t("cancel")}</button><button class="dark-button" use:ripple={{ rippleColor: "#ADD8E6" }} disabled={!matchLogPath.trim()} on:click={continueWithLogPath}><Play size={17} />{t("continue")}</button></footer>
    </div>
  </div>
{/if}

{#if addDialog}
  <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 0, startScale: 1, easing: modalSurfaceEasing }} class="modal-backdrop" class:modal-active={addDialog !== null} role="presentation" on:click={(event) => event.currentTarget === event.target && closeAddDialog()}>
    <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 8, startScale: .985 }} class="modal" role="dialog" aria-modal="true" aria-labelledby="add-title" tabindex="-1">
      <header><h2 id="add-title">{addDialog === "microsoft" ? t("microsoftLogin") : addDialog === "cookie" ? t("importCookie") : t("importAccessToken")}</h2><button class="icon-button" title={t("cancel")} on:click={closeAddDialog}><X size={19} /></button></header>
      <div class="modal-body">
        {#if addDialog === "access_token"}<label>{t("minecraftJavaToken")}<textarea bind:value={formCredential} on:input={() => formCredentialError = ""} rows="4"></textarea>{#if formCredentialError}{#key formCredentialError}<small transition:surfaceMotion={{ duration: 160, exitDuration: 120, offsetY: -4, startScale: 1, easing: inlineNoticeSurfaceEasing }} class="field-error">{formCredentialError}</small>{/key}{/if}</label>{/if}
        {#if addDialog === "cookie"}<label>{t("microsoftCookieFile")}<div class="cookie-drop-zone" class:drag-active={cookieDragging}><textarea bind:value={formCredential} on:input={() => formCredentialError = ""} on:dragenter={startCookieDrag} on:dragover={startCookieDrag} on:dragleave={() => cookieDragging = false} on:drop={importCookieDrop} rows="7" placeholder={t("cookiePlaceholder")}></textarea><span class="cookie-drop-icon" title={t("dropCookieFile")} aria-hidden="true"><Upload size={18} /></span></div>{#if formCredentialError}{#key formCredentialError}<small transition:surfaceMotion={{ duration: 160, exitDuration: 120, offsetY: -4, startScale: 1, easing: inlineNoticeSurfaceEasing }} class="field-error">{formCredentialError}</small>{/key}{/if}</label>{/if}
        <label>{t("serverAddress")}<input bind:value={formServer} on:input={() => formServerError = ""} placeholder="play.example.net:25565" />{#if formServerError}{#key formServerError}<small transition:surfaceMotion={{ duration: 160, exitDuration: 120, offsetY: -4, startScale: 1, easing: inlineNoticeSurfaceEasing }} class="field-error">{formServerError}</small>{/key}{/if}</label>
        {#if formError}{#key formError}<div transition:surfaceMotion={{ duration: 160, exitDuration: 120, offsetY: -4, startScale: 1, easing: inlineNoticeSurfaceEasing }} class="form-error">{formError}</div>{/key}{/if}
      </div>
      <footer><button on:click={closeAddDialog}>{t("cancel")}</button><button class="dark-button" use:ripple={{ rippleColor: "#ADD8E6" }} disabled={submitting} on:click={submitAccount}><LogIn size={17} />{t("addAccount")}</button></footer>
    </div>
  </div>
{/if}

{#if deleteDialogOpen}
  <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 0, startScale: 1, easing: modalSurfaceEasing }} class="modal-backdrop" class:modal-active={deleteDialogOpen} role="presentation" on:click={(event) => event.currentTarget === event.target && !deletingAccounts && (deleteDialogOpen = false)}>
    <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 8, startScale: .985 }} class="modal confirm-modal" role="alertdialog" aria-modal="true" aria-labelledby="delete-title" tabindex="-1">
      <header><div class="confirm-heading"><span class="danger-icon"><Trash2 size={19} /></span><div><h2 id="delete-title">{t("deleteAccountsTitle")}</h2><p>{t("confirmDelete")}</p></div></div><button class="icon-button" title={t("cancel")} disabled={deletingAccounts} on:click={() => deleteDialogOpen = false}><X size={19} /></button></header>
      <footer><button disabled={deletingAccounts} on:click={() => deleteDialogOpen = false}>{t("cancel")}</button><button class="danger-button" disabled={deletingAccounts} on:click={deleteSelected}><Trash2 size={17} />{t("deleteSelected")}</button></footer>
    </div>
  </div>
{/if}

{#if deviceCode}
  <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 0, startScale: 1, easing: modalSurfaceEasing }} class="modal-backdrop" class:modal-active={deviceCode !== null}>
    <div transition:surfaceMotion={{ duration: 220, exitDuration: 180, offsetY: 8, startScale: .985 }} class="modal device-modal" role="dialog" aria-modal="true" tabindex="-1">
      <header><h2>{t("deviceLogin")}</h2><button class="icon-button" title={t("cancel")} on:click={() => userApp.clearDeviceCode()}><X size={19} /></button></header>
      <div class="modal-body"><p>{t("deviceHelp")}</p><button class="device-code" on:click={copyCode}><code>{deviceCode.code}</code>{#if copied}<Check size={19} />{:else}<Copy size={19} />{/if}</button><button class="dark-button link-button" use:ripple={{ rippleColor: "#ADD8E6" }} type="button" on:click={openDeviceLogin}><ExternalLink size={17} /><span>{t("openLink")}</span></button>{#if deviceLinkError}{#key deviceLinkError}<div transition:surfaceMotion={{ duration: 160, exitDuration: 120, offsetY: -4, startScale: 1, easing: inlineNoticeSurfaceEasing }} class="form-error">{deviceLinkError}</div>{/key}{/if}</div>
    </div>
  </div>
{/if}

{#if toast}
  <div transition:surfaceMotion={{ duration: 200, exitDuration: 160, offsetY: 100, offsetUnit: "%", startScale: .98, easing: toastSurfaceEasing }} class="toast" class:toast-error={toast.kind === "error"} role="status" aria-live="polite">
    {#if toast.kind === "success"}<Check size={18} />{:else}<X size={18} />{/if}
    <span>{toast.message}</span>
  </div>
{/if}
