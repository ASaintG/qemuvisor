/* Declaración de tipos para @novnc/novnc (el paquete no publica .d.ts).
 * Solo declaramos la superficie de API que usa VncModal.svelte. */
declare module "@novnc/novnc" {
  export default class RFB {
    constructor(
      target: HTMLElement | null,
      url: string,
      options?: Record<string, unknown>
    );

    /** Escala la pantalla del servidor al tamaño del contenedor. */
    scaleViewport: boolean;
    /** Pide al servidor redimensionar al tamaño del viewer. */
    resizeSession: boolean;
    /** Color de fondo del canvas, en formato #RRGGBBAA. */
    backgroundColor: string;
    /** ¿Cliente conectado al servidor? */
    connected: boolean;
    /** Desconecta y limpia los listeners internos. */
    disconnect(): void;
    /** Envía Ctrl+Alt+Supr al servidor. */
    sendCtrlAltDel(): void;
    addEventListener(type: string, listener: EventListener): void;
    removeEventListener(type: string, listener: EventListener): void;
  }
}
