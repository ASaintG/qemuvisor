<script lang="ts">
  import { onMount } from "svelte";
  import {
    checkDependencies,
    createVm,
    deleteVm,
    refreshHealth,
    startVm,
    stopVm
  } from "$lib/api";
  import type { DependencyStatus, VmConfig, VmStatus, VmSummary } from "$lib/types";
  import VmModal from "$lib/VmModal.svelte";
  import VncModal from "$lib/VncModal.svelte";

  let vms = $state<VmSummary[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let deps = $state<DependencyStatus | null>(null);
  let showModal = $state(false);
  let viewerVm: VmSummary | undefined = $state(undefined);
  let busyIds = $state<string[]>([]);

  onMount(() => {
    refresh();
    checkDeps();
  });

  function setError(err: unknown) {
    error = typeof err === "string" ? err : String(err);
  }

  async function checkDeps() {
    try {
      deps = await checkDependencies();
    } catch (err) {
      setError(err);
    }
  }

  const depsMissing = $derived(
    deps !== null && (!deps.qemu_available || !deps.qemu_img_available)
  );

  async function refresh() {
    loading = true;
    error = null;
    try {
      // Health check primero: detecta procesos QEMU muertos y después lista.
      vms = await refreshHealth();
    } catch (err) {
      setError(err);
    } finally {
      loading = false;
    }
  }

  function isBusy(id: string): boolean {
    return busyIds.includes(id);
  }

  function setBusy(id: string, busy: boolean) {
    busyIds = busy ? [...busyIds, id] : busyIds.filter((busyId) => busyId !== id);
  }

  async function handleCreate(config: VmConfig) {
    error = null;
    try {
      await createVm(config);
      showModal = false;
      await refresh();
    } catch (err) {
      setError(err);
    }
  }

  async function doStart(vm: VmSummary) {
    setBusy(vm.id, true);
    error = null;
    try {
      await startVm(vm.id);
      await refresh();
    } catch (err) {
      setError(err);
    } finally {
      setBusy(vm.id, false);
    }
  }

  async function doStop(vm: VmSummary) {
    setBusy(vm.id, true);
    error = null;
    try {
      await stopVm(vm.id);
      await refresh();
    } catch (err) {
      setError(err);
    } finally {
      setBusy(vm.id, false);
    }
  }

  async function doDelete(vm: VmSummary) {
    if (!confirm(`¿Eliminar la VM "${vm.name}"? El disco no se borrará del sistema.`)) return;
    setBusy(vm.id, true);
    error = null;
    try {
      await deleteVm(vm.id);
      await refresh();
    } catch (err) {
      setError(err);
    } finally {
      setBusy(vm.id, false);
    }
  }

  function statusInfo(status: VmStatus): { label: string; cls: string } {
    switch (status) {
      case "Running":
        return { label: "Running", cls: "running" };
      case "Paused":
        return { label: "Paused", cls: "paused" };
      case "Stopped":
        return { label: "Stopped", cls: "stopped" };
      default:
        return { label: `Error: ${status.Error}`, cls: "errored" };
    }
  }
</script>

<main class="container">
  <header class="topbar">
    <div>
      <h1>qemuvisor</h1>
      <p class="subtitle">Gestión de máquinas virtuales con QEMU</p>
    </div>
    <div class="topbar-actions">
      <button class="btn ghost" onclick={refresh} disabled={loading}>
        {loading ? "Refrescando…" : "Refrescar"}
      </button>
      <button class="btn primary" onclick={() => (showModal = true)}>+ Nueva VM</button>
    </div>
  </header>

  {#if depsMissing && deps}
    <div class="deps-banner" role="alert">
      <div class="deps-body">
        <strong>Faltan dependencias externas</strong>
        <span class="deps-message">{deps.error_message}</span>
        {#if deps.qemu_path}
          <span class="deps-found">qemu-system-x86_64: {deps.qemu_path}</span>
        {/if}
        {#if deps.qemu_img_path}
          <span class="deps-found">qemu-img: {deps.qemu_img_path}</span>
        {/if}
        <span class="deps-hint">
          Tras instalar QEMU, cierra y reabre la aplicación (o pulsa «Reintentar») para que se
          detecte.
        </span>
      </div>
      <button class="btn ghost" onclick={checkDeps}>Reintentar</button>
    </div>
  {/if}

  {#if error}
    <div class="alert" role="alert">
      <span>{error}</span>
      <button class="alert-close" onclick={() => (error = null)}>×</button>
    </div>
  {/if}

  <section class="card">
    <table>
      <thead>
        <tr>
          <th>Nombre</th>
          <th>RAM</th>
          <th>CPUs</th>
          <th>Estado</th>
          <th>VNC</th>
          <th class="col-actions">Acciones</th>
        </tr>
      </thead>
      <tbody>
        {#each vms as vm (vm.id)}
          <tr>
            <td class="vm-name">{vm.name}</td>
            <td>{vm.ram_mb} MB</td>
            <td>{vm.cpus}</td>
            <td>
              <span class="badge {statusInfo(vm.status).cls}">
                {statusInfo(vm.status).label}
              </span>
            </td>
            <td>
              {#if vm.status === "Running" && vm.vnc_port !== 0}
                <code>:{vm.vnc_port}</code>
              {:else}
                <span class="muted">—</span>
              {/if}
            </td>
            <td class="col-actions">
              {#if isBusy(vm.id)}
                <span class="muted">…</span>
              {:else}
                <div class="row-actions">
                  {#if vm.status === "Running"}
                    <button class="btn primary" onclick={() => (viewerVm = vm)}>
                      Conectar
                    </button>
                    <button class="btn danger" onclick={() => doStop(vm)}>Detener</button>
                  {:else if vm.status === "Paused"}
                    <button class="btn ghost" disabled title="Pause/resume no implementado aún">Iniciar</button>
                  {:else}
                    <button class="btn primary" onclick={() => doStart(vm)}>Iniciar</button>
                  {/if}
                  <button class="btn ghost danger-outline" onclick={() => doDelete(vm)}>Eliminar</button>
                </div>
              {/if}
            </td>
          </tr>
        {:else}
          {#if !loading}
            <tr>
              <td colspan="6" class="empty">No hay VMs registradas. Crea una con «+ Nueva VM».</td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>
  </section>
</main>

{#if showModal}
  <VmModal onsubmit={handleCreate} onclose={() => (showModal = false)} />
{/if}

{#if viewerVm}
  <VncModal vm={viewerVm} onclose={() => (viewerVm = undefined)} />
{/if}

<style>
  .container {
    max-width: 900px;
    margin: 0 auto;
    padding: 32px 24px;
  }

  .topbar {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 24px;
  }

  h1 {
    margin: 0;
    font-size: 1.6rem;
    letter-spacing: -0.01em;
  }

  .subtitle {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.85rem;
  }

  .topbar-actions {
    display: flex;
    gap: 10px;
  }

  .alert {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    background: rgba(248, 81, 73, 0.12);
    border: 1px solid rgba(248, 81, 73, 0.45);
    color: #ffb4af;
    border-radius: 10px;
    padding: 10px 14px;
    margin-bottom: 18px;
    font-size: 0.85rem;
  }

  .deps-banner {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    background: rgba(210, 153, 34, 0.12);
    border: 1px solid rgba(210, 153, 34, 0.5);
    color: #e9c25a;
    border-radius: 10px;
    padding: 14px 16px;
    margin-bottom: 18px;
    font-size: 0.85rem;
  }

  .deps-body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .deps-message {
    color: var(--text);
    white-space: pre-wrap;
  }

  .deps-found {
    font-family: Consolas, monospace;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .deps-hint {
    color: var(--muted);
  }

  .alert-close {
    background: none;
    border: none;
    color: inherit;
    font-size: 1.2rem;
    cursor: pointer;
    line-height: 1;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
  }

  th,
  td {
    padding: 12px 16px;
    text-align: left;
    border-bottom: 1px solid var(--border);
  }

  thead th {
    color: var(--muted);
    font-weight: 600;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    background: var(--surface-2);
  }

  tbody tr:last-child td {
    border-bottom: none;
  }

  .vm-name {
    font-weight: 600;
  }

  .col-actions {
    width: 220px;
  }

  .row-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .empty {
    text-align: center;
    color: var(--muted);
    padding: 32px;
  }

  .badge {
    display: inline-block;
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .badge.running {
    background: rgba(63, 185, 80, 0.15);
    color: var(--green);
    border: 1px solid rgba(63, 185, 80, 0.4);
  }

  .badge.stopped {
    background: rgba(248, 81, 73, 0.15);
    color: var(--red);
    border: 1px solid rgba(248, 81, 73, 0.4);
  }

  .badge.paused {
    background: rgba(210, 153, 34, 0.15);
    color: var(--yellow);
    border: 1px solid rgba(210, 153, 34, 0.4);
  }

  .badge.errored {
    background: rgba(139, 152, 165, 0.15);
    color: var(--muted);
    border: 1px solid var(--border);
  }

  .btn {
    border: 1px solid transparent;
    border-radius: 8px;
    padding: 7px 14px;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    color: var(--text);
    background: var(--surface-2);
    transition: filter 0.15s ease, border-color 0.15s ease;
  }

  .btn:hover:not(:disabled) {
    filter: brightness(1.15);
  }

  .btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .btn.primary {
    background: var(--accent);
    color: #fff;
  }

  .btn.danger {
    background: rgba(248, 81, 73, 0.18);
    border-color: rgba(248, 81, 73, 0.5);
    color: #ff9d97;
  }

  .btn.ghost {
    background: transparent;
    border-color: var(--border);
    color: var(--text);
  }

  .btn.danger-outline {
    color: #ff9d97;
  }

  .muted {
    color: var(--muted);
  }
</style>
