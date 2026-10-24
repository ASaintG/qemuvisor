/** Estado de una VM tal como lo serializa `qemuvisor-core`.
 * `VmStatus::Error(String)` llega como `{ Error: string }` (serde por defecto). */
export type VmStatus = "Stopped" | "Running" | "Paused" | { Error: string };

/** Resumen serializado de `VmSummary`. */
export interface VmSummary {
  id: string;
  name: string;
  status: VmStatus;
  ram_mb: number;
  cpus: number;
  /** Puerto VNC activo (0 si no está en ejecución). */
  vnc_port: number;
}

/** Configuración serializada de `VmConfig` (los nombres de campo deben
 * coincidir con los del struct Rust para que serde deserialice bien). */
export interface VmConfig {
  name: string;
  ram_mb: number;
  cpus: number;
  iso_path: string | null;
  disk_path: string;
  /** Tamaño del disco a crear si no existe, en GB. */
  disk_size_gb: number;
  /** 0 = asignación automática de puerto VNC. */
  vnc_port: number;
}

/** Estado de dependencias externas (QEMU / qemu-img). */
export interface DependencyStatus {
  qemu_available: boolean;
  qemu_img_available: boolean;
  qemu_path: string | null;
  qemu_img_path: string | null;
  error_message: string | null;
}

/** Especificaciones del host, para limitar los sliders de creación. */
export interface HostSpecs {
  /** RAM física total instalada, en MB. */
  total_ram_mb: number;
  /** RAM usable para una VM (total menos margen para el SO anfitrión). */
  usable_ram_mb: number;
  /** Número de núcleos lógicos del host. */
  logical_cpus: number;
}
