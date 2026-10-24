# qemuvisor 🖥️🚀

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-blue.svg)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/Svelte-v5-red.svg)](https://svelte.dev/)

Un gestor de máquinas virtuales de escritorio ligero, rápido y de código abierto construido sobre **QEMU**.

`qemuvisor` abstrae la complejidad de la línea de comandos de QEMU y ofrece una interfaz gráfica moderna y nativa construida con **Rust**, **Tauri** y **Svelte**.

---

## ✨ Características Principales

*   **⚡ Interfaz Gráfica Nativa**: Construida con **Tauri v2** y **Svelte 5**, ofreciendo un rendimiento extremadamente ligero y bajo consumo de recursos en comparación con Electron.
*   **🖥️ Visor VNC Integrado**: Conéctate a la pantalla de tu VM directamente dentro de la aplicación usando `noVNC`, sin necesidad de instalar clientes externos.
*   **🚀 Aceleración por Hardware**: Detección automática de hipervisores para máximo rendimiento:
    *   **Windows**: WHPX (`HypervisorPlatform`)
    *   **Linux**: KVM (`/dev/kvm`)
*   **💾 Gestión de Discos**: Creación y manipulación automática de discos virtuales en formato `qcow2` con tamaño dinámico y configurable.
*   **📌 Persistencia de Estado**: Las configuraciones de las VMs se persisten en formato JSON y sobreviven a los reinicios de la aplicación.
*   **🌐 Red NAT por Defecto**: Acceso a internet listo para usar inmediatamente al arrancar la máquina virtual.
*   **🛠️ Doble Interfaz**: Incluye una aplicación gráfica (GUI) y una herramienta de línea de comandos (CLI) para automatización.

---

## 🏗️ Arquitectura del Proyecto

El proyecto está estructurado como un **Cargo Workspace** en Rust para mantener una separación clara de responsabilidades:

```text
qemuvisor/
├── qemuvisor-core/     # Librería central: lógica de QEMU, red, discos y abstracciones
├── qemuvisor-cli/      # Interfaz de línea de comandos para automatización y scripts
└── qemuvisor-gui/      # Frontend de escritorio (Tauri v2 + Svelte 5)
```

| Componente | Descripción |
| :--- | :--- |
| **`qemuvisor-core`** | Define la *trait* `HypervisorBackend` (permitiendo soportar hipervisores adicionales en el futuro) y maneja el ciclo de vida de los procesos de QEMU. |
| **`qemuvisor-cli`** | Herramienta CLI rápida e independiente para administrar máquinas virtuales sin interfaz visual. |
| **`qemuvisor-gui`** | Capa visual con Svelte que se comunica con el backend mediante comandos de Tauri. |

---

## 📋 Requisitos Previos

Asegúrate de contar con los siguientes componentes en tu sistema antes de compilar o ejecutar el proyecto:

*   [Rust](https://www.rust-lang.org/) (canal `stable`)
*   [Node.js](https://nodejs.org/) (versión 18+ con `pnpm` o `npm`)
*   **QEMU** instalado y accesible desde el `PATH` de tu sistema:
    *   `qemu-system-x86_64`
    *   `qemu-img`

> [!TIP]
> **Aceleración por Hardware:**
> - **Windows:** Habilita la característica *Windows Hypervisor Platform* (WHPX).
> - **Linux:** Asegúrate de que tu usuario tenga permisos de lectura/escritura sobre `/dev/kvm` (ej. añadiéndote al grupo `kvm`).

---

## 🚀 Instalación y Desarrollo

### 1. Clonar el Repositorio

```bash
git clone https://github.com/ASaintG/qemuvisor.git
cd qemuvisor
```

### 2. Ejecutar la Aplicación Gráfica (GUI)

```bash
cd qemuvisor-gui
npm install
npm run tauri dev
```

### 3. Usar la Línea de Comandos (CLI)

Puedes interactuar con el backend directamente mediante la herramienta CLI:

```bash
# Ver ayuda y comandos disponibles
cargo run -p qemuvisor-cli -- --help

# Listar máquinas virtuales configuradas
cargo run -p qemuvisor-cli -- list
```

---

## 🤝 Contribuciones

¡Las contribuciones son bien recibidas! Si deseas colaborar con el desarrollo de `qemuvisor`:

1. Haz un **Fork** del repositorio.
2. Crea una rama para tu funcionalidad (`git checkout -b feature/nueva-funcionalidad`).
3. Realiza los cambios y realiza un **Commit** (`git commit -m 'Añade nueva funcionalidad'`).
4. Haz **Push** a la rama (`git push origin feature/nueva-funcionalidad`).
5. Abre un **Pull Request**.

---
## ⚠️ Descargo de responsabilidad

`qemuvisor` es un proyecto independiente y de código abierto. No está afiliado, respaldado ni patrocinado por el proyecto oficial QEMU ni por Software Freedom Conservancy, Inc. "QEMU" es una marca registrada de Fabrice Bellard.

## 📄 Licencia

Este proyecto está bajo la **Licencia MIT**. Consulta el archivo [LICENSE](LICENSE) para obtener más información.