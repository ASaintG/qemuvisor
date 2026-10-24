use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use qemuvisor_core::{HypervisorBackend, QemuBackend, VmConfig, VmId, VmSummary, create_disk};
use tabled::{Table, Tabled};
use uuid::Uuid;

/// Gestor de máquinas virtuales basado en QEMU.
#[derive(Parser)]
#[command(
    name = "qemuvisor",
    version,
    about = "CLI para gestionar VMs con QEMU vía qemuvisor-core"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verifica que QEMU y qemu-img estén disponibles en este sistema.
    CheckDeps,
    /// Lista las VMs registradas y su estado.
    List,
    /// Crea una nueva VM. Si no existe, el disco se genera automáticamente.
    Create {
        /// Nombre visible de la VM.
        #[arg(long)]
        name: String,
        /// RAM asignada, en megabytes.
        #[arg(long)]
        ram: u32,
        /// Número de vCPUs.
        #[arg(long)]
        cpus: u32,
        /// Ruta a una ISO para arrancar desde CD-ROM (opcional).
        #[arg(long)]
        iso: Option<PathBuf>,
        /// Ruta del disco qcow2 (opcional; por defecto, una en el directorio de datos).
        #[arg(long)]
        disk: Option<PathBuf>,
        /// Tamaño del disco a crear si no existe, en GB.
        #[arg(long, default_value_t = 10)]
        size_gb: u64,
        /// Puerto TCP fijo del VNC (5901..=65535); por defecto, automático.
        #[arg(long, default_value_t = 0)]
        vnc_port: u16,
    },
    /// Inicia una VM dado su ID.
    Start {
        /// ID de la VM (UUID v4, obtenido con `list` o al crearla).
        id: String,
    },
    /// Detiene una VM dado su ID.
    Stop {
        /// ID de la VM (UUID v4, obtenido con `list` o al crearla).
        id: String,
    },
    /// Elimina una VM dado su ID (el disco no se borra del sistema).
    Delete {
        /// ID de la VM (UUID v4, obtenido con `list` o al crearla).
        id: String,
    },
    /// Operaciones sobre discos virtuales (qemu-img).
    Disk {
        #[command(subcommand)]
        command: DiskCommands,
    },
}

#[derive(Subcommand)]
enum DiskCommands {
    /// Crea una imagen de disco qcow2 con qemu-img.
    Create {
        /// Ruta destino del disco.
        #[arg(long)]
        path: PathBuf,
        /// Tamaño del disco, en gigabytes.
        #[arg(long)]
        size_gb: u64,
    },
}

/// Fila de la tabla `list` (deriva `Tabled` para tabled).
#[derive(Tabled)]
struct VmRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Nombre")]
    name: String,
    #[tabled(rename = "Estado")]
    status: String,
    #[tabled(rename = "RAM")]
    ram: String,
    #[tabled(rename = "CPUs")]
    cpus: u32,
}

impl VmRow {
    fn from_summary(summary: VmSummary) -> Self {
        let status = match summary.status {
            qemuvisor_core::VmStatus::Running => "Running".to_string(),
            qemuvisor_core::VmStatus::Paused => "Paused".to_string(),
            qemuvisor_core::VmStatus::Stopped => "Stopped".to_string(),
            qemuvisor_core::VmStatus::Error(err) => format!("Error ({err})"),
        };
        Self {
            id: summary.id.to_string(),
            name: summary.name,
            status,
            ram: format!("{} MB", summary.ram_mb),
            cpus: summary.cpus,
        }
    }
}

/// Directorio por defecto de metadatos/discos: `<cwd>/vms`, coherente con la
/// GUI, que usa `<AppData>/vms`.
fn default_vms_dir() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("vms"))
}

/// Ruta de disco por defecto para `create` si no se pasa `--disk`.
fn default_disk_path(vms_dir: &PathBuf, name: &str) -> PathBuf {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    vms_dir.join("disks").join(format!("{slug}.qcow2"))
}

/// Convierte un ID textual a [`VmId`], mapeando el error de parseo a un
/// mensaje amigable.
fn parse_vm_id(id: &str) -> Result<VmId> {
    Ok(VmId(
        Uuid::parse_str(id)
            .map_err(|e| anyhow::anyhow!("ID de VM inválido {id:?}: {e}"))?,
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let vms_dir = default_vms_dir()?;
    fs::create_dir_all(&vms_dir)?;
    let backend = QemuBackend::new(&vms_dir);
    // Hidrata el registro desde los JSON persistidos en <cwd>/vms.
    // Con esto `create` y luego `start` en procesos separados ya funcionan.
    backend.restore_persisted().await?;

    match cli.command {
        Commands::CheckDeps => {
            let status = backend.check_dependencies().await;

            println!("=== qemuvisor: verificación de dependencias ===");
            println!(
                "qemu-system-x86_64 : {}",
                status
                    .qemu_path
                    .as_deref()
                    .unwrap_or("NO ENCONTRADO")
            );
            println!(
                "qemu-img           : {}",
                status.qemu_img_path.as_deref().unwrap_or("NO ENCONTRADO")
            );
            println!(
                "QEMU disponible    : {}",
                if status.qemu_available { "sí" } else { "no" }
            );
            println!(
                "qemu-img disponible: {}",
                if status.qemu_img_available { "sí" } else { "no" }
            );

            if let Some(message) = status.error_message {
                eprintln!("\n{message}");
                std::process::exit(1);
            }
            println!("\nOK: puede ejecutar operaciones con VMs.");
        }
        Commands::List => {
            let vms = backend.list_vms().await?;
            let rows: Vec<VmRow> = vms.into_iter().map(VmRow::from_summary).collect();
            if rows.is_empty() {
                println!("No hay VMs registradas.");
            } else {
                println!("{}", Table::new(rows));
            }
        }
        Commands::Create {
            name,
            ram,
            cpus,
            iso,
            disk,
            size_gb,
            vnc_port,
        } => {
            let disk_path = disk.unwrap_or_else(|| default_disk_path(&vms_dir, &name));
            let config = VmConfig {
                name: name.clone(),
                ram_mb: ram,
                cpus,
                iso_path: iso,
                disk_path,
                disk_size_gb: size_gb,
                vnc_port,
            };
            let id = backend.create_vm(config).await?;
            println!("VM «{name}» creada con ID {id}");
            println!("Usa `qemuvisor start {id}` para arrancarla.");
        }
        Commands::Start { id } => {
            backend.start_vm(&parse_vm_id(&id)?).await?;
            println!("VM {id} iniciada.");
        }
        Commands::Stop { id } => {
            backend.stop_vm(&parse_vm_id(&id)?).await?;
            println!("VM {id} detenida.");
        }
        Commands::Delete { id } => {
            backend.delete_vm(&parse_vm_id(&id)?).await?;
            println!("VM {id} eliminada (el disco se conserva en disco).");
        }
        Commands::Disk {
            command: DiskCommands::Create { path, size_gb },
        } => {
            create_disk(&path, size_gb).await?;
            println!("Disco qcow2 de {size_gb} GB creado en {}", path.display());
        }
    }

    Ok(())
}
