import type { IpcChannels } from './channels';

export interface AppInfo {
  name: string;
  version: string;
  platform: NodeJS.Platform;
  arch: string;
  electronVersion: string;
  chromeVersion: string;
  nodeVersion: string;
  isPackaged: boolean;
}

export interface IpcInvokeMap {
  [IpcChannels.APP_GET_INFO]: { args: []; result: AppInfo };
  [IpcChannels.APP_GET_VERSION]: { args: []; result: string };
  [IpcChannels.WINDOW_MINIMIZE]: { args: []; result: void };
  [IpcChannels.WINDOW_MAXIMIZE]: { args: []; result: boolean };
  [IpcChannels.WINDOW_CLOSE]: { args: []; result: void };
  [IpcChannels.WINDOW_IS_MAXIMIZED]: { args: []; result: boolean };
}

export type FeelAiApi = {
  getAppInfo: () => Promise<AppInfo>;
  getVersion: () => Promise<string>;
  minimizeWindow: () => Promise<void>;
  maximizeWindow: () => Promise<boolean>;
  closeWindow: () => Promise<void>;
  isWindowMaximized: () => Promise<boolean>;
};

declare global {
  interface Window {
    feelAI: FeelAiApi;
  }
}

export {};
