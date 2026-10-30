import { create } from 'zustand';

export type CameraStatus = 'idle' | 'requesting' | 'active' | 'denied' | 'unavailable';
export type ThemeMode = 'dark' | 'light';

interface AppState {
  userName: string;
  cameraStatus: CameraStatus;
  aiStatus: 'ready' | 'calibrating' | 'offline';
  theme: ThemeMode;
  notificationsOpen: boolean;
  setUserName: (name: string) => void;
  setCameraStatus: (status: CameraStatus) => void;
  setAiStatus: (status: AppState['aiStatus']) => void;
  setTheme: (theme: ThemeMode) => void;
  toggleTheme: () => void;
  setNotificationsOpen: (open: boolean) => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  userName: 'Creator',
  cameraStatus: 'idle',
  aiStatus: 'ready',
  theme: 'dark',
  notificationsOpen: false,
  setUserName: (userName) => set({ userName }),
  setCameraStatus: (cameraStatus) => set({ cameraStatus }),
  setAiStatus: (aiStatus) => set({ aiStatus }),
  setTheme: (theme) => set({ theme }),
  toggleTheme: () => set({ theme: get().theme === 'dark' ? 'light' : 'dark' }),
  setNotificationsOpen: (notificationsOpen) => set({ notificationsOpen }),
}));
