import { contextBridge, ipcRenderer } from 'electron';
import { IpcChannels } from '../../shared/ipc/channels';
import type { AppInfo, FeelAiApi } from '../../shared/ipc/types';

const api: FeelAiApi = {
  getAppInfo: (): Promise<AppInfo> => ipcRenderer.invoke(IpcChannels.APP_GET_INFO),
  getVersion: (): Promise<string> => ipcRenderer.invoke(IpcChannels.APP_GET_VERSION),
  minimizeWindow: (): Promise<void> => ipcRenderer.invoke(IpcChannels.WINDOW_MINIMIZE),
  maximizeWindow: (): Promise<boolean> => ipcRenderer.invoke(IpcChannels.WINDOW_MAXIMIZE),
  closeWindow: (): Promise<void> => ipcRenderer.invoke(IpcChannels.WINDOW_CLOSE),
  isWindowMaximized: (): Promise<boolean> =>
    ipcRenderer.invoke(IpcChannels.WINDOW_IS_MAXIMIZED),
};

contextBridge.exposeInMainWorld('feelAI', api);
