import type {
  AuthResult,
  LoginInput,
  ProfileResult,
  RegisterInput,
  SessionResult,
  UpdateProfileInput,
} from '../auth/schemas';
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
  [IpcChannels.AUTH_REGISTER]: { args: [RegisterInput]; result: AuthResult };
  [IpcChannels.AUTH_LOGIN]: { args: [LoginInput]; result: AuthResult };
  [IpcChannels.AUTH_LOGOUT]: { args: []; result: { ok: true } };
  [IpcChannels.AUTH_GET_SESSION]: { args: []; result: SessionResult };
  [IpcChannels.AUTH_UPDATE_PROFILE]: { args: [UpdateProfileInput]; result: ProfileResult };
}

export type FeelAiApi = {
  getAppInfo: () => Promise<AppInfo>;
  getVersion: () => Promise<string>;
  minimizeWindow: () => Promise<void>;
  maximizeWindow: () => Promise<boolean>;
  closeWindow: () => Promise<void>;
  isWindowMaximized: () => Promise<boolean>;
  register: (input: RegisterInput) => Promise<AuthResult>;
  login: (input: LoginInput) => Promise<AuthResult>;
  logout: () => Promise<{ ok: true }>;
  getSession: () => Promise<SessionResult>;
  updateProfile: (input: UpdateProfileInput) => Promise<ProfileResult>;
};

declare global {
  interface Window {
    feelAI: FeelAiApi;
  }
}

export {};
