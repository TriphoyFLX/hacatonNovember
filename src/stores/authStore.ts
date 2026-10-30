import { create } from 'zustand';
import type {
  LoginInput,
  PublicUser,
  RegisterInput,
  UpdateProfileInput,
} from '@shared/auth/schemas';
import { authClient } from '@/services/authClient';
import { useAppStore } from './appStore';

interface AuthState {
  user: PublicUser | null;
  bootstrapped: boolean;
  loading: boolean;
  error: string | null;
  bootstrap: () => Promise<void>;
  login: (input: LoginInput) => Promise<boolean>;
  register: (input: RegisterInput) => Promise<boolean>;
  logout: () => Promise<void>;
  updateProfile: (input: UpdateProfileInput) => Promise<boolean>;
  clearError: () => void;
}

function syncDisplayName(user: PublicUser | null): void {
  useAppStore.getState().setUserName(user?.displayName ?? 'Творец');
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  bootstrapped: false,
  loading: false,
  error: null,

  clearError: () => set({ error: null }),

  bootstrap: async () => {
    set({ loading: true, error: null });
    try {
      const session = await authClient.getSession();
      if (session.ok && session.user) {
        syncDisplayName(session.user);
        set({ user: session.user, bootstrapped: true, loading: false });
        return;
      }
      syncDisplayName(null);
      set({ user: null, bootstrapped: true, loading: false });
    } catch {
      set({
        user: null,
        bootstrapped: true,
        loading: false,
        error: 'Не удалось проверить сессию',
      });
    }
  },

  login: async (input) => {
    set({ loading: true, error: null });
    const result = await authClient.login(input);
    if (!result.ok) {
      set({ loading: false, error: result.error });
      return false;
    }
    syncDisplayName(result.user);
    set({ user: result.user, loading: false, error: null });
    return true;
  },

  register: async (input) => {
    set({ loading: true, error: null });
    const result = await authClient.register(input);
    if (!result.ok) {
      set({ loading: false, error: result.error });
      return false;
    }
    syncDisplayName(result.user);
    set({ user: result.user, loading: false, error: null });
    return true;
  },

  logout: async () => {
    set({ loading: true, error: null });
    await authClient.logout();
    syncDisplayName(null);
    set({ user: null, loading: false });
  },

  updateProfile: async (input) => {
    set({ loading: true, error: null });
    const result = await authClient.updateProfile(input);
    if (!result.ok) {
      set({ loading: false, error: result.error });
      return false;
    }
    syncDisplayName(result.user);
    set({ user: result.user, loading: false, error: null });
    return true;
  },
}));
