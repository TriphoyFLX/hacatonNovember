export const IpcChannels = {
  APP_GET_INFO: 'app:get-info',
  APP_GET_VERSION: 'app:get-version',
  WINDOW_MINIMIZE: 'window:minimize',
  WINDOW_MAXIMIZE: 'window:maximize',
  WINDOW_CLOSE: 'window:close',
  WINDOW_IS_MAXIMIZED: 'window:is-maximized',
  AUTH_REGISTER: 'auth:register',
  AUTH_LOGIN: 'auth:login',
  AUTH_LOGOUT: 'auth:logout',
  AUTH_GET_SESSION: 'auth:get-session',
  AUTH_UPDATE_PROFILE: 'auth:update-profile',
} as const;

export type IpcChannel = (typeof IpcChannels)[keyof typeof IpcChannels];
