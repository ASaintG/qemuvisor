import { invoke } from "@tauri-apps/api/core";
import type { DependencyStatus, HostSpecs, VmConfig, VmSummary } from "./types";

export const checkDependencies = () => invoke<DependencyStatus>("check_dependencies");

/** Specs del host (RAM usable, núcleos lógicos) para límites del modal. */
export const getHostSpecs = () => invoke<HostSpecs>("get_host_specs");

export const listVms = () => invoke<VmSummary[]>("list_vms");

/** Health check de todas las VMs + lista actualizada. */
export const refreshHealth = () => invoke<VmSummary[]>("refresh_health");

/** Logs de QEMU de una VM. */
export const getVmLogs = (id: string) => invoke<string>("get_vm_logs", { id });

export const createVm = (config: VmConfig) => invoke<string>("create_vm", { config });

export const startVm = (id: string) => invoke<void>("start_vm", { id });

export const stopVm = (id: string) => invoke<void>("stop_vm", { id });

export const deleteVm = (id: string) => invoke<void>("delete_vm", { id });
