import {
  loginSchema,
  registerSchema,
  updateProfileSchema,
  type AuthResult,
  type AuthSession,
  type LoginInput,
  type ProfileResult,
  type PublicUser,
  type RegisterInput,
  type SessionResult,
  type UpdateProfileInput,
} from '@shared/auth/schemas';

const STORAGE_USERS = 'feelai.auth.users';
const STORAGE_SESSION = 'feelai.auth.session';

interface LocalUser extends PublicUser {
  password: string;
}

function readUsers(): LocalUser[] {
  try {
    const raw = localStorage.getItem(STORAGE_USERS);
    if (!raw) {
      return [];
    }
    return JSON.parse(raw) as LocalUser[];
  } catch {
    return [];
  }
}

function writeUsers(users: LocalUser[]): void {
  localStorage.setItem(STORAGE_USERS, JSON.stringify(users));
}

function readSession(): AuthSession | null {
  try {
    const raw = localStorage.getItem(STORAGE_SESSION);
    if (!raw) {
      return null;
    }
    return JSON.parse(raw) as AuthSession;
  } catch {
    return null;
  }
}

function writeSession(session: AuthSession | null): void {
  if (!session) {
    localStorage.removeItem(STORAGE_SESSION);
    return;
  }
  localStorage.setItem(STORAGE_SESSION, JSON.stringify(session));
}

function createSession(userId: string): AuthSession {
  const now = Date.now();
  return {
    token: crypto.randomUUID(),
    userId,
    createdAt: new Date(now).toISOString(),
    expiresAt: new Date(now + 1000 * 60 * 60 * 24 * 30).toISOString(),
  };
}

function browserRegister(input: RegisterInput): AuthResult {
  const parsed = registerSchema.safeParse(input);
  if (!parsed.success) {
    return { ok: false, error: parsed.error.issues[0]?.message ?? 'Некорректные данные' };
  }

  const users = readUsers();
  const email = parsed.data.email.trim().toLowerCase();
  if (users.some((user) => user.email === email)) {
    return { ok: false, error: 'Пользователь с таким email уже существует' };
  }

  const now = new Date().toISOString();
  const user: LocalUser = {
    id: crypto.randomUUID(),
    email,
    displayName: parsed.data.displayName.trim(),
    avatar: parsed.data.displayName.trim().slice(0, 1).toUpperCase(),
    password: parsed.data.password,
    createdAt: now,
    updatedAt: now,
  };
  users.push(user);
  writeUsers(users);
  const session = createSession(user.id);
  writeSession(session);

  const { password: _password, ...publicUser } = user;
  return { ok: true, user: publicUser, session };
}

function browserLogin(input: LoginInput): AuthResult {
  const parsed = loginSchema.safeParse(input);
  if (!parsed.success) {
    return { ok: false, error: parsed.error.issues[0]?.message ?? 'Некорректные данные' };
  }

  const email = parsed.data.email.trim().toLowerCase();
  const user = readUsers().find(
    (item) => item.email === email && item.password === parsed.data.password,
  );
  if (!user) {
    return { ok: false, error: 'Неверный email или пароль' };
  }

  const session = createSession(user.id);
  writeSession(session);
  const { password: _password, ...publicUser } = user;
  return { ok: true, user: publicUser, session };
}

function browserGetSession(): SessionResult {
  const session = readSession();
  if (!session || Date.parse(session.expiresAt) <= Date.now()) {
    writeSession(null);
    return { ok: false, user: null, session: null };
  }

  const user = readUsers().find((item) => item.id === session.userId);
  if (!user) {
    writeSession(null);
    return { ok: false, user: null, session: null };
  }

  const { password: _password, ...publicUser } = user;
  return { ok: true, user: publicUser, session };
}

function browserUpdateProfile(input: UpdateProfileInput): ProfileResult {
  const parsed = updateProfileSchema.safeParse(input);
  if (!parsed.success) {
    return { ok: false, error: parsed.error.issues[0]?.message ?? 'Некорректные данные' };
  }

  const active = browserGetSession();
  if (!active.ok || !active.user) {
    return { ok: false, error: 'Сессия не найдена. Войдите снова.' };
  }

  const users = readUsers();
  const index = users.findIndex((item) => item.id === active.user.id);
  if (index < 0) {
    return { ok: false, error: 'Не удалось обновить профиль' };
  }

  users[index] = {
    ...users[index],
    displayName: parsed.data.displayName,
    avatar: parsed.data.displayName.slice(0, 1).toUpperCase(),
    updatedAt: new Date().toISOString(),
  };
  writeUsers(users);
  const { password: _password, ...publicUser } = users[index];
  return { ok: true, user: publicUser };
}

function getApi() {
  return typeof window !== 'undefined' ? window.feelAI : undefined;
}

export const authClient = {
  register(input: RegisterInput): Promise<AuthResult> {
    const api = getApi();
    if (api?.register) {
      return api.register(input);
    }
    return Promise.resolve(browserRegister(input));
  },
  login(input: LoginInput): Promise<AuthResult> {
    const api = getApi();
    if (api?.login) {
      return api.login(input);
    }
    return Promise.resolve(browserLogin(input));
  },
  logout(): Promise<{ ok: true }> {
    const api = getApi();
    if (api?.logout) {
      return api.logout();
    }
    writeSession(null);
    return Promise.resolve({ ok: true });
  },
  getSession(): Promise<SessionResult> {
    const api = getApi();
    if (api?.getSession) {
      return api.getSession();
    }
    return Promise.resolve(browserGetSession());
  },
  updateProfile(input: UpdateProfileInput): Promise<ProfileResult> {
    const api = getApi();
    if (api?.updateProfile) {
      return api.updateProfile(input);
    }
    return Promise.resolve(browserUpdateProfile(input));
  },
};
