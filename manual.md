
```markdown
# Guía de Usuario de qemuvisor

Esta guía explica cómo utilizar `qemuvisor` para crear y gestionar tus máquinas virtuales.

## 1. Primeros Pasos (GUI)

Al abrir la aplicación por primera vez, verás una tabla vacía. Si `qemuvisor` no detecta QEMU instalado, mostrará un banner de advertencia.

1. Haz clic en **"+ Nueva VM"**.
2. Rellena los datos:
   - **Nombre**: Un nombre para identificar tu VM.
   - **RAM y CPUs**: Utiliza los deslizadores para asignar recursos (la app te limitará a lo que tu equipo puede soportar de forma segura).
   - **Tamaño del Disco**: En GB.
   - **Ruta de la ISO (Opcional)**: Si vas a instalar un sistema operativo, selecciona el archivo `.iso` aquí.
3. Haz clic en **"Crear VM"**. Aparecerá en la lista en estado `Stopped` (Detenida).

## 2. Arrancar y Conectar

1. Pulsa el botón **"Iniciar"** en la fila de tu VM. El estado cambiará a `Running` (En ejecución).
2. Aparecerá un botón llamado **"Conectar"**. Púlsalo.
3. Se abrirá un visor VNC integrado donde podrás ver la pantalla de tu máquina virtual (verás el instalador si le asignaste una ISO, o la consola de BIOS si el disco está vacío).

> **Nota sobre el rendimiento:** Si tu VM tarda demasiado en arrancar o se siente lenta, asegúrate de tener la aceleración por hardware habilitada (WHPX en Windows o KVM en Linux).

## 3. Consola y Logs

Dentro del visor, puedes cambiar a la pestaña **"Consola/Logs"** para ver lo que QEMU está reportando por detrás. Esto es útil si la VM no arranca y necesitas saber por qué.

## 4. Detener y Eliminar

- **Detener**: Apaga la máquina virtual de forma forzosa (simula desconectar el cable de alimentación).
- **Eliminar**: Elimina la máquina de la lista. *Nota: El archivo de disco `.qcow2` no se borra automáticamente para evitar pérdida accidental de datos.*

## 5. Uso de la CLI

Si eres un usuario avanzado, puedes usar la línea de comandos para automatizar tareas:

```bash
# Ver todas las VMs registradas
cargo run -p qemuvisor-cli -- list

# Crear una VM de 2GB RAM, 2 CPUs y 20GB de disco con una ISO
cargo run -p qemuvisor-cli -- create --name mi-servidor --ram 2048 --cpus 2 --size-gb 20 --iso /ruta/a/debian.iso

# Iniciar la VM (usa el ID que te devolvió el comando anterior)
cargo run -p qemuvisor-cli -- start <ID>