//! Descubrimiento y verificación de dependencias externas (QEMU, qemu-img).
//!
//! La estrategia de búsqueda, en orden, es:
//!
//! 1. **PATH del sistema** (`which`).
//! 2. **Variables de entorno de QEMU**: `QEMU_IMG_PATH` / `QEMU_SYSTEM_X86_64_PATH`
//!    (específicas por binario) y `QEMU_PATH` / `QEMU_HOME` / `QEMU_DIR`
//!    (genéricas, apuntan a la carpeta raíz de instalación de QEMU).
//! 3. **Ubicaciones comunes por SO** (carpetas de instalación típicas).
//!
//! Si nada funciona, se produce un [`CoreError::BinaryNotFound`] con la lista
//! de ubicaciones buscadas y una instrucción de instalación accionable según
//! el SO del anfitrión.

use std::env;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult, install_hint_for_current_os};

/// Binario de QEMU para huéspedes x86_64.
pub const QEMU_SYSTEM_BINARY: &str = "qemu-system-x86_64";

/// Binario auxiliar para manipulación de imágenes de disco.
pub const QEMU_IMG_BINARY: &str = "qemu-img";

/// Variables de entorno genéricas donde los usuarios suelen indicar la
/// carpeta de instalación de QEMU.
const GENERIC_ENV_VARS: [&str; 3] = ["QEMU_PATH", "QEMU_HOME", "QEMU_DIR"];

/// Nombre del ejecutable en el SO actual (`qemu-img.exe` en Windows).
fn exe_file_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Carpetas de instalación típicas de QEMU según el SO anfitrión.
#[must_use]
fn common_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    if cfg!(target_os = "windows") {
        dirs.extend([
            PathBuf::from(r"C:\Program Files\qemu"),
            PathBuf::from(r"C:\Program Files (x86)\qemu"),
            // Instalaciones vía MSYS2/MinGW: la oficial de QEMU para Windows
            // usa el entorno UCRT64 (mingw-w64-ucrt-x86_64-qemu), y también
            // existen los entornos clásicos MINGW64 y CLANG64.
            PathBuf::from(r"C:\msys64\ucrt64\bin"),
            PathBuf::from(r"C:\msys64\clang64\bin"),
            PathBuf::from(r"C:\msys64\mingw64\bin"),
            PathBuf::from(r"C:\msys64\usr\bin"),
        ]);
        if let Some(program_files) = env::var_os("ProgramFiles") {
            dirs.push(PathBuf::from(program_files).join("QEMU"));
        }
        if let Some(local_app_data) = env::var_os("LocalAppData") {
            dirs.push(PathBuf::from(local_app_data).join(r"Programs\qemu"));
        }
        dirs
    } else if cfg!(target_os = "linux") {
        dirs.extend([
            PathBuf::from("/usr/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/qemu/bin"),
        ]);
        dirs
    } else if cfg!(target_os = "macos") {
        dirs.extend([
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
        ]);
        dirs
    } else {
        dirs.extend([PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")]);
        dirs
    }
}

/// Candidatos derivados de variables de entorno, con etiquetas para el
/// informe de búsqueda.
fn env_probe_results(name: &str) -> (Vec<PathBuf>, Vec<String>) {
    let mut candidates = Vec::new();
    let mut labels = Vec::new();
    let specific = format!("{}_PATH", name.to_uppercase());

    for var in [specific.as_str(), GENERIC_ENV_VARS[0], GENERIC_ENV_VARS[1], GENERIC_ENV_VARS[2]] {
        if let Some(value) = env::var_os(var) {
            labels.push(format!("{var}={}", value.to_string_lossy()));
            let path = PathBuf::from(&value);
            if path.is_file() {
                candidates.push(path);
            } else {
                candidates.push(path.join(exe_file_name(name)));
            }
        }
    }

    (candidates, labels)
}

/// Indica si un candidato es un binario instalado utilizable.
fn is_usable_candidate(path: &Path) -> bool {
    path.is_file()
}

/// Intento local intensivo: coordina PATH, variables de entorno y ubicaciones
/// comunes.
fn find_binary_inner(name: &str) -> CoreResult<PathBuf> {
    let mut searched: Vec<String> = vec!["PATH".to_string()];

    // a) PATH del sistema.
    if let Ok(path) = which::which(name) {
        return Ok(path);
    }

    // b) Variables de entorno de QEMU.
    let (env_paths, env_labels) = env_probe_results(name);
    searched.extend(env_labels);
    for path in &env_paths {
        if is_usable_candidate(path) {
            return Ok(path.clone());
        }
    }

    // c) Ubicaciones comunes del sistema operativo.
    let dirs = common_dirs();
    searched.extend(dirs.iter().map(|dir| dir.display().to_string()));
    for dir in &dirs {
        let candidate = dir.join(exe_file_name(name));
        if is_usable_candidate(&candidate) {
            return Ok(candidate);
        }
    }

    Err(CoreError::BinaryNotFound {
        name: name.to_string(),
        search_paths: searched,
    })
}

/// Localiza un binario (`qemu-system-x86_64` o `qemu-img`) de forma inteligente.
///
/// # Errores
/// Devuelve [`CoreError::BinaryNotFound`] con las ubicaciones buscadas y
/// una instrucción de instalación para el SO actual si no se encuentra.
pub async fn find_binary(name: &str) -> CoreResult<PathBuf> {
    find_binary_inner(name)
}

/// Versión síncrona de [`find_binary`], para usarla en inicializadores no async.
pub fn find_binary_sync(name: &str) -> CoreResult<PathBuf> {
    find_binary_inner(name)
}

/// Estado de las dependencias externas, serializable para CLI y GUI.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DependencyStatus {
    /// ¿Se encontró `qemu-system-x86_64`?
    pub qemu_available: bool,
    /// ¿Se encontró `qemu-img`?
    pub qemu_img_available: bool,
    /// Ruta absoluta resuelta de `qemu-system-x86_64`, si está disponible.
    pub qemu_path: Option<String>,
    /// Ruta absoluta resuelta de `qemu-img`, si está disponible.
    pub qemu_img_path: Option<String>,
    /// Mensaje accionable con instrucciones si falta alguna dependencia.
    pub error_message: Option<String>,
}

/// Verifica la disponibilidad de ambos binarios y compone un resumen.
#[must_use]
pub fn check_dependencies() -> DependencyStatus {
    let qemu = find_binary_sync(QEMU_SYSTEM_BINARY);
    let qemu_img = find_binary_sync(QEMU_IMG_BINARY);

    let qemu_available = qemu.is_ok();
    let qemu_img_available = qemu_img.is_ok();

    let error_message = if qemu_available && qemu_img_available {
        None
    } else {
        let mut missing = Vec::new();
        if qemu.is_err() {
            missing.push(QEMU_SYSTEM_BINARY.to_string());
        }
        if qemu_img.is_err() {
            missing.push(QEMU_IMG_BINARY.to_string());
        }
        Some(format!(
            "Missing dependencies: {}. {}",
            missing.join(", "),
            install_hint_for_current_os()
        ))
    };

    DependencyStatus {
        qemu_available,
        qemu_img_available,
        qemu_path: qemu.ok().map(|p| p.display().to_string()),
        qemu_img_path: qemu_img.ok().map(|p| p.display().to_string()),
        error_message,
    }
}
