import { BrowserWindow, app, ipcMain } from 'electron';
import { IpcChannels } from '../../../shared/ipc/channels';
import type { AppInfo } from '../../../shared/ipc/types';

type WindowProvider = () => BrowserWindow | null;

export function registerIpcHandlers(getWindow: WindowProvider): void {
  ipcMain.handle(IpcChannels.APP_GET_INFO, (): AppInfo => {
    return {
      name: app.getName(),
      version: app.getVersion(),
      platform: process.platform,
      arch: process.arch,
      electronVersion: process.versions.electron,
      chromeVersion: process.versions.chrome,
      nodeVersion: process.versions.node,
      isPackaged: app.isPackaged,
    };
  });

  ipcMain.handle(IpcChannels.APP_GET_VERSION, (): string => app.getVersion());

  ipcMain.handle(IpcChannels.WINDOW_MINIMIZE, (): void => {
    getWindow()?.minimize();
  });

  ipcMain.handle(IpcChannels.WINDOW_MAXIMIZE, (): boolean => {
    const window = getWindow();
    if (!window) {
      return false;
    }

    if (window.isMaximized()) {
      window.unmaximize();
      return false;
    }

    window.maximize();
    return true;
  });

  ipcMain.handle(IpcChannels.WINDOW_CLOSE, (): void => {
    getWindow()?.close();
  });

  ipcMain.handle(IpcChannels.WINDOW_IS_MAXIMIZED, (): boolean => {
    return getWindow()?.isMaximized() ?? false;
  });
}
