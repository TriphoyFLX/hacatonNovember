export const IpcChannels = {
  APP_GET_INFO: 'app:get-info',
  APP_GET_VERSION: 'app:get-version',
  WINDOW_MINIMIZE: 'window:minimize',
  WINDOW_MAXIMIZE: 'window:maximize',
  WINDOW_CLOSE: 'window:close',
  WINDOW_IS_MAXIMIZED: 'window:is-maximized',
} as const;

export type IpcChannel = (typeof IpcChannels)[keyof typeof IpcChannels];
