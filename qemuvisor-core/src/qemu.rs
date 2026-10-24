//! Backend de referencia sobre QEMU (`qemu-system-x86_64`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::backend::HypervisorBackend;
use crate::deps::{
    QEMU_IMG_BINARY, QEMU_SYSTEM_BINARY, DependencyStatus, check_dependencies as deps_check,
};
use crate::disk::create_disk;
use crate::error::{CoreError, CoreResult, install_hint_for_current_os};
use crate::types::{VmConfig, VmId, VmStatus, VmSummary};
use uuid::Uuid;

/// Tamaño usado al auto-crear el disco cuando `disk_path` aún no existe.
const DEFAULT_DISK_SIZE_GB: u64 = 10;

/// Snapshot persistido de una VM en `<vms_dir>/<id>.json`: la configuración
/// original con la que se creó y el resumen visible (estado, puerto VNC).
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedVm {
    config: VmConfig,
    summary: VmSummary,
}

/// Backend que gestiona VMs con `qemu-system-x86_64`.
///
/// El estado de las VMs se mantiene en memoria: un registro de
/// [`VmSummary`] para consultas rápidas y el [`VmConfig`] original para
/// reconstruir la línea de comandos al arrancar. Cada proceso QEMU
/// arrancado se conserva para controlar su ciclo de vida.
///
/// Las rutas de los binarios externos (`qemu-system-x86_64` y `qemu-img`)
/// se resuelven de forma perezosa con [`crate::deps::find_binary`] (PATH,
/// variables de entorno de QEMU y ubicaciones comunes del SO) y se guardan
/// en caché una vez resueltas; si aún no están disponibles se reintenta en
/// cada operación, así que instalar QEMU "en caliente" funciona sin reiniciar.
///
/// TODO: migrar la pausa y los logs hacia un canal QMP.
#[derive(Debug, Default)]
pub struct QemuBackend {
    /// Directorio raíz para metadatos y discos gestionados por el backend.
    vms_dir: PathBuf,
    /// Resúmenes en memoria (estado visible para la UI).
    vms: Mutex<HashMap<VmId, VmSummary>>,
    /// Configuración original con la que se registró cada VM.
    configs: Mutex<HashMap<VmId, VmConfig>>,
    /// Procesos QEMU vivos por VM (solo presentes mientras están Running).
    processes: Mutex<HashMap<VmId, Child>>,
    /// Ruta absoluta resuelta de `qemu-system-x86_64` (caché).
    qemu_system_path: Mutex<Option<PathBuf>>,
    /// Ruta absoluta resuelta de `qemu-img` (caché).
    qemu_img_path: Mutex<Option<PathBuf>>,
}

impl QemuBackend {
    /// Crea un backend cuyo directorio de metadatos será `vms_dir`.
    ///
    /// No lee del disco: llama a [`restore_persisted`](Self::restore_persisted)
    /// después para hidratar las VMs guardadas en `<vms_dir>/<id>.json`.
    #[must_use]
    pub fn new(vms_dir: impl Into<PathBuf>) -> Self {
        Self {
            vms_dir: vms_dir.into(),
            vms: Mutex::new(HashMap::new()),
            configs: Mutex::new(HashMap::new()),
            processes: Mutex::new(HashMap::new()),
            qemu_system_path: Mutex::new(None),
            qemu_img_path: Mutex::new(None),
        }
    }

    /// Ruta del JSON persistido de una VM.
    fn vm_json_path(&self, id: &VmId) -> PathBuf {
        self.vms_dir.join(format!("{id}.json"))
    }

    /// Volca el estado actual de una VM a `<vms_dir>/<id>.json`.
    ///
    /// Escritura atómica (tmp + rename) para no dejar JSON corruptos si el
    /// proceso se corta a mitad de escritura.
    async fn persist_vm(&self, id: &VmId) -> CoreResult<()> {
        let config = self.configs.lock().await.get(id).cloned();
        let summary = self.vms.lock().await.get(id).cloned();

        if let (Some(config), Some(summary)) = (config, summary) {
            self.ensure_vms_dir().await?;
            let path = self.vm_json_path(id);
            let tmp = path.with_extension("json.tmp");
            let json = serde_json::to_string_pretty(&PersistedVm { config, summary })?;
            tokio::fs::write(&tmp, json).await?;
            tokio::fs::rename(&tmp, &path).await?;
        }
        Ok(())
    }

    /// Elimina el JSON persistido de una VM (si existe).
    async fn remove_persisted(&self, id: &VmId) {
        if let Err(err) = tokio::fs::remove_file(self.vm_json_path(id)).await {
            if err.kind() != std::io::ErrorKind::NotFound {
                eprintln!("could not remove persisted VM file for {id}: {err}");
            }
        }
    }

    /// Hidrata los HashMaps del backend desde los JSON de `<vms_dir>`.
    ///
    /// Se debe llamar justo tras [`new`](Self::new) (por ejemplo en el
    /// `setup` de Tauri). Los archivos con formato inválido se ignoran con
    /// una advertencia en stderr, sin bloquear el resto.
    ///
    /// Las VMs restauradas arrancan siempre como `Stopped` con `vnc_port = 0`:
    /// aunque el JSON diga `Running`, el proceso QEMU asociado ya no existe
    /// tras un reinicio de la app.
    pub async fn restore_persisted(&self) -> CoreResult<()> {
        let mut vms = self.vms.lock().await;
        let mut configs = self.configs.lock().await;

        let mut stream = tokio::fs::read_dir(&self.vms_dir).await?;
        let mut loaded = 0u32;
        while let Some(entry) = stream.next_entry().await? {
            let path = entry.path();
            // Solo archivos `<uuid>.json` (descarta logs/, tmp/ y basura).
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .is_none()
            {
                continue;
            }

            let content = match tokio::fs::read_to_string(&path).await {
                Ok(content) => content,
                Err(err) => {
                    eprintln!("skipping unreadable VM file {}: {err}", path.display());
                    continue;
                }
            };

            match serde_json::from_str::<PersistedVm>(&content) {
                Ok(data) => {
                    let summary = VmSummary {
                        vnc_port: 0,
                        status: VmStatus::Stopped,
                        ..data.summary
                    };
                    vms.insert(summary.id, summary);
                    configs.insert(data.summary.id, data.config);
                    loaded += 1;
                }
                Err(err) => eprintln!(
                    "skipping corrupted VM file {}: {err}",
                    path.display()
                ),
            }
        }

        if loaded > 0 {
            eprintln!("restored {loaded} persisted VM(s) from {}", self.vms_dir.display());
        }
        Ok(())
    }

    /// Garantiza que `vms_dir` exista, creándolo si es necesario.
    async fn ensure_vms_dir(&self) -> CoreResult<()> {
        tokio::fs::create_dir_all(&self.vms_dir).await?;
        Ok(())
    }

    /// Resuelve la ruta absoluta de un binario externo.
    ///
    /// Si ya hay una resolución en caché se reutiliza; en caso contrario se
    /// busca con [`crate::deps::find_binary`] y el resultado se guarda en
    /// caché.
    ///
    /// # Errores
    /// [`CoreError::BinaryNotFound`] con instrucciones de instalación si el
    /// binario no está disponible.
    async fn resolve_binary(
        &self,
        cache: &Mutex<Option<PathBuf>>,
        name: &str,
    ) -> CoreResult<PathBuf> {
        if let Some(path) = cache.lock().await.clone() {
            return Ok(path);
        }

        let path = crate::deps::find_binary(name).await?;
        *cache.lock().await = Some(path.clone());
        Ok(path)
    }

    /// Resuelve `qemu-system-x86_64` (con caché).
    async fn qemu_system(&self) -> CoreResult<PathBuf> {
        self.resolve_binary(&self.qemu_system_path, QEMU_SYSTEM_BINARY)
            .await
    }

    /// Resuelve `qemu-img` (con caché).
    async fn qemu_img(&self) -> CoreResult<PathBuf> {
        self.resolve_binary(&self.qemu_img_path, QEMU_IMG_BINARY).await
    }

    /// Comprueba la disponibilidad de las dependencias externas.
    ///
    /// A diferencia de la resolución interna de `qemu-system-x86_64` y
    /// `qemu-img`, este método no falla: siempre devuelve un
    /// [`DependencyStatus`] completo con las rutas resueltas y un mensaje
    /// accionable si algo falta.
    pub async fn check_dependencies(&self) -> DependencyStatus {
        let qemu = self.qemu_system().await;
        let qemu_img = self.qemu_img().await;

        let error_message = if qemu.is_ok() && qemu_img.is_ok() {
            None
        } else {
            let mut missing = Vec::new();
            if qemu.is_err() {
                missing.push(QEMU_SYSTEM_BINARY);
            }
            if qemu_img.is_err() {
                missing.push(QEMU_IMG_BINARY);
            }
            Some(format!(
                "Missing dependencies: {}. {}",
                missing.join(", "),
                install_hint_for_current_os()
            ))
        };

        DependencyStatus {
            qemu_available: qemu.is_ok(),
            qemu_img_available: qemu_img.is_ok(),
            qemu_path: qemu.ok().map(|p| p.display().to_string()),
            qemu_img_path: qemu_img.ok().map(|p| p.display().to_string()),
            error_message,
        }
    }

    /// Devuelve el contenido del log de QEMU de una VM.
    ///
    /// El log (`<vms_dir>/logs/<id>.log`) se trunca en cada arranque y
    /// acumula el `stderr` de QEMU mientras el proceso está vivo.
    ///
    /// # Errores
    /// [`CoreError::VmNotFound`] si la VM no está registrada;
    /// [`CoreError::Io`] ante fallo de lectura distinto de "no existe".
    /// Si la VM existe pero nunca arrancó, devuelve una cadena vacía.
    pub async fn get_logs(&self, id: &VmId) -> CoreResult<String> {
        if !self.vms.lock().await.contains_key(id) {
            return Err(CoreError::VmNotFound(*id));
        }

        match tokio::fs::read_to_string(log_path(&self.vms_dir, id)).await {
            Ok(content) => Ok(content),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(err) => Err(err.into()),
        }
    }

    /// Health check de una VM: verifica si su proceso QEMU sigue vivo.
    ///
    /// - Si el proceso vivía y murió, lo retira del registro y marca la VM
    ///   como [`VmStatus::Error`] con el código de salida observado.
    /// - Si el proceso vivía y sigue vivo, garantiza que el estado visible
    ///   sea [`VmStatus::Running`] (auto-sincronización).
    /// - Si no hay proceso registrado, devuelve el estado actual sin tocar
    ///   nada (una VM `Stopped` no tiene proceso y esto es normal).
    ///
    /// TODO: al implementar la persistencia (`<vms_dir>/<id>.json`),
    /// volcar aquí el cambio de estado a disco.
    pub async fn check_health(&self, id: &VmId) -> CoreResult<VmStatus> {
        let mut process_guard = self.processes.lock().await;

        if let Some(child) = process_guard.get_mut(id) {
            match child.try_wait()? {
                Some(status) => {
                    // El proceso murió por sí solo (crash, disco corrupto...).
                    process_guard.remove(id);
                    drop(process_guard);

                    let message = format!(
                        "qemu process exited unexpectedly (exit code: {:?})",
                        status.code()
                    );
                    let mut vms = self.vms.lock().await;
                    let new_status = vms.get_mut(id).map(|s| {
                        s.status = VmStatus::Error(message.clone());
                        s.status.clone()
                    });
                    drop(vms);
                    // Persiste el estado de error en el JSON de la VM.
                    self.persist_vm(id).await?;
                    return new_status.ok_or_else(|| CoreError::VmNotFound(*id));
                }
                None => {
                    // Vivo: consolida el estado visible como Running.
                    drop(process_guard);
                    let mut vms = self.vms.lock().await;
                    let status = vms.get_mut(id).map(|s| {
                        s.status = VmStatus::Running;
                        s.status.clone()
                    });
                    return status.ok_or_else(|| CoreError::VmNotFound(*id));
                }
            }
        }

        // Sin proceso: estado actual inalterado.
        drop(process_guard);
        let vms = self.vms.lock().await;
        let status = vms.get(id).map(|s| s.status.clone());
        status.ok_or_else(|| CoreError::VmNotFound(*id))
    }
}

/// Devuelve los argumentos de aceleración según el SO anfitrión.
fn accel_args() -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec!["-accel".into(), "whpx".into()]
    } else if cfg!(target_os = "linux") {
        vec!["-accel".into(), "kvm".into()]
    } else {
        // Sin aceleración nativa: QEMU cae a TCG (emulación pura).
        Vec::new()
    }
}

/// Construye la lista de argumentos para lanzar la VM en QEMU.
///
/// `vnc_display` es el **número de display** de QEMU (no el puerto TCP):
/// la sintaxis `-vnc 127.0.0.1:<d>` escucha raw VNC en `5900+<d>` y, con
/// `websocket=on`, el WebSocket en `5700+<d>`. El GUI usa ese puerto
/// WebSocket (verificado empíricamente en QEMU 10.x).
fn build_vm_args(config: &VmConfig, vnc_display: u16) -> Vec<String> {
    let mut args = vec![
        "-name".into(),
        config.name.clone(),
        "-m".into(),
        config.ram_mb.to_string(),
        "-smp".into(),
        config.cpus.to_string(),
        "-drive".into(),
        format!("file={},format=qcow2", config.disk_path.display()),
    ];

    if let Some(iso) = &config.iso_path {
        args.push("-cdrom".into());
        args.push(iso.display().to_string());
        // Arrancar desde CD-ROM cuando hay ISO.
        args.push("-boot".into());
        args.push("d".into());
    }

    // Sin ventana SDL/GTK: el display real lo maneja el visor VNC embebido
    // de la GUI. TODO: sustituir por SPICE o `screendump` de QMP si la GUI
    // cambia de estrategia para renderizar el framebuffer.
    args.push("-display".into());
    args.push("none".into());

    // Red NAT de usuario (SLIRP): la VM recibe DHCP (10.0.2.15) y sale a
    // internet a través del host sin ninguna configuración extra.
    args.push("-netdev".into());
    args.push("user,id=net0".into());
    args.push("-device".into());
    args.push("e1000,netdev=net0".into());

    args.push("-vnc".into());
    args.push(format!("127.0.0.1:{vnc_display},websocket=on"));

    args.extend(accel_args());
    args
}

/// Puertos VNC de una VM en arranque.
#[derive(Clone, Copy, Debug)]
struct VncPorts {
    /// Puerto raw del VNC de QEMU (`5900 + display`).
    raw: u16,
    /// Puerto WebSocket para noVNC: `5700 + display` (es decir, `raw - 200`).
    /// Confirmado empíricamente con `websocket=on` en QEMU 10.x.
    websocket: u16,
}

impl VncPorts {
    fn display(&self) -> u16 {
        self.raw - 5900
    }
}

/// Elige el puerto VNC para una VM recién arrancada.
///
/// Si `config.vnc_port` es distinto de 0 se respeta como raw fijo; en caso
/// contrario se buscan displays 1..=99 cuyo raw y WebSocket estén libres,
/// intentando primero el derivado del `VmId` para estabilidad.
async fn pick_vnc_ports(id: &VmId, config: &VmConfig) -> VncPorts {
    if config.vnc_port != 0 {
        return VncPorts {
            raw: config.vnc_port,
            websocket: config.vnc_port - 200,
        };
    }

    let preferred_raw = 5900 + ((id.0.as_u128() % 99) as u16) + 1;
    let mut displays: Vec<u16> = (0..=99).collect();
    displays.sort_by_key(|d| (5900u32 + *d as u32).abs_diff(preferred_raw as u32));

    for d in displays {
        let raw = 5900 + d;
        let websocket = raw - 200;
        if tokio::net::TcpListener::bind(("127.0.0.1", raw)).await.is_ok()
            && tokio::net::TcpListener::bind(("127.0.0.1", websocket)).await.is_ok()
        {
            return VncPorts { raw, websocket };
        }
    }

    // Último recurso (muy improbable llegar aquí): display derivado del id.
    let raw = preferred_raw;
    VncPorts {
        raw,
        websocket: raw - 200,
    }
}

/// Ruta del archivo de log de una VM.
fn log_path(vms_dir: &Path, id: &VmId) -> PathBuf {
    vms_dir.join("logs").join(format!("{id}.log"))
}

#[async_trait::async_trait]
impl HypervisorBackend for QemuBackend {
    /// Valida la configuración, garantiza el disco (`qcow2`) y registra la VM.
    ///
    /// Si `disk_path` no existe se crea automáticamente con
    /// `DEFAULT_DISK_SIZE_GB`; la creación de discos con tamaño explícito debe
    /// hacerse antes con [`crate::create_disk`].
    async fn create_vm(&self, config: VmConfig) -> CoreResult<VmId> {
        config.validate()?;

        if !config.disk_path.is_file() {
            if let Some(parent) = config.disk_path.parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await?;
                }
            }
            let size = if config.disk_size_gb == 0 {
                DEFAULT_DISK_SIZE_GB
            } else {
                config.disk_size_gb
            };
            self.qemu_img().await?;
            create_disk(&config.disk_path, size).await?;
        }

        self.ensure_vms_dir().await?;

        let id = VmId::new();
        let summary = VmSummary {
            id,
            name: config.name.clone(),
            status: VmStatus::Stopped,
            ram_mb: config.ram_mb,
            cpus: config.cpus,
            vnc_port: 0,
        };

        self.configs.lock().await.insert(id, config.clone());
        self.vms.lock().await.insert(id, summary);
        self.persist_vm(&id).await?;
        Ok(id)
    }

    /// Lanza el proceso `qemu-system-x86_64` asociado a la VM.
    async fn start_vm(&self, id: &VmId) -> CoreResult<()> {
        let config = {
            let configs = self.configs.lock().await;
            configs
                .get(id)
                .cloned()
                .ok_or_else(|| CoreError::VmNotFound(*id))?
        };

        let vms = self.vms.lock().await;
        let summary = vms.get(id).ok_or_else(|| CoreError::VmNotFound(*id))?;

        if summary.status == VmStatus::Running {
            return Err(CoreError::InvalidConfig(format!(
                "VM {id} is already running"
            )));
        }
        if summary.status == VmStatus::Paused {
            return Err(CoreError::InvalidConfig(format!(
                "VM {id} is paused; resume support is not implemented yet"
            )));
        }
        // El lock solo cubre la validación; se libera antes de spawnar.
        drop(vms);

        // Resolución de dependencias antes de spawmar: si falta el binario,
        // el error incluye instrucciones accionables de instalación.
        let qemu_binary = self.qemu_system().await?;

        // Puertos VNC: fijos si la config lo define, dinámicos en otro caso.
        // `vnc_port` del resumen guarda el puerto WebSocket, que es el que
        // consume el visor noVNC de la GUI.
        let ports = pick_vnc_ports(id, &config).await;
        let mut command = Command::new(qemu_binary);
        command
            .args(build_vm_args(&config, ports.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            // stderrpipado: la tarea de logging (abajo) lo vuelca a disco.
            .stderr(Stdio::piped());

        // TODO: capturar también `stdout` si en el futuro usamos
        // `-serial stdio` o similar para consolas de texto.
        let mut child = command.spawn().map_err(CoreError::Io)?;

        // Tarea asíncrona de logging: `stderr` de QEMU → `<vms_dir>/logs/<id>.log`.
        // El log se trunca en cada arranque (File::create) y la tarea muere
        // solo cuando el pipe se cierra (proceso QEMU terminado).
        let log_dir = self.vms_dir.join("logs");
        tokio::fs::create_dir_all(&log_dir).await?;
        let log_file_path = log_path(&self.vms_dir, id);
        let mut log_file = tokio::fs::File::create(&log_file_path).await?;
        if let Some(mut stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                loop {
                    match stderr.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if log_file.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                            // Flush inmediato para que `get_logs` responda en vivo.
                            if log_file.flush().await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }

        self.processes.lock().await.insert(*id, child);

        let mut vms = self.vms.lock().await;
        if let Some(summary) = vms.get_mut(id) {
            summary.status = VmStatus::Running;
            summary.vnc_port = ports.websocket;
        }
        drop(vms);
        self.persist_vm(id).await?;
        Ok(())
    }

    /// Detiene el proceso QEMU de la VM y marca su estado como `Stopped`.
    async fn stop_vm(&self, id: &VmId) -> CoreResult<()> {
        if !self.vms.lock().await.contains_key(id) {
            return Err(CoreError::VmNotFound(*id));
        }

        if let Some(mut child) = self.processes.lock().await.remove(id) {
            // TODO: usar un apagado limpio vía QMP (`system_powerdown`)
            // en vez de SIGKILL para permitir el flush de los discos.
            child.kill().await?;
            child.wait().await?;
        }

        let mut vms = self.vms.lock().await;
        if let Some(summary) = vms.get_mut(id) {
            summary.status = VmStatus::Stopped;
            summary.vnc_port = 0;
        }
        drop(vms);
        self.persist_vm(id).await?;
        Ok(())
    }

    /// Pausa la VM (emulación v1: solo cambia el estado en memoria).
    ///
    /// TODO: implementar la pausa real con el monitor QMP de QEMU
    /// (`stop` / `cont`) o con señales del SO donde estén disponibles;
    /// por ahora solo se refleja el estado, sin congelar la vCPU.
    async fn pause_vm(&self, id: &VmId) -> CoreResult<()> {
        let mut vms = self.vms.lock().await;
        let summary = vms.get_mut(id).ok_or_else(|| CoreError::VmNotFound(*id))?;
        match summary.status {
            VmStatus::Running => {
                summary.status = VmStatus::Paused;
            }
            VmStatus::Paused => {}
            _ => {
                return Err(CoreError::InvalidConfig(format!(
                    "VM {id} cannot be paused from its current state"
                )));
            }
        }
        Ok(())
    }

    /// Elimina la VM del registro, deteniéndola primero si está activa.
    ///
    /// No borra la imagen de disco: eso es responsabilidad de la capa superior.
    async fn delete_vm(&self, id: &VmId) -> CoreResult<()> {
        // Si falla el kill se propaga, para no dejar procesos huérfanos al borrar.
        if let Some(mut child) = self.processes.lock().await.remove(id) {
            child.kill().await?;
            child.wait().await?;
        }

        let removed = self.vms.lock().await.remove(id);
        self.configs.lock().await.remove(id);
        self.remove_persisted(id).await;

        match removed {
            Some(_) => Ok(()),
            None => Err(CoreError::VmNotFound(*id)),
        }
    }

    /// Devuelve el estado actual de la VM.
    async fn get_vm_status(&self, id: &VmId) -> CoreResult<VmStatus> {
        let vms = self.vms.lock().await;
        let summary = vms.get(id).ok_or_else(|| CoreError::VmNotFound(*id))?;
        Ok(summary.status.clone())
    }

    /// Devuelve un resumen de todas las VMs registradas.
    async fn list_vms(&self) -> CoreResult<Vec<VmSummary>> {
        let vms = self.vms.lock().await;
        Ok(vms.values().cloned().collect())
    }
}

/// Estado de dependencias calculado sin tocar la caché del backend
/// (útil para chequeos públicos rápidos, por ejemplo desde la CLI).
#[must_use]
pub fn quick_dependency_check() -> DependencyStatus {
    deps_check()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> VmConfig {
        VmConfig {
            name: "persisted-vm".into(),
            ram_mb: 2048,
            cpus: 2,
            iso_path: None,
            disk_path: "C:\\fake\\disk.qcow2".into(),
            disk_size_gb: 10,
            vnc_port: 0,
        }
    }

    #[tokio::test]
    async fn restore_reads_persisted_vms() {
        let dir =
            std::env::temp_dir().join(format!("qemuvisor-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let id = VmId::new();
        let data = PersistedVm {
            config: sample_config(),
            summary: VmSummary {
                id,
                name: "persisted-vm".into(),
                // Persistida "Running": al restaurar debe forzarse Stopped.
                status: VmStatus::Running,
                ram_mb: 2048,
                cpus: 2,
                vnc_port: 1234,
            },
        };
        std::fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_string_pretty(&data).unwrap(),
        )
        .unwrap();

        // Un archivo sin relación con VMs no debe romper la restauración.
        std::fs::write(dir.join("garbage.json"), "not a vm").unwrap();

        let backend = QemuBackend::new(&dir);
        backend.restore_persisted().await.expect("restore");

        let vms = backend.list_vms().await.unwrap();
        assert_eq!(vms.len(), 1);
        assert_eq!(vms[0].id, id);
        assert_eq!(vms[0].name, "persisted-vm");
        // Nunca se confía en el estado del proceso tras reiniciar la app.
        assert_eq!(vms[0].status, VmStatus::Stopped);
        assert_eq!(vms[0].vnc_port, 0);
        // La config también se restaura para poder hacer start_vm luego.
        assert!(backend.configs.lock().await.contains_key(&id));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
