import { contextBridge, ipcRenderer } from 'electron';
import { IpcChannels } from '../../shared/ipc/channels';
import type { AppInfo, FeelAiApi } from '../../shared/ipc/types';
import type {
  AuthResult,
  LoginInput,
  ProfileResult,
  RegisterInput,
  SessionResult,
  UpdateProfileInput,
} from '../../shared/auth/schemas';

const api: FeelAiApi = {
  getAppInfo: (): Promise<AppInfo> => ipcRenderer.invoke(IpcChannels.APP_GET_INFO),
  getVersion: (): Promise<string> => ipcRenderer.invoke(IpcChannels.APP_GET_VERSION),
  minimizeWindow: (): Promise<void> => ipcRenderer.invoke(IpcChannels.WINDOW_MINIMIZE),
  maximizeWindow: (): Promise<boolean> => ipcRenderer.invoke(IpcChannels.WINDOW_MAXIMIZE),
  closeWindow: (): Promise<void> => ipcRenderer.invoke(IpcChannels.WINDOW_CLOSE),
  isWindowMaximized: (): Promise<boolean> =>
    ipcRenderer.invoke(IpcChannels.WINDOW_IS_MAXIMIZED),
  register: (input: RegisterInput): Promise<AuthResult> =>
    ipcRenderer.invoke(IpcChannels.AUTH_REGISTER, input),
  login: (input: LoginInput): Promise<AuthResult> =>
    ipcRenderer.invoke(IpcChannels.AUTH_LOGIN, input),
  logout: (): Promise<{ ok: true }> => ipcRenderer.invoke(IpcChannels.AUTH_LOGOUT),
  getSession: (): Promise<SessionResult> => ipcRenderer.invoke(IpcChannels.AUTH_GET_SESSION),
  updateProfile: (input: UpdateProfileInput): Promise<ProfileResult> =>
    ipcRenderer.invoke(IpcChannels.AUTH_UPDATE_PROFILE, input),
};

contextBridge.exposeInMainWorld('feelAI', api);
