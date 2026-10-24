//! Abstracción agnóstica del hipervisor.

use crate::error::CoreResult;
use crate::types::{VmConfig, VmId, VmStatus, VmSummary};

/// Interfaz que todo backend de virtualización debe implementar.
///
/// Los métodos son asíncronos porque la implementación de referencia
/// ([`crate::QemuBackend`]) lanza y controla procesos externos.
///
/// TODO: añadir métodos de consola/monitor (QMP) y de captura de logs
/// cuando la GUI necesite interactuar con el display real de la VM.
#[async_trait::async_trait]
pub trait HypervisorBackend: Send + Sync {
    /// Registra una nueva VM con la configuración dada y devuelve su [`VmId`].
    async fn create_vm(&self, config: VmConfig) -> CoreResult<VmId>;

    /// Arranca la VM identificada por `id`.
    async fn start_vm(&self, id: &VmId) -> CoreResult<()>;

    /// Detiene la VM identificada por `id` (equivale a apagarla).
    async fn stop_vm(&self, id: &VmId) -> CoreResult<()>;

    /// Pausa la VM identificada por `id` sin perder su estado en memoria.
    async fn pause_vm(&self, id: &VmId) -> CoreResult<()>;

    /// Elimina la VM identificada por `id`, liberando sus recursos.
    async fn delete_vm(&self, id: &VmId) -> CoreResult<()>;

    /// Devuelve el estado actual de la VM identificada por `id`.
    async fn get_vm_status(&self, id: &VmId) -> CoreResult<VmStatus>;

    /// Devuelve un resumen de todas las VMs registradas en este backend.
    async fn list_vms(&self) -> CoreResult<Vec<VmSummary>>;
}
