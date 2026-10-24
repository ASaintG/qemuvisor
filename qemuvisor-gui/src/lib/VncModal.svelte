<script lang="ts">
  import RFB from "@novnc/novnc";
  import { onMount } from "svelte";
  import { getVmLogs } from "$lib/api";
  import type { VmSummary } from "$lib/types";

  interface Props {
    vm: VmSummary;
    onclose: () => void;
  }

  let { vm, onclose }: Props = $props();

  let screenEl: HTMLDivElement | undefined = $state();
  let logEl: HTMLPreElement | undefined = $state();
  let activeTab = $state<"viewer" | "logs">("viewer");
  let connected = $state(false);
  let connecting = $state(true);
  let viewerError = $state<string | null>(null);
  let logs = $state("");
  let logsError = $state<string | null>(null);

  let rfb: RFB | null = null;
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  async function loadLogs() {
    try {
      logs = await getVmLogs(vm.id);
      logsError = null;
      // Auto-scroll al final del log.
      if (logEl) logEl.scrollTop = logEl.scrollHeight;
    } catch (err) {
      logsError = typeof err === "string" ? err : String(err);
    }
  }

  function attachViewer() {
    if (!screenEl || !vm.vnc_port || rfb !== null) return;

    // RFB conecta vía WebSocket a QEMU (`-vnc ...websocket=on`).
    rfb = new RFB(screenEl, `ws://127.0.0.1:${vm.vnc_port}`);
    rfb.scaleViewport = true; // Pantalla del huésped escalada al contenedor.
    rfb.resizeSession = false; // Se respeta la resolución configurada de la VM.
    rfb.backgroundColor = "#000000";

    rfb.addEventListener("connect", () => {
      connected = true;
      connecting = false;
      viewerError = null;
    });
    rfb.addEventListener("disconnect", () => {
      connected = false;
      connecting = false;
      viewerError =
        "Se perdió la conexión con la VM (¿proceso terminado o VM detenida?).";
    });
  }

  function switchTab(tab: "viewer" | "logs") {
    activeTab = tab;
    if (tab === "viewer") {
      // Reintenta la conexión si la pestaña quedó sin cliente.
      if (!rfb && vm.status === "Running") attachViewer();
    } else {
      loadLogs();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onclose();
  }

  onMount(() => {
    if (vm.vnc_port === 0) {
      connecting = false;
      viewerError = "La VM no está corriendo: no hay puerto VNC asignado.";
    }
    attachViewer();
    // Polling de logs cada 2 s mientras el modal esté abierto.
    pollTimer = setInterval(loadLogs, 2000);

    return () => {
      if (pollTimer) clearInterval(pollTimer);
      // Al desmontar el modal, desconecta el cliente RFB.
      if (rfb) {
        rfb.disconnect();
        rfb = null;
      }
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  <div class="modal" role="dialog" aria-modal="true" aria-label="Visor de VM">
    <header class="modal-head">
      <div>
        <h2>VM «{vm.name}»</h2>
        <span class="sub">
          {#if vm.vnc_port !== 0}
            VNC: 127.0.0.1:{vm.vnc_port} · {connected ? "conectado" : connecting ? "conectando…" : "desconectado"}
          {:else}
            sin VNC (VM detenida)
          {/if}
        </span>
      </div>
      <div class="head-actions">
        <button
          class="tab {activeTab === 'viewer' ? 'active' : ''}"
          onclick={() => switchTab("viewer")}
        >
          Visor
        </button>
        <button
          class="tab {activeTab === 'logs' ? 'active' : ''}"
          onclick={() => switchTab("logs")}
        >
          Consola/Logs
        </button>
        <button class="btn ghost" onclick={onclose}>Cerrar</button>
      </div>
    </header>

    {#if activeTab === "viewer"}
      {#if vm.vnc_port === 0}
        <div class="placeholder">Arranca la VM para poder conectarte al visor.</div>
      {:else if viewerError}
        <div class="placeholder error">
          {viewerError}
          <button class="btn primary" onclick={attachViewer}>Reintentar</button>
        </div>
      {:else}
        <!-- Contenedor de la pantalla VNC; NoVNC pinta su propio canvas aquí -->
        <div class="screen" bind:this={screenEl}></div>
        {#if connecting}
          <div class="connecting">Conectando a 127.0.0.1:{vm.vnc_port}…</div>
        {/if}
      {/if}
    {:else}
      <pre class="logs" bind:this={logEl}>{logs}</pre>
      {#if logsError}
        <div class="logerror">{logsError}</div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }

  .modal {
    width: min(1100px, 95vw);
    height: min(750px, 92vh);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 16px;
    position: relative;
    display: flex;
    flex-direction: column;
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
  }

  .sub {
    color: var(--muted);
    font-size: 0.78rem;
    font-family: Consolas, monospace;
  }

  .head-actions {
    display: flex;
    gap: 8px;
  }

  .tab {
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .tab.active {
    color: var(--text);
    border-color: var(--accent);
  }

  .screen {
    flex: 1;
    min-height: 0;
    background: #000;
    border-radius: 10px;
    overflow: hidden;
  }

  .connecting {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    pointer-events: none;
  }

  .placeholder {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--muted);
    text-align: center;
    padding: 24px;
  }

  .placeholder.error {
    color: #ffb4af;
  }

  .logs {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 12px;
    overflow: auto;
    background: #0a0e13;
    border: 1px solid var(--border);
    border-radius: 10px;
    color: #9fe870;
    font-family: Consolas, monospace;
    font-size: 0.75rem;
    white-space: pre-wrap;
  }

  .logerror {
    color: #ffb4af;
    font-size: 0.8rem;
    margin-top: 8px;
  }
</style>
