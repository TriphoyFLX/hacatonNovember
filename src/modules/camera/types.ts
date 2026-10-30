export type CameraPermissionState = 'prompt' | 'granted' | 'denied' | 'unavailable';

export interface CameraDeviceInfo {
  deviceId: string;
  label: string;
}

export interface CameraStartOptions {
  deviceId?: string;
  width?: number;
  height?: number;
  facingMode?: 'user' | 'environment';
}

export interface CameraSnapshot {
  stream: MediaStream | null;
  devices: CameraDeviceInfo[];
  activeDeviceId: string | null;
  status: 'idle' | 'requesting' | 'active' | 'denied' | 'unavailable' | 'error';
  errorMessage: string | null;
  fps: number;
}
