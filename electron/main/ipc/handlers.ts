import { BrowserWindow, app, ipcMain } from 'electron';
import { IpcChannels } from '../../../shared/ipc/channels';
import type { AppInfo } from '../../../shared/ipc/types';
import type {
  LoginInput,
  RegisterInput,
  UpdateProfileInput,
} from '../../../shared/auth/schemas';
import {
  getCurrentSession,
  login,
  logout,
  register,
  updateProfile,
} from '../services/authService';

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

  ipcMain.handle(IpcChannels.AUTH_REGISTER, (_event, input: RegisterInput) => register(input));
  ipcMain.handle(IpcChannels.AUTH_LOGIN, (_event, input: LoginInput) => login(input));
  ipcMain.handle(IpcChannels.AUTH_LOGOUT, () => logout());
  ipcMain.handle(IpcChannels.AUTH_GET_SESSION, () => getCurrentSession());
  ipcMain.handle(IpcChannels.AUTH_UPDATE_PROFILE, (_event, input: UpdateProfileInput) =>
    updateProfile(input),
  );
}
