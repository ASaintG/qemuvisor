//! Dominio de errores de `qemuvisor-core`.

use thiserror::Error;

use crate::types::VmId;

/// Resultado estándar del crate.
pub type CoreResult<T> = Result<T, CoreError>;

/// Todos los errores que puede producir la capa de core.
///
/// Está diseñado para propagarse hacia arriba (CLI / comandos Tauri) sin
/// usar `anyhow`, convirtiéndose a `String` cuando el frontend lo necesita.
#[derive(Debug, Error)]
pub enum CoreError {
    /// El binario `qemu-system-x86_64` no se encontró en el `PATH`.
    #[error("qemu-system-x86_64 not found in PATH")]
    QemuNotFound,

    /// El binario `qemu-img` no se encontró en el `PATH`.
    #[error("qemu-img not found in PATH")]
    QemuImgNotFound,

    /// `qemu-system-x86_64` terminó con un código de salida no exitoso.
    #[error("qemu-system-x86_64 failed (exit code: {code:?})\nstderr: {stderr}")]
    QemuFailed {
        /// Código de salida del proceso, si se pudo observar.
        code: Option<i32>,
        /// Salida de error estándar capturada del proceso.
        stderr: String,
    },

    /// `qemu-img` terminó con un código de salida no exitoso.
    #[error("qemu-img failed (exit code: {code:?})\nstderr: {stderr}")]
    QemuImgFailed {
        /// Código de salida del proceso, si se pudo observar.
        code: Option<i32>,
        /// Salida de error estándar capturada del proceso.
        stderr: String,
    },

    /// Error de I/O del sistema operativo (archivos, procesos, pipes...).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// La configuración de la VM no es válida (nombre vacío, RAM/cpus en 0, rutas...).
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// No existe ninguna VM registrada con el identificador dado.
    #[error("VM {0} not found")]
    VmNotFound(VmId),

    /// Binario externo (qemu-system-x86_64 / qemu-img) no hallado en PATH,
    /// variables de entorno de QEMU ni ubicaciones comunes del SO.
    #[error(
        "dependency '{name}' was not found (searched: {searched}). {hint}",
        searched = .search_paths.join("; "),
        hint = install_hint_for_current_os(),
    )]
    BinaryNotFound {
        /// Nombre del binario buscado (por ejemplo `qemu-img`).
        name: String,
        /// Ubicaciones/variables que se intentaron usar, para depuración.
        search_paths: Vec<String>,
    },

    /// La verificación de dependencias no pudo completarse.
    #[error("dependency check failed: {details}")]
    DependencyCheckFailed { details: String },

    /// Error al serializar/deserializar (persistencia JSON de VMs).
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Convierte `CoreError` en `String` para comandos Tauri / CLI simples.
impl From<CoreError> for String {
    fn from(err: CoreError) -> Self {
        err.to_string()
    }
}

/// Instrucción de instalación de QEMU según el SO del anfitrión, para dar
/// un mensaje accionable cuando un binario no se encuentre.
#[must_use]
pub fn install_hint_for_current_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "Install QEMU from https://qemu.org/download/#windows (or run `winget install SoftwareFreedomConservancy.QEMU`), then add its bin folder to PATH or set the QEMU_PATH environment variable to the install folder."
    } else if cfg!(target_os = "linux") {
        "Install QEMU with your distro's package manager (e.g. `sudo apt install qemu-system-x86 qemu-utils`, `sudo dnf install qemu` or `sudo pacman -S qemu`)."
    } else if cfg!(target_os = "macos") {
        "Install QEMU with Homebrew: `brew install qemu`."
    } else {
        "Install QEMU from https://qemu.org/download/ and make sure it is on PATH."
    }
}
