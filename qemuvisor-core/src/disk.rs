//! Operaciones de discos virtuales vía `qemu-img`.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use crate::deps::{QEMU_IMG_BINARY, find_binary};
use crate::error::{CoreError, CoreResult};

/// Ejecuta `qemu-img` con los argumentos dados y traduce los fallos a [`CoreError`].
async fn run_qemu_img(qemu_img: &Path, args: &[&str]) -> CoreResult<()> {
    let output = Command::new(qemu_img)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                CoreError::QemuImgNotFound
            } else {
                CoreError::Io(err)
            }
        })?;

    if !output.status.success() {
        let code = output.status.code();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CoreError::QemuImgFailed { code, stderr });
    }

    Ok(())
}

/// Localiza `qemu-img` con la estrategia de [`crate::deps::find_binary`]
/// antes de invocarlo.
async fn resolve_qemu_img() -> CoreResult<std::path::PathBuf> {
    find_binary(QEMU_IMG_BINARY).await
}

/// Crea una imagen de disco nueva en formato `qcow2`.
///
/// # Errores
/// Devuelve [`CoreError::BinaryNotFound`] si `qemu-img` no se encuentra en
/// ubicaciones estándar o [`CoreError::QemuImgFailed`] si el comando falla.
pub async fn create_disk(path: &Path, size_gb: u64) -> CoreResult<()> {
    let qemu_img = resolve_qemu_img().await?;
    let size_arg = format!("{size_gb}G");
    let path_arg = path.as_os_str().to_string_lossy().to_string();
    run_qemu_img(&qemu_img, &["create", "-f", "qcow2", &path_arg, &size_arg]).await
}

/// Cambia el tamaño de una imagen de disco existente.
///
/// # Errores
/// Devuelve [`CoreError::BinaryNotFound`] si `qemu-img` no se encuentra en
/// ubicaciones estándar o [`CoreError::QemuImgFailed`] si el comando falla.
pub async fn resize_disk(path: &Path, new_size_gb: u64) -> CoreResult<()> {
    let qemu_img = resolve_qemu_img().await?;
    let size_arg = format!("{new_size_gb}G");
    let path_arg = path.as_os_str().to_string_lossy().to_string();
    run_qemu_img(&qemu_img, &["resize", &path_arg, &size_arg]).await
}

/// Convierte una imagen de disco a otro formato (por ejemplo, `raw`, `vmdk`, `qcow2`).
///
/// # Errores
/// Devuelve [`CoreError::BinaryNotFound`] si `qemu-img` no se encuentra en
/// ubicaciones estándar o [`CoreError::QemuImgFailed`] si el comando falla.
pub async fn convert_disk(src: &Path, dst: &Path, format: &str) -> CoreResult<()> {
    let qemu_img = resolve_qemu_img().await?;
    let src_arg = src.as_os_str().to_string_lossy().to_string();
    let dst_arg = dst.as_os_str().to_string_lossy().to_string();
    run_qemu_img(&qemu_img, &["convert", "-O", format, &src_arg, &dst_arg]).await
}
