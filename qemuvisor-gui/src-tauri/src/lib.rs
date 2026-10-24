use std::path::PathBuf;

use tauri::Manager;
use qemuvisor_core::{
    CoreError, DependencyStatus, HypervisorBackend, QemuBackend, VmConfig, VmId, VmSummary,
};

/// Estado global de la aplicación compartido con todos los comandos Tauri.
///
/// El [`QemuBackend`] va envuelto en `tokio::sync::Mutex` porque sus métodos
/// son asíncronos y se ejecutan en el runtime de Tokio de Tauri.
pub struct AppState {
    backend: tokio::sync::Mutex<QemuBackend>,
}

/// Convierte a `String` un ID recibido desde el frontend (`CoreError`
/// tiene `From<CoreError> for String`, pero el parse del UUID necesita su map propio).
fn parse_vm_id(id: &str) -> Result<VmId, String> {
    uuid::Uuid::parse_str(id)
        .map(VmId)
        .map_err(|err| CoreError::InvalidConfig(format!("invalid VM id {id:?}: {err}")).to_string())
}

#[tauri::command]
async fn check_dependencies(
    state: tauri::State<'_, AppState>,
) -> Result<DependencyStatus, String> {
    let backend = state.backend.lock().await;
    Ok(backend.check_dependencies().await)
}

#[tauri::command]
async fn list_vms(state: tauri::State<'_, AppState>) -> Result<Vec<VmSummary>, String> {
    let backend = state.backend.lock().await;
    Ok(backend.list_vms().await?)
}

#[tauri::command]
async fn create_vm(config: VmConfig, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let backend = state.backend.lock().await;
    let id = backend.create_vm(config).await?;
    Ok(id.to_string())
}

#[tauri::command]
async fn start_vm(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let id = parse_vm_id(&id)?;
    let backend = state.backend.lock().await;
    backend.start_vm(&id).await?;
    Ok(())
}

#[tauri::command]
async fn stop_vm(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let id = parse_vm_id(&id)?;
    let backend = state.backend.lock().await;
    backend.stop_vm(&id).await?;
    Ok(())
}

#[tauri::command]
async fn delete_vm(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let id = parse_vm_id(&id)?;
    let backend = state.backend.lock().await;
    backend.delete_vm(&id).await?;
    Ok(())
}

/// Logs de QEMU de una VM (`<AppData>/vms/logs/<id>.log`).
#[tauri::command]
async fn get_vm_logs(id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let id = parse_vm_id(&id)?;
    let backend = state.backend.lock().await;
    Ok(backend.get_logs(&id).await?)
}

/// Health check de todas las VMs registradas y lista actualizada.
///
/// Detecta procesos QEMU muertos (crash) y actualiza su estado a
/// `Error(...)`; también consolida `Running` en los vivos. El frontend lo
/// llama en cada "Refrescar".
#[tauri::command]
async fn refresh_health(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<VmSummary>, String> {
    let backend = state.backend.lock().await;
    let ids: Vec<VmId> = backend
        .list_vms()
        .await?
        .into_iter()
        .map(|summary| summary.id)
        .collect();

    for id in &ids {
        // Un fallo puntual de health en una VM no debe abortar el resto.
        if let Err(err) = backend.check_health(id).await {
            eprintln!("check_health({id}) failed: {err}");
        }
    }

    Ok(backend.list_vms().await?)
}

/// Especificaciones del host para limitar los sliders del modal de creación.
#[derive(serde::Serialize)]
pub struct HostSpecs {
    /// RAM física total instalada, en MB.
    pub total_ram_mb: u64,
    /// RAM "usable" para una VM: total menos un margen para el SO anfitrión.
    pub usable_ram_mb: u64,
    /// Número de núcleos lógicos del host.
    pub logical_cpus: usize,
}

/// Margen de RAM (MB) reservado al anfitrión: los sliders de RAM del
/// frontend nunca superan `total - margen`.
const HOST_RAM_RESERVE_MB: u64 = 2048;

#[tauri::command]
async fn get_host_specs() -> Result<HostSpecs, String> {
    let system = sysinfo::System::new_all();

    let total_ram_mb = system.total_memory() / (1024 * 1024);
    let logical_cpus = system.cpus().len();

    let usable_ram_mb = total_ram_mb
        .saturating_sub(HOST_RAM_RESERVE_MB)
        .max(512); // floor sensible para equipos muy modestos

    Ok(HostSpecs {
        total_ram_mb,
        usable_ram_mb,
        logical_cpus,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Directorio de metadatos/discos gestionados, bajo el directorio
            // de datos de la aplicación (AppData en Windows).
            let data_dir: PathBuf = app.path().app_data_dir()?;
            let vms_dir = data_dir.join("vms");
            std::fs::create_dir_all(&vms_dir)?;

            let backend = QemuBackend::new(vms_dir);
            // Hidrata VMs desde <AppData>/vms/<id>.json: las VMs creadas en
            // sesiones anteriores aparecen en la lista (Estado: Stopped).
            // Un fallo de restauración no debe impedir abrir la app.
            if let Err(err) = tauri::async_runtime::block_on(backend.restore_persisted()) {
                eprintln!("failed to restore persisted VMs: {err}");
            }

            app.manage(AppState {
                backend: tokio::sync::Mutex::new(backend),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_dependencies,
            list_vms,
            create_vm,
            start_vm,
            stop_vm,
            delete_vm,
            get_vm_logs,
            refresh_health,
            get_host_specs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
