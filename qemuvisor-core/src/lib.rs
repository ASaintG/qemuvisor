//! `qemuvisor-core`: capa de dominio y backend QEMU para el gestor de VMs.
//!
//! Este crate encapsula toda la lógica de virtualización:
//!
//! - [`HypervisorBackend`]: trait agnóstico del hipervisor (QEMU hoy, otros mañana).
//! - [`QemuBackend`]: implementación de referencia sobre `qemu-system-x86_64`.
//! - [`CoreError`]: dominio de errores compartido con CLI y GUI.
//! - [`create_disk`] y compañía: manipulación de discos vía `qemu-img`.
//!
//! La GUI (Tauri) y la CLI deben depender únicamente de estos tipos y de la
//! trait [`HypervisorBackend`], manteniendo el acoplamiento a QEMU dentro de
//! este crate.

pub mod backend;
pub mod deps;
pub mod disk;
pub mod error;
pub mod qemu;
pub mod types;

pub use backend::HypervisorBackend;
pub use deps::{DependencyStatus, QEMU_IMG_BINARY, QEMU_SYSTEM_BINARY, find_binary, find_binary_sync};
pub use disk::{convert_disk, create_disk, resize_disk};
pub use error::{CoreError, CoreResult, install_hint_for_current_os};
pub use qemu::{QemuBackend, quick_dependency_check};
pub use types::{VmConfig, VmId, VmStatus, VmSummary};
