export interface AlteredRenderMountResult {
  canvas: HTMLCanvasElement;
}

export interface AlteredRenderApi {
  init(options?: Record<string, unknown>): Promise<unknown>;
  mountFromApi(
    element: HTMLElement,
    apiJson: Record<string, unknown>,
    fieldMap?: Record<string, unknown>,
    options?: Record<string, unknown>,
  ): Promise<AlteredRenderMountResult>;
}

declare global {
  interface Window {
    AlteredRender: AlteredRenderApi;
  }
}

export {};
