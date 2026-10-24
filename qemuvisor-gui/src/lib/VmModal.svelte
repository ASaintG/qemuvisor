<script lang="ts">
  import { onMount } from "svelte";
  import { appDataDir, join } from "@tauri-apps/api/path";
  import { getHostSpecs } from "./api";
  import type { HostSpecs, VmConfig } from "./types";

  interface Props {
    onsubmit: (config: VmConfig) => void;
    onclose: () => void;
  }

  let { onsubmit, onclose }: Props = $props();

  let name = $state("");
  let ramMb = $state(1024);
  let cpus = $state(2);
  let diskSizeGb = $state(10);
  let diskPath = $state("");
  let isoPath = $state("");
  let diskPlaceholder = $state("(se generará automáticamente)");

  /** Specs del host; null hasta que responde el backend Tauri. */
  let hostSpecs = $state<HostSpecs | null>(null);

  // Defaults seguros si `get_host_specs` falla (backend sin sysinfo, etc.).
  const FALLBACK_SPECS: HostSpecs = {
    total_ram_mb: 6144,
    usable_ram_mb: 4096,
    logical_cpus: 2,
  };

  let ramMax = $derived(hostSpecs?.usable_ram_mb ?? FALLBACK_SPECS.usable_ram_mb);
  let cpuMax = $derived(hostSpecs?.logical_cpus ?? FALLBACK_SPECS.logical_cpus);

  onMount(async () => {
    try {
      hostSpecs = await getHostSpecs();
      // Recorta los valores por defecto por si el host es muy modesto.
      if (ramMb > ramMax) ramMb = ramMax;
      if (cpus > cpuMax) cpus = cpuMax;
    } catch (err) {
      console.error("get_host_specs failed; using safe fallbacks", err);
      hostSpecs = FALLBACK_SPECS;
      if (ramMb > ramMax) ramMb = ramMax;
      if (cpus > cpuMax) cpus = cpuMax;
    }

    const base = await appDataDir();
    const example = await join(base, "disks", "<nombre>.qcow2");
    diskPlaceholder = `(opcional) ${example}`;
  });

  /** Advertencia: RAM consumida casi al máximo del margen del host. */
  let ramWarning = $derived(hostSpecs !== null && ramMb >= ramMax);
  /** Advertencia: se asignan todos los núcleos lógicos. */
  let cpuWarning = $derived(hostSpecs !== null && cpus >= cpuMax);

  function slug(value: string): string {
    return value.toLowerCase().replace(/[^a-z0-9_-]+/g, "_") || "vm";
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const trimmedName = name.trim();
    const disk =
      diskPath.trim() ||
      (await join(await appDataDir(), "disks", `${slug(trimmedName || "vm")}.qcow2`));

    onsubmit({
      name: trimmedName,
      ram_mb: ramMb,
      cpus,
      iso_path: isoPath.trim() === "" ? null : isoPath.trim(),
      disk_path: disk,
      disk_size_gb: diskSizeGb,
      vnc_port: 0, // asignación automática de puerto VNC
    });
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  <div class="modal" role="dialog" aria-modal="true" aria-label="Nueva VM">
    <h2>Nueva VM</h2>
    <form onsubmit={submit}>
      <label class="full">
        <span>Nombre</span>
        <input type="text" bind:value={name} required placeholder="Mi máquina virtual" />
      </label>

      <label>
        <span>RAM <b class="value">{ramMb} MB</b></span>
        <input type="range" min="256" max={ramMax} step="256" bind:value={ramMb} />
      </label>

      <label>
        <span>vCPUs <b class="value">{cpus}</b></span>
        <input type="range" min="1" max={cpuMax} step="1" bind:value={cpus} />
      </label>

      <label>
        <span>Tamaño del disco <b class="value">{diskSizeGb} GB</b></span>
        <input type="range" min="1" max="256" step="1" bind:value={diskSizeGb} />
      </label>

      <label class="full">
        <span>Ruta de la ISO (opcional)</span>
        <input type="text" bind:value={isoPath} placeholder="C:\isos\debian-12-netinst.iso" />
      </label>

      <details class="full advanced">
        <summary>Opciones avanzadas</summary>
        <label>
          <span>Ruta del disco (opcional, qcow2)</span>
          <input type="text" bind:value={diskPath} placeholder={diskPlaceholder} />
        </label>
      </details>

      {#if ramWarning || cpuWarning}
        <p class="warn">
          {#if ramWarning}
            Estás asignando el máximo de RAM permitido ({ramMax} MB). Deja margen
            para el sistema anfitrión o tu equipo puede volverse lento.
          {/if}
          {#if ramWarning && cpuWarning}
            {" · "}
          {/if}
          {#if cpuWarning}
            Estás asignando los {cpuMax} núcleos lógicos completos; el anfitrión
            quedará sin CPU dedicada.
          {/if}
        </p>
      {:else}
        <p class="hint">Límites tomados del host: hasta {ramMax} MB de RAM y {cpuMax} núcleos lógicos.</p>
      {/if}

      <div class="footer">
        <button type="button" class="btn ghost" onclick={onclose}>Cancelar</button>
        <button type="submit" class="btn primary">Crear VM</button>
      </div>
    </form>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }

  .modal {
    width: min(560px, 94vw);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 24px;
    box-shadow: 0 24px 48px rgba(0, 0, 0, 0.45);
  }

  h2 {
    margin: 0 0 18px;
    font-size: 1.25rem;
  }

  /* Grid simétrico de 2 columnas; las etiquetas .full ocupan las dos. */
  form {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  label.full,
  details.full {
    grid-column: 1 / -1;
  }

  span {
    font-size: 0.8rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }

  .value {
    color: var(--text);
    font-size: 0.82rem;
    font-weight: 600;
    text-transform: none;
    letter-spacing: normal;
  }

  input[type="text"] {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text);
    padding: 10px 12px;
    font-size: 0.9rem;
  }

  input[type="text"]:focus {
    outline: none;
    border-color: var(--accent);
  }

  /* Sliders con acento del tema */
  input[type="range"] {
    accent-color: var(--accent);
    width: 100%;
    height: 4px;
    cursor: pointer;
  }

  details.advanced summary {
    font-size: 0.8rem;
    color: var(--muted);
    cursor: pointer;
    user-select: none;
  }

  details.advanced summary:hover {
    color: var(--text);
  }

  details.advanced {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .hint,
  .warn {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 0.78rem;
  }

  .hint {
    color: var(--muted);
  }

  .warn {
    color: #f5b942;
  }

  .footer {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 4px;
  }
</style>
