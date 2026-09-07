<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { api } from './lib/api';
  import { CAPTURE_EVENT_NAME } from './lib/contract';
  import {
    LIVE_WINDOW_SIZE,
    appendLiveViewRecord,
    createLiveViewState,
    filterWorkbenchRecords,
    freezeLiveViewState,
    recordFromCaptureEvent,
    recordsFromSession,
    resumeLiveViewState,
    type WorkbenchFilter,
    type WorkbenchFilterMode,
    type WorkbenchRecord
  } from './lib/workbench';
  import type {
    AppSnapshot,
    CandidateProcess,
    CaptureEvent,
    CapturePhase,
    EnvironmentDiagnosis,
    RawBytes,
    SessionDetail,
    SessionSummary
  } from './lib/types';

  let appState: AppSnapshot | null = null;
  let diagnosis: EnvironmentDiagnosis | null = null;
  let candidates: CandidateProcess[] = [];
  let sessions: SessionSummary[] = [];
  let selectedSession: SessionDetail | null = null;
  let selectedCandidateId = '';
  let workspaceInput = '';
  let liveView = createLiveViewState();
  let selectedRecord: WorkbenchRecord | null = null;
  let selectedRawBytes: RawBytes | null = null;
  let rawBytesLoading = false;
  let liveListElement: HTMLDivElement | undefined;
  let followLatest = true;
  let messageFilter = '';
  let protocolFilter = '';
  let filterMode: WorkbenchFilterMode = 'all';
  let historyFilterMode: WorkbenchFilterMode = 'all';
  let historyMessageFilter = '';
  let historyProtocolFilter = '';
  let busy = false;
  let loading = true;
  let errorMessage = '';

  $: capture = appState?.capture;
  $: recovery = capture?.recovery;
  $: active = capture !== undefined &&
    ['starting', 'ready', 'capturing', 'stopping'].includes(capture.phase);
  $: canStart = Boolean(
    appState?.workspace &&
    diagnosis?.captureAllowed &&
    selectedCandidateId &&
    !active &&
    !busy
  );
  $: liveFilter = {
    messageId: messageFilter,
    protocolName: protocolFilter,
    mode: filterMode
  } satisfies WorkbenchFilter;
  $: historyFilter = {
    messageId: historyMessageFilter,
    protocolName: historyProtocolFilter,
    mode: historyFilterMode
  } satisfies WorkbenchFilter;
  $: liveRecords = liveView.records;
  $: liveViewFrozen = liveView.frozen;
  $: pendingRecordCount = liveView.pendingRecordCount;
  $: evictedRecordCount = liveView.evictedRecordCount;
  $: liveViewRecords = liveView.visibleRecords;
  $: filteredLiveRecords = filterWorkbenchRecords(liveViewRecords, liveFilter);
  $: historyRecords = selectedSession ? recordsFromSession(selectedSession) : [];
  $: filteredHistoryRecords = filterWorkbenchRecords(historyRecords, historyFilter);

  const phaseLabels: Record<CapturePhase, string> = {
    idle: '待命',
    starting: '正在附加采集器',
    ready: '双边界已附加，等待流量',
    capturing: '持续采集中',
    stopping: '正在正常停止',
    completed: '已正常停止',
    error: '附加或采集失败'
  };

  function formatTime(timestamp: number | null | undefined): string {
    if (!timestamp) return '—';
    return new Date(timestamp).toLocaleTimeString('zh-TW', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false
    });
  }

  function formatBytes(length: number): string {
    return `${length.toLocaleString()} B`;
  }

  function shortHash(hash: string): string {
    return `${hash.slice(0, 10)}…`;
  }

  function directionLabel(direction: 'request' | 'response'): string {
    return direction === 'request' ? '请求' : '响应';
  }

  function recordKindLabel(record: WorkbenchRecord): string {
    return record.kind === 'unparsed_payload' ? '未分类明文' : '协议帧';
  }

  function recordTransportLabel(transport: 'websocket' | 'http'): string {
    return transport === 'websocket' ? 'WebSocket' : 'HTTP';
  }

  function resetWorkbench(): void {
    liveView = createLiveViewState();
    selectedRecord = null;
    selectedRawBytes = null;
    rawBytesLoading = false;
    followLatest = true;
  }

  function freezeView(): void {
    if (liveView.frozen) return;
    liveView = freezeLiveViewState(liveView);
    followLatest = false;
  }

  function resumeView(): void {
    liveView = resumeLiveViewState(liveView);
    followLatest = true;
  }

  function toggleLiveView(): void {
    if (liveView.frozen) resumeView();
    else freezeView();
  }

  function handleLiveScroll(event: Event): void {
    const element = event.currentTarget as HTMLDivElement;
    const atLatest = element.scrollHeight - element.scrollTop - element.clientHeight < 16;
    followLatest = atLatest;
    if (!atLatest) freezeView();
    else if (liveView.frozen) resumeView();
  }

  async function selectRecord(record: WorkbenchRecord): Promise<void> {
    selectedRecord = record;
    selectedRawBytes = null;
    rawBytesLoading = true;
    errorMessage = '';
    try {
      selectedRawBytes = await api.readRawBytes(record.sessionId, record.rawFile);
    } catch (error) {
      errorMessage = String(error);
    } finally {
      rawBytesLoading = false;
    }
  }

  async function refreshSessions(): Promise<void> {
    if (!appState?.workspace) {
      sessions = [];
      return;
    }
    sessions = await api.listSessions();
  }

  async function refreshDiagnosis(): Promise<void> {
    diagnosis = await api.diagnoseEnvironment();
    candidates = diagnosis.candidates;
    // 单一候选可直接进入开始按钮；多个候选必须先由用户在选择器中确认。
    if (candidates.length === 1) {
      selectedCandidateId = candidates[0].id;
    } else if (!candidates.some((candidate) => candidate.id === selectedCandidateId)) {
      selectedCandidateId = '';
    }
  }

  async function load(): Promise<void> {
    loading = true;
    errorMessage = '';
    try {
      appState = await api.getAppState();
      workspaceInput = appState.workspace?.path ?? '';
      await refreshDiagnosis();
      await refreshSessions();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      loading = false;
    }
  }

  async function chooseWorkspace(): Promise<void> {
    busy = true;
    errorMessage = '';
    try {
      const workspace = await api.pickWorkspace();
      appState = { ...(appState as AppSnapshot), workspace };
      workspaceInput = workspace.path;
      await refreshDiagnosis();
      await refreshSessions();
    } catch (error) {
      if (!String(error).includes('取消')) errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function saveWorkspace(): Promise<void> {
    if (!workspaceInput.trim()) return;
    busy = true;
    errorMessage = '';
    try {
      const workspace = await api.setWorkspace(workspaceInput.trim());
      appState = { ...(appState as AppSnapshot), workspace };
      await refreshDiagnosis();
      await refreshSessions();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function startCapture(): Promise<void> {
    if (!canStart) return;
    busy = true;
    errorMessage = '';
    resetWorkbench();
    selectedSession = null;
    try {
      await api.startCapture(selectedCandidateId);
      appState = await api.getAppState();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function stopCapture(): Promise<void> {
    if (!active || busy) return;
    busy = true;
    errorMessage = '';
    try {
      const summary = await api.stopCapture();
      appState = await api.getAppState();
      await refreshSessions();
      const latest = sessions.find((session) => session.sessionId === summary.sessionId) ?? sessions[0];
      if (latest) selectedSession = await api.getSession(latest.sessionId);
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function openSession(session: SessionSummary): Promise<void> {
    busy = true;
    errorMessage = '';
    try {
      selectedSession = await api.getSession(session.sessionId);
      const records = recordsFromSession(selectedSession);
      if (records.length > 0) await selectRecord(records[records.length - 1]);
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function refreshRecovery(): Promise<void> {
    busy = true;
    errorMessage = '';
    try {
      await api.refreshRecoveryCandidates();
      appState = await api.getAppState();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function selectRecoveryCandidate(candidateId: string): Promise<void> {
    busy = true;
    errorMessage = '';
    try {
      await api.selectRecoveryCandidate(candidateId);
      appState = await api.getAppState();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function openSessionDirectory(session: SessionSummary): Promise<void> {
    errorMessage = '';
    try {
      await api.openSessionDirectory(session.sessionId);
    } catch (error) {
      errorMessage = String(error);
    }
  }

  function onCaptureEvent(event: CaptureEvent): void {
    const record = recordFromCaptureEvent(event);
    if (record) {
      liveView = appendLiveViewRecord(liveView, record, LIVE_WINDOW_SIZE);
    }
    if (appState) {
      const next = { ...appState.capture };
      next.sessionId = event.sessionId;
      if (event.kind === 'started') next.phase = 'starting';
      if (event.kind === 'ready') next.phase = 'ready';
      if (event.kind === 'raw_observation') {
        next.phase = 'capturing';
        next.observationCount += 1;
      }
      if (event.kind === 'protocol_frame') {
        next.phase = 'capturing';
        next.frameCount += 1;
      }
      if (event.kind === 'stopped' || event.kind === 'summary') next.phase = 'completed';
      if (event.kind === 'error') next.phase = 'error';
      appState = { ...appState, capture: next };
    }
    // 恢复状态包含分段/候选列表/磁盘水位等结构化字段，由 Rust 状态机维护；
    // 终端和恢复控制事件到达后重新读取一次，避免实时事件投影覆盖这些字段。
    if (event.kind === 'started' || event.kind === 'ready' || event.kind === 'error' || event.kind === 'summary') {
      api.getAppState().then((nextState) => {
        if (nextState.capture.sessionId === event.sessionId) appState = nextState;
      }).catch(() => undefined);
    }
  }

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    let disposed = false;
    listen<CaptureEvent>(CAPTURE_EVENT_NAME, (event) => onCaptureEvent(event.payload)).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    load();
    return () => {
      disposed = true;
      unlisten?.();
    };
  });
</script>

<svelte:head>
  <title>MiniTrace · 世界 Online 明文采集</title>
</svelte:head>

<main class="mx-auto flex min-h-screen max-w-[1500px] flex-col gap-5 px-8 py-7 text-slate-100">
  <header class="flex items-end justify-between border-b border-white/10 pb-5">
    <div>
      <div class="mb-2 flex items-center gap-3 text-xs font-semibold uppercase tracking-[0.28em] text-cyan-300/80">
        <span class="h-2 w-2 rounded-full bg-cyan-300 shadow-[0_0_15px_#67e8f9]"></span>
        Passive capture workspace
      </div>
      <h1 class="text-3xl font-semibold tracking-tight text-white">MiniTrace</h1>
      <p class="mt-1 text-sm text-slate-400">世界 Online 微信小游戏明文流量采集</p>
    </div>
    <div class="flex items-center gap-3 text-right text-xs text-slate-400">
      <span class="rounded-full border border-amber-300/20 bg-amber-300/10 px-3 py-1.5 text-amber-200">
        {#if diagnosis?.captureAllowed}支持正式采集{:else if diagnosis?.platform.toLowerCase() === 'windows'}平台接入、未验证{:else}兼容性诊断{/if}
      </span>
      <span class="rounded-full border border-white/10 bg-white/[0.03] px-3 py-1.5">Tauri 2 · {diagnosis?.platform ?? '桌面端'}</span>
    </div>
  </header>

  {#if loading}
    <section class="rounded-2xl border border-white/10 bg-white/[0.04] p-8 text-sm text-slate-400">正在载入 MiniTrace…</section>
  {:else}
    {#if errorMessage}
      <section class="flex items-start justify-between gap-4 rounded-xl border border-rose-300/20 bg-rose-400/10 px-4 py-3 text-sm text-rose-100">
        <span>{errorMessage}</span>
        <button class="text-rose-200 underline underline-offset-4" on:click={load}>重试</button>
      </section>
    {/if}

    {#if diagnosis}
      <section class="rounded-2xl border {diagnosis.captureAllowed ? 'border-emerald-300/20 bg-emerald-300/[0.035]' : 'border-amber-300/20 bg-amber-300/[0.035]'} p-5">
        <div class="flex items-start justify-between gap-5">
          <div>
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">Environment diagnosis</p>
            <h2 class="mt-1 text-lg font-medium text-white">{diagnosis.platform} 采集环境</h2>
          </div>
          <div class="flex items-center gap-3">
            <span class="rounded-full px-3 py-1 text-xs {diagnosis.captureAllowed ? 'bg-emerald-400/10 text-emerald-200' : 'bg-amber-400/10 text-amber-200'}">
              {diagnosis.captureAllowed ? '支持正式采集' : '仅兼容性诊断'}
            </span>
            <button class="rounded-lg border border-white/10 bg-white/[0.06] px-3 py-1.5 text-xs text-slate-300 transition hover:bg-white/10 disabled:opacity-40" on:click={refreshDiagnosis} disabled={busy || active}>重新诊断</button>
          </div>
        </div>
        <div class="mt-4 grid grid-cols-4 gap-3 text-xs">
          <div class="rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2"><span class="block text-slate-500">平台</span><strong class="mt-1 block font-medium text-slate-200">{diagnosis.platform}</strong></div>
          <div class="rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2"><span class="block text-slate-500">系统版本</span><strong class="mt-1 block font-medium text-slate-200">{diagnosis.osVersion}</strong></div>
          <div class="rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2"><span class="block text-slate-500">CPU 架构</span><strong class="mt-1 block font-medium text-slate-200">{diagnosis.architecture}</strong></div>
          <div class="rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2"><span class="block text-slate-500">微信版本</span><strong class="mt-1 block font-medium text-slate-200">{diagnosis.wechatVersion || '未知'}</strong></div>
        </div>
        {#if diagnosis.issues.length}
          <div class="mt-4 space-y-1.5">
            {#each diagnosis.issues as issue}
              <p class="text-xs {issue.blocking ? 'text-amber-200' : 'text-cyan-200'}"><span class="mr-2 inline-block h-1.5 w-1.5 rounded-full {issue.blocking ? 'bg-amber-300' : 'bg-cyan-300'}"></span>{issue.message}</p>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

    <section class="grid grid-cols-[1.1fr_0.9fr] gap-5">
      <div class="rounded-2xl border border-white/10 bg-[#111722]/80 p-5 shadow-2xl shadow-black/10">
        <div class="mb-4 flex items-start justify-between">
          <div>
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">01 / Workspace</p>
            <h2 class="mt-1 text-lg font-medium text-white">采集工作区</h2>
          </div>
          <span class="rounded-full px-3 py-1 text-xs {appState?.workspace ? 'bg-emerald-400/10 text-emerald-300' : 'bg-amber-400/10 text-amber-200'}">
            {appState?.workspace ? '已配置' : '首次运行'}
          </span>
        </div>
        <p class="mb-4 text-sm leading-6 text-slate-400">所有原始字节、追加式时间线与可重建索引都会写入此目录。重新启动后会自动复用。</p>
        <div class="flex gap-2">
          <input
            class="min-w-0 flex-1 rounded-lg border border-white/10 bg-black/20 px-3 py-2.5 text-sm text-slate-200 placeholder:text-slate-600"
            bind:value={workspaceInput}
            placeholder="选择一个本地目录"
            aria-label="采集工作区路径"
          />
          <button class="rounded-lg border border-cyan-300/20 bg-cyan-300/10 px-4 text-sm font-medium text-cyan-100 transition hover:bg-cyan-300/20 disabled:opacity-40" on:click={chooseWorkspace} disabled={busy}>选择目录</button>
          <button class="rounded-lg border border-white/10 bg-white/[0.06] px-4 text-sm font-medium text-slate-200 transition hover:bg-white/10 disabled:opacity-40" on:click={saveWorkspace} disabled={busy || !workspaceInput.trim()}>保存</button>
        </div>
        {#if appState?.workspace}
          <p class="mt-3 truncate font-mono text-[11px] text-slate-500" title={appState.workspace.path}>{appState.workspace.path}</p>
        {/if}
      </div>

      <div class="rounded-2xl border border-white/10 bg-[#111722]/80 p-5 shadow-2xl shadow-black/10">
        <div class="mb-4 flex items-start justify-between">
          <div>
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">02 / Target</p>
            <h2 class="mt-1 text-lg font-medium text-white">候选进程</h2>
          </div>
          <span class="text-xs text-slate-500">{candidates.length} 个合格候选</span>
        </div>
        <select class="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2.5 text-sm text-slate-200 disabled:opacity-50" bind:value={selectedCandidateId} aria-label="选择候选进程" disabled={candidates.length === 0 || busy}>
          {#if candidates.length !== 1}
            <option value="" disabled>{candidates.length > 1 ? '请选择要采集的运行链' : '没有合格候选进程'}</option>
          {/if}
          {#each candidates as candidate}
            <option value={candidate.id}>{candidate.displayName} · PID {candidate.pid}</option>
          {/each}
        </select>
        {#if diagnosis?.requiresCandidateConfirmation}
          <p class="mt-3 text-xs text-cyan-200/80">检测到多个合格候选。开始采集前必须确认目标进程。</p>
        {:else if !diagnosis?.captureAllowed}
          <p class="mt-3 text-xs text-amber-200/80">当前环境未通过完整资格检查，不会建立正式采集会话。</p>
        {/if}
        {#if candidates[0]}
          <div class="mt-4 grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
            <div><span class="text-slate-500">角色</span><span class="ml-2 text-slate-300">{candidates.find((item) => item.id === selectedCandidateId)?.processRole}</span></div>
            <div><span class="text-slate-500">微信</span><span class="ml-2 text-slate-300">{candidates.find((item) => item.id === selectedCandidateId)?.wechatVersion}</span></div>
            <div><span class="text-slate-500">架构</span><span class="ml-2 text-slate-300">{candidates.find((item) => item.id === selectedCandidateId)?.architecture}</span></div>
            <div><span class="text-slate-500">模块</span><span class="ml-2 text-slate-300">{candidates.find((item) => item.id === selectedCandidateId)?.modules.length} 个</span></div>
            <div class="col-span-2"><span class="text-slate-500">关键模块</span><span class="ml-2 text-slate-300">{candidates.find((item) => item.id === selectedCandidateId)?.modules.join('、')}</span></div>
          </div>
        {/if}
      </div>
    </section>

    <section class="rounded-2xl border border-white/10 bg-[#111722]/80 p-5 shadow-2xl shadow-black/10">
      <div class="flex items-center justify-between gap-5">
        <div class="flex items-center gap-4">
          <span class="relative flex h-11 w-11 items-center justify-center rounded-xl border border-cyan-300/20 bg-cyan-300/10 text-cyan-200">
            <span class="h-3 w-3 rounded-full {active ? 'animate-pulse bg-cyan-300 shadow-[0_0_18px_#67e8f9]' : 'bg-slate-500'}"></span>
          </span>
          <div>
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">03 / Capture</p>
            <h2 class="mt-1 text-lg font-medium text-white">{capture ? phaseLabels[capture.phase] : '待命'}</h2>
          </div>
        </div>
        <div class="flex items-center gap-7 text-sm">
          <div><span class="text-slate-500">原始观察</span><strong class="ml-2 font-mono text-white">{capture?.observationCount ?? 0}</strong></div>
          <div><span class="text-slate-500">协议帧</span><strong class="ml-2 font-mono text-cyan-200">{capture?.frameCount ?? 0}</strong></div>
          {#if canStart}
            <button class="rounded-lg bg-cyan-300 px-5 py-2.5 text-sm font-semibold text-slate-950 transition hover:bg-cyan-200" on:click={startCapture}>开始采集</button>
          {:else}
            <button class="rounded-lg border border-rose-300/30 bg-rose-300/10 px-5 py-2.5 text-sm font-semibold text-rose-100 transition hover:bg-rose-300/20 disabled:opacity-40" on:click={stopCapture} disabled={!active || busy}>正常停止</button>
          {/if}
        </div>
      </div>
      {#if active || liveRecords.length}
        <div class="mt-5 grid grid-cols-[1fr_auto] items-center gap-4 border-t border-white/[0.06] pt-4">
          <div class="h-1.5 overflow-hidden rounded-full bg-white/[0.06]"><div class="h-full w-2/3 rounded-full bg-gradient-to-r from-cyan-300 to-blue-400 {active ? 'animate-pulse' : ''}"></div></div>
          <span class="font-mono text-xs text-slate-500">{capture?.sessionId ?? '未建立会话'}</span>
        </div>
      {/if}
      {#if recovery}
        <div class="mt-4 grid gap-3 rounded-xl border border-white/[0.06] bg-black/10 p-4 text-xs">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="flex flex-wrap items-center gap-3">
              <span class="text-slate-500">会话恢复</span>
              <span class="rounded-full px-2.5 py-1 {recovery.state === 'attached' ? 'bg-emerald-400/10 text-emerald-200' : recovery.state === 'awaiting_candidate' || recovery.state === 'interrupted' ? 'bg-amber-400/10 text-amber-200' : 'bg-white/[0.06] text-slate-300'}">
                {recovery.state === 'attached' ? '已附加' : recovery.state === 'awaiting_candidate' ? '等待选择候选' : recovery.state === 'interrupted' ? '承载进程中断' : recovery.state === 'reattaching' ? '正在重新附加' : recovery.state === 'attach_failed' ? '重新附加失败' : recovery.state ?? '准备中'}
              </span>
              <span class="text-slate-500">分段 <strong class="font-mono text-slate-200">{recovery.segments.length}</strong></span>
              <span class="text-slate-500">中断 <strong class="font-mono text-amber-200">{recovery.interruptions.length}</strong></span>
              {#if recovery.currentPid}<span class="font-mono text-slate-500">PID {recovery.currentPid}</span>{/if}
            </div>
            {#if recovery.state === 'interrupted' || recovery.state === 'awaiting_candidate' || recovery.state === 'attach_failed'}
              <button class="rounded-lg border border-cyan-300/20 bg-cyan-300/10 px-3 py-1.5 text-cyan-100 transition hover:bg-cyan-300/20 disabled:opacity-40" on:click={refreshRecovery} disabled={busy}>重新扫描候选</button>
            {/if}
          </div>
          {#if recovery.lastError}
            <p class="text-amber-200/90">{recovery.lastError}</p>
          {/if}
          {#if recovery.state === 'awaiting_candidate' && recovery.candidates.length > 0}
            <div class="grid gap-2 border-t border-white/[0.06] pt-3">
              <p class="text-cyan-200/90">发现多个兼容候选。请选择同一《世界 Online》运行链，未选择前不会混入新进程：</p>
              <div class="grid gap-2 sm:grid-cols-2">
                {#each recovery.candidates as candidate}
                  <button class="rounded-lg border border-white/10 bg-white/[0.04] p-2 text-left transition hover:border-cyan-300/30 hover:bg-cyan-300/[0.06] disabled:opacity-40" on:click={() => selectRecoveryCandidate(candidate.id)} disabled={busy}>
                    <span class="block truncate text-slate-200">{candidate.displayName}</span>
                    <span class="mt-1 block font-mono text-[11px] text-slate-500">PID {candidate.pid} · {candidate.processRole}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}
          {#if recovery.disk?.warning}
            <p class="border-t border-amber-300/10 pt-3 text-amber-200">磁盘水位警告：剩余 {formatBytes(recovery.disk.freeBytes)}。采集仍会继续，不会自动停止或清理历史会话。</p>
          {/if}
          {#if recovery.zeroFrame}
            <p class="text-amber-200/80">当前会话尚未收到协议帧；停止后会明确标记为零帧状态。</p>
          {/if}
        </div>
      {/if}
    </section>

    <section class="grid min-h-[560px] grid-cols-[1.55fr_0.95fr] gap-5">
      <div class="rounded-2xl border border-white/10 bg-[#111722]/80 p-5">
        <div class="flex items-start justify-between gap-4">
          <div>
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">04 / Live protocol workbench</p>
            <h2 class="mt-1 text-lg font-medium text-white">实时协议检查</h2>
            <p class="mt-1 text-xs text-slate-500">窗口最多保留 {LIVE_WINDOW_SIZE} 条。淘汰只影响视图，不删除磁盘证据。</p>
          </div>
          <div class="flex items-center gap-2 text-xs">
            <span class="rounded-full {liveViewFrozen ? 'bg-amber-400/10 text-amber-200' : 'bg-emerald-400/10 text-emerald-200'} px-3 py-1.5">
              {liveViewFrozen ? '视图已冻结' : '跟随实时流'}
            </span>
            <button class="rounded-lg border border-white/10 bg-white/[0.06] px-3 py-1.5 text-slate-200 transition hover:bg-white/10" on:click={toggleLiveView} aria-pressed={liveViewFrozen}>
              {liveViewFrozen ? '恢复实时' : '冻结视图'}
            </button>
          </div>
        </div>

        <div class="mt-4 grid grid-cols-[0.8fr_1.2fr_0.85fr_auto] gap-2">
          <input class="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-xs text-slate-200 placeholder:text-slate-600" bind:value={messageFilter} placeholder="消息号" aria-label="按消息号筛选" />
          <input class="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-xs text-slate-200 placeholder:text-slate-600" bind:value={protocolFilter} placeholder="协议名称" aria-label="按协议名称筛选" />
          <select class="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-xs text-slate-200" bind:value={filterMode} aria-label="按方向或分类筛选">
            <option value="all">全部记录</option>
            <option value="request">请求</option>
            <option value="response">响应</option>
            <option value="unparsed">未分类明文</option>
          </select>
          <button class="rounded-lg border border-white/10 bg-white/[0.06] px-3 py-2 text-xs text-slate-300 transition hover:bg-white/10" on:click={() => { messageFilter = ''; protocolFilter = ''; filterMode = 'all'; }}>清除</button>
        </div>

        <div class="mt-3 flex items-center justify-between text-[11px] text-slate-500">
          <span>显示 {filteredLiveRecords.length} / {liveViewRecords.length} 条 · 实时窗口 {liveRecords.length} / {LIVE_WINDOW_SIZE}</span>
          <span>{#if liveViewFrozen}{pendingRecordCount} 条新记录在后台写入{#if !followLatest} · 已暂停跟随{/if}{:else}后端与磁盘持续运行{/if}</span>
        </div>
        {#if evictedRecordCount > 0}
          <p class="mt-1 text-[11px] text-cyan-200/70">已淘汰 {evictedRecordCount} 条实时显示项；原始观察仍保存在采集会话中。</p>
        {/if}

        <div bind:this={liveListElement} on:scroll={handleLiveScroll} class="mt-3 max-h-[410px] overflow-auto rounded-xl border border-white/[0.06]">
          {#if filteredLiveRecords.length === 0}
            <div class="flex h-64 items-center justify-center px-6 text-center text-sm text-slate-600">
              {#if liveRecords.length === 0}开始采集后，协议帧和未分类明文会出现在这里。{:else}当前筛选没有匹配记录。{/if}
            </div>
          {:else}
            <div class="min-w-[700px]">
              <div class="grid grid-cols-[72px_66px_70px_minmax(190px,1fr)_76px_110px] gap-2 border-b border-white/[0.06] bg-[#111722] px-3 py-2 text-[11px] text-slate-500">
                <span>时间</span><span>方向</span><span>消息号</span><span>协议名称</span><span>长度</span><span>SHA-256</span>
              </div>
              <div class="divide-y divide-white/[0.05]">
                {#each [...filteredLiveRecords].reverse() as record (record.id)}
                  <button class="grid w-full grid-cols-[72px_66px_70px_minmax(190px,1fr)_76px_110px] gap-2 px-3 py-3 text-left text-xs transition hover:bg-cyan-300/[0.04] {selectedRecord?.id === record.id ? 'bg-cyan-300/[0.08]' : ''}" on:click={() => selectRecord(record)} aria-label={`查看${recordKindLabel(record)} ${record.protocolName}`}>
                    <span class="font-mono text-slate-500">{formatTime(record.observedAtMs)}</span>
                    <span class={record.direction === 'request' ? 'text-violet-200' : 'text-emerald-200'}>{directionLabel(record.direction)}</span>
                    <span class="font-mono text-cyan-200">{record.messageId ?? '—'}</span>
                    <span class="min-w-0 truncate text-slate-300"><span class="mr-2 rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-slate-500">{recordTransportLabel(record.transport)}</span>{record.protocolName}</span>
                    <span class="font-mono text-slate-400">{formatBytes(record.length)}</span>
                    <span class="font-mono text-slate-600">{shortHash(record.sha256)}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </div>

      <aside class="rounded-2xl border border-white/10 bg-[#111722]/80 p-5">
        <div class="mb-4 flex items-start justify-between gap-3">
          <div>
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">Record inspector</p>
            <h2 class="mt-1 text-lg font-medium text-white">原始字节详情</h2>
          </div>
          {#if selectedRecord}<span class="rounded-full bg-cyan-300/10 px-2.5 py-1 text-[10px] text-cyan-200">已选中</span>{/if}
        </div>
        {#if !selectedRecord}
          <div class="flex h-[420px] items-center justify-center rounded-xl border border-dashed border-white/10 px-6 text-center text-sm text-slate-600">选择一条实时记录或历史记录，查看完整原始字节、采集分段和原始文件。</div>
        {:else}
          <div class="space-y-3 text-xs">
            <div class="flex items-center justify-between gap-3"><span class="text-slate-500">分类</span><span class="text-slate-200">{recordKindLabel(selectedRecord)} · {recordTransportLabel(selectedRecord.transport)}</span></div>
            <div class="flex items-center justify-between gap-3"><span class="text-slate-500">方向</span><span class="text-slate-200">{directionLabel(selectedRecord.direction)}</span></div>
            <div class="flex items-center justify-between gap-3"><span class="text-slate-500">消息号</span><span class="font-mono text-cyan-200">{selectedRecord.messageId ?? '未分类'}</span></div>
            <div class="flex items-center justify-between gap-3"><span class="text-slate-500">协议</span><span class="max-w-[220px] truncate text-right text-slate-200">{selectedRecord.protocolName}</span></div>
            <div class="flex items-center justify-between gap-3"><span class="text-slate-500">采集分段</span><span class="max-w-[220px] truncate font-mono text-right text-slate-300" title={selectedRecord.segmentId}>{selectedRecord.segmentId}</span></div>
            <div class="flex items-center justify-between gap-3"><span class="text-slate-500">连接</span><span class="max-w-[220px] truncate font-mono text-right text-slate-300" title={selectedRecord.connectionId}>{selectedRecord.connectionId}</span></div>
            <div><span class="text-slate-500">原始文件</span><p class="mt-1 break-all rounded-lg bg-black/20 p-2 font-mono text-[11px] text-cyan-200/80">{selectedRecord.rawFile}</p></div>
            <div><span class="text-slate-500">SHA-256</span><p class="mt-1 break-all rounded-lg bg-black/20 p-2 font-mono text-[11px] text-slate-400">{selectedRecord.sha256}</p></div>
            <div class="border-t border-white/[0.06] pt-3"><div class="flex items-center justify-between"><span class="text-slate-500">原始字节 · {formatBytes(selectedRecord.length)}</span>{#if rawBytesLoading}<span class="text-cyan-200">读取中…</span>{/if}</div>
              {#if selectedRawBytes}
                <pre class="mt-2 max-h-[170px] overflow-auto whitespace-pre-wrap break-all rounded-lg border border-cyan-300/10 bg-black/30 p-3 font-mono text-[11px] leading-5 text-emerald-200">{selectedRawBytes.bytesHex}</pre>
                <p class="mt-2 text-[10px] text-slate-600">已读取 {selectedRawBytes.length} B，SHA-256 {shortHash(selectedRawBytes.sha256)}</p>
              {:else if !rawBytesLoading}
                <p class="mt-2 rounded-lg bg-black/20 p-3 text-[11px] text-rose-200/80">无法读取原始字节，请检查会话文件是否仍然存在。</p>
              {/if}
            </div>
          </div>
        {/if}
      </aside>
    </section>

    <section class="rounded-2xl border border-white/10 bg-[#111722]/80 p-5">
      <div class="flex items-start justify-between gap-4">
        <div>
          <p class="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">05 / Session archive</p>
          <h2 class="mt-1 text-lg font-medium text-white">停止后的历史读取</h2>
          <p class="mt-1 text-xs text-slate-500">历史列表来自会话索引；实时窗口淘汰不会影响这里的完整记录。</p>
        </div>
        <span class="text-xs text-slate-500">{sessions.length} 条会话</span>
      </div>
      <div class="mt-4 grid grid-cols-[0.72fr_1.28fr] gap-5">
        <div class="max-h-[330px] space-y-2 overflow-auto pr-1">
          {#if sessions.length === 0}
            <div class="flex h-48 items-center justify-center rounded-xl border border-dashed border-white/10 px-5 text-center text-sm text-slate-600">完成一次采集后，会话会自动归档。</div>
          {:else}
            {#each sessions as session}
              <button class="w-full rounded-xl border border-white/[0.06] bg-black/10 p-3 text-left transition hover:border-cyan-300/20 hover:bg-cyan-300/[0.04] {selectedSession?.summary.sessionId === session.sessionId ? 'border-cyan-300/30 bg-cyan-300/[0.06]' : ''}" on:click={() => openSession(session)}>
                <div class="flex items-center justify-between gap-3"><span class="truncate font-mono text-xs text-slate-300">{session.sessionId}</span><span class="shrink-0 text-[11px] {session.status === 'completed' ? 'text-emerald-300' : 'text-amber-200'}">{session.status === 'completed' ? '完整' : '不完整'}</span></div>
                <div class="mt-2 flex flex-wrap gap-4 text-xs text-slate-500"><span>{formatTime(session.startedAtMs)}</span><span>{session.frameCount} 帧</span><span>{session.observationCount} 观察</span><span>{session.segments?.length ?? 1} 分段</span>{#if session.interruptions?.length}<span class="text-amber-200">{session.interruptions.length} 次中断</span>{/if}{#if session.zeroFrame}<span class="text-amber-200">零帧</span>{/if}</div>
              </button>
            {/each}
          {/if}
        </div>
        <div>
          {#if selectedSession}
            <div class="mb-3 flex flex-wrap items-center justify-between gap-3"><span class="font-mono text-xs text-cyan-200/80">{selectedSession.summary.sessionId}</span><div class="flex min-w-0 items-center gap-2"><span class="truncate font-mono text-[11px] text-slate-600" title={selectedSession.summary.sessionPath}>{selectedSession.summary.sessionPath}</span><button class="shrink-0 rounded-lg border border-cyan-300/20 bg-cyan-300/10 px-2.5 py-1.5 text-[11px] text-cyan-100 transition hover:bg-cyan-300/20" on:click={() => openSessionDirectory(selectedSession!.summary)}>打开目录</button></div></div>
            {#if selectedSession.summary.status === 'incomplete' || selectedSession.summary.zeroFrame || (selectedSession.summary.interruptions?.length ?? 0) > 0}
              <div class="mb-3 flex flex-wrap gap-2 text-[11px]">
                {#if selectedSession.summary.status === 'incomplete'}<span class="rounded-full bg-amber-400/10 px-2.5 py-1 text-amber-200">不完整会话：原始证据已保留</span>{/if}
                {#if selectedSession.summary.zeroFrame}<span class="rounded-full bg-amber-400/10 px-2.5 py-1 text-amber-200">零帧</span>{/if}
                {#if (selectedSession.summary.interruptions?.length ?? 0) > 0}<span class="rounded-full bg-cyan-400/10 px-2.5 py-1 text-cyan-200">{selectedSession.summary.interruptions?.length} 次中断 · {selectedSession.summary.segments?.length ?? 1} 个分段</span>{/if}
              </div>
            {/if}
            <div class="mb-3 grid grid-cols-[0.8fr_1.2fr_0.85fr] gap-2">
              <input class="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-xs text-slate-200 placeholder:text-slate-600" bind:value={historyMessageFilter} placeholder="消息号" aria-label="历史按消息号筛选" />
              <input class="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-xs text-slate-200 placeholder:text-slate-600" bind:value={historyProtocolFilter} placeholder="协议名称" aria-label="历史按协议名称筛选" />
              <select class="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-xs text-slate-200" bind:value={historyFilterMode} aria-label="历史按方向或分类筛选"><option value="all">全部记录</option><option value="request">请求</option><option value="response">响应</option><option value="unparsed">未分类明文</option></select>
            </div>
            <div class="max-h-[235px] overflow-auto rounded-xl border border-white/[0.06]">
              {#if filteredHistoryRecords.length === 0}
                <p class="p-6 text-center text-sm text-slate-600">当前筛选没有匹配记录。</p>
              {:else}
                <div class="min-w-[600px] divide-y divide-white/[0.05]">
                  {#each [...filteredHistoryRecords].reverse() as record (record.id)}
                    <button class="grid w-full grid-cols-[72px_66px_70px_minmax(180px,1fr)_76px_110px] gap-2 px-3 py-3 text-left text-xs transition hover:bg-cyan-300/[0.04] {selectedRecord?.id === record.id ? 'bg-cyan-300/[0.08]' : ''}" on:click={() => selectRecord(record)}>
                      <span class="font-mono text-slate-500">{formatTime(record.observedAtMs)}</span><span class={record.direction === 'request' ? 'text-violet-200' : 'text-emerald-200'}>{directionLabel(record.direction)}</span><span class="font-mono text-cyan-200">{record.messageId ?? '—'}</span><span class="min-w-0 truncate text-slate-300">{record.protocolName}</span><span class="font-mono text-slate-400">{formatBytes(record.length)}</span><span class="font-mono text-slate-600">{shortHash(record.sha256)}</span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          {:else}
            <div class="flex h-48 items-center justify-center rounded-xl border border-dashed border-white/10 px-6 text-center text-sm text-slate-600">选择左侧会话，读取停止后的完整协议帧和未分类明文。</div>
          {/if}
        </div>
      </div>
    </section>
  {/if}
</main>
