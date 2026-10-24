//! Tipos de dominio compartidos por todos los backends.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::CoreError;

/// Identificador único e inmutable de una VM.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VmId(pub Uuid);

impl VmId {
    /// Genera un nuevo identificador aleatorio (UUID v4).
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for VmId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for VmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuración declarativa con la que se crea una VM.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VmConfig {
    /// Nombre visible de la VM (se pasa a QEMU con `-name`).
    pub name: String,
    /// RAM asignada, en megabytes (se pasa a QEMU con `-m`).
    pub ram_mb: u32,
    /// Número de vCPUs (se pasa a QEMU con `-smp`).
    pub cpus: u32,
    /// Ruta opcional a una ISO para arrancar desde CD-ROM.
    pub iso_path: Option<PathBuf>,
    /// Ruta del disco virtual en formato `qcow2`.
    pub disk_path: PathBuf,
    /// Tamaño del disco a crear si aún no existe, en GB. `0` = usar el
    /// tamaño por defecto del backend (10 GB).
    pub disk_size_gb: u64,
    /// Puerto TCP fijo del servidor VNC *raw* de la VM (`5900 + display`).
    /// QEMU publica además el WebSocket en `vnc_port - 200` (es decir
    /// `5700 + display`, convención interna de QEMU con `websocket=on`);
    /// ese es el puerto que usa noVNC.
    /// `0` = asignar automáticamente el primer par libre desde 5901.
    pub vnc_port: u16,
}

impl VmConfig {
    /// Valida la configuración antes de crear la VM.
    ///
    /// # Errores
    /// Devuelve [`CoreError::InvalidConfig`] si el nombre está vacío,
    /// `ram_mb` o `cpus` son cero, si la ISO indicada no existe, o si
    /// `vnc_port` se sale del rango permitido.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::InvalidConfig("name must not be empty".into()));
        }
        if self.ram_mb == 0 {
            return Err(CoreError::InvalidConfig("ram_mb must be greater than 0".into()));
        }
        if self.cpus == 0 {
            return Err(CoreError::InvalidConfig("cpus must be greater than 0".into()));
        }
        if self.disk_path.as_os_str().is_empty() {
            return Err(CoreError::InvalidConfig("disk_path must not be empty".into()));
        }
        if self.disk_path.is_dir() {
            return Err(CoreError::InvalidConfig(format!(
                "disk_path points to a directory, please point to a qcow2 file (e.g. {}\\disk.qcow2)",
                self.disk_path.display()
            )));
        }
        if self.vnc_port != 0 && !(5901..=65535).contains(&self.vnc_port) {
            return Err(CoreError::InvalidConfig(
                "vnc_port must be 0 (auto) or in the 5901..=65535 range".into(),
            ));
        }
        if let Some(iso) = &self.iso_path {
            if !iso.is_file() {
                return Err(CoreError::InvalidConfig(format!(
                    "iso_path does not exist or is not a file: {}",
                    iso.display()
                )));
            }
        }
        Ok(())
    }
}

/// Estado actual de una VM.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum VmStatus {
    /// La VM está apagada.
    Stopped,
    /// La VM está en ejecución.
    Running,
    /// La VM está pausada (vCPU congelada, memoria residente).
    Paused,
    /// La VM entró en un estado de error con un mensaje descriptivo.
    Error(String),
}

/// Vista resumida de una VM, pensada para listados en la UI.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VmSummary {
    /// Identificador único de la VM.
    pub id: VmId,
    /// Nombre visible de la VM.
    pub name: String,
    /// Estado actual de la VM.
    pub status: VmStatus,
    /// RAM asignada, en megabytes.
    pub ram_mb: u32,
    /// Número de vCPUs.
    pub cpus: u32,
    /// Puerto TCP activo del VNC (entra para noVNC), 0 si la VM no está
    /// en ejecución.
    pub vnc_port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_id_display_is_uuid() {
        let id = VmId::new();
        assert_eq!(id.to_string(), id.0.to_string());
    }

    #[test]
    fn validate_rejects_empty_name() {
        let config = VmConfig {
            name: "  ".into(),
            ram_mb: 1024,
            cpus: 2,
            iso_path: None,
            disk_path: "disk.qcow2".into(),
            disk_size_gb: 10,
            vnc_port: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_resources() {
        let config = VmConfig {
            name: "vm".into(),
            ram_mb: 0,
            cpus: 0,
            iso_path: None,
            disk_path: "disk.qcow2".into(),
            disk_size_gb: 10,
            vnc_port: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_invalid_vnc_port() {
        let config = VmConfig {
            name: "vm".into(),
            ram_mb: 1024,
            cpus: 1,
            iso_path: None,
            disk_path: "disk.qcow2".into(),
            disk_size_gb: 10,
            vnc_port: 5900,
        };
        assert!(config.validate().is_err());
    }
}
